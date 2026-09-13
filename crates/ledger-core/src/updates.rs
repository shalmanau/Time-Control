use anyhow::{ensure, Context, Result};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub version: String,
    pub version_code: u64,
    pub apk_url: String,
    pub sha256: String,
    pub signature: String,
}
pub fn validate_endpoint(url: &str) -> Result<()> {
    let u = reqwest::Url::parse(url)?;
    ensure!(
        u.scheme() == "https"
            && u.host_str().is_some()
            && u.username().is_empty()
            && u.password().is_none(),
        "Use an HTTPS URL without embedded credentials"
    );
    Ok(())
}
fn validate_redirect(url: &str, previous: usize) -> Result<()> {
    ensure!(previous < 5, "Too many update redirects");
    validate_endpoint(url)
}
fn client() -> Result<reqwest::blocking::Client> {
    Ok(reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(90))
        .redirect(reqwest::redirect::Policy::custom(
            |attempt| match validate_redirect(attempt.url().as_str(), attempt.previous().len()) {
                Ok(()) => attempt.follow(),
                Err(e) => attempt.error(e.to_string()),
            },
        ))
        .build()?)
}
pub fn verify(m: &Manifest, key: &str, current: &str) -> Result<()> {
    validate_endpoint(&m.apk_url)?;
    ensure!(
        semver::Version::parse(&m.version)? > semver::Version::parse(current)?,
        "Already up to date"
    );
    verify_signature(m, key)
}
pub fn verify_signature(m: &Manifest, key: &str) -> Result<()> {
    validate_endpoint(&m.apk_url)?;
    semver::Version::parse(&m.version)?;
    ensure!(m.version_code > 0, "Invalid Android version code");
    super::bytes::<32>(&m.sha256)?;
    let message = format!(
        "{}\n{}\n{}\n{}",
        m.version, m.version_code, m.apk_url, m.sha256
    );
    VerifyingKey::from_bytes(&super::bytes(key)?)?
        .verify(
            message.as_bytes(),
            &Signature::from_bytes(&super::bytes(&m.signature)?),
        )
        .context("Update signature is invalid")?;
    Ok(())
}
pub fn check(url: &str, key: &str, current: &str) -> Result<Option<Manifest>> {
    validate_endpoint(url)?;
    let response = client()?.get(url).send()?.error_for_status()?;
    ensure!(
        response.status().is_success(),
        "Release host did not return a manifest"
    );
    let mut data = vec![];
    response.take(65537).read_to_end(&mut data)?;
    ensure!(data.len() <= 65536, "Release manifest is too large");
    let m: Manifest = serde_json::from_slice(&data)?;
    // Verify authenticity even when the version is equal or older.
    verify_signature(&m, key)?;
    if semver::Version::parse(&m.version)? <= semver::Version::parse(current)? {
        return Ok(None);
    }
    Ok(Some(m))
}
pub fn download(m: &Manifest, key: &str, current: &str, directory: &Path) -> Result<PathBuf> {
    verify(m, key, current)?;
    std::fs::create_dir_all(directory)?;
    let partial = directory.join("update.apk.part");
    let output = directory.join("update.apk");
    let result = (|| -> Result<()> {
        let mut response = client()?.get(&m.apk_url).send()?.error_for_status()?;
        ensure!(
            response.status().is_success(),
            "Release host did not return an APK"
        );
        let mut file = std::fs::File::create(&partial)?;
        let mut hash = Sha256::new();
        let mut total = 0usize;
        let mut buffer = [0u8; 65536];
        loop {
            let n = response.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            total += n;
            ensure!(total <= 200 * 1024 * 1024, "Update package is too large");
            hash.update(&buffer[..n]);
            file.write_all(&buffer[..n])?;
        }
        ensure!(
            hex::encode(hash.finalize()) == m.sha256,
            "Update checksum does not match"
        );
        file.sync_all()?;
        std::fs::rename(&partial, &output)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&partial);
    }
    result?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn github_https_redirects_allowed_but_downgrades_credentials_and_loops_rejected() {
        assert!(validate_redirect("https://release-assets.githubusercontent.com/github-production-release-asset/file?token=test", 2).is_ok());
        assert!(validate_redirect("http://example.com/update.apk", 1).is_err());
        assert!(validate_redirect("https://user:password@example.com/update.apk", 1).is_err());
        assert!(validate_redirect("https://github.com/loop", 5).is_err());
    }
}
