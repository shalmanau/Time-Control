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
fn client() -> Result<reqwest::blocking::Client> {
    Ok(reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(90))
        .redirect(reqwest::redirect::Policy::none())
        .build()?)
}
pub fn verify(m: &Manifest, key: &str, current: &str) -> Result<()> {
    validate_endpoint(&m.apk_url)?;
    ensure!(
        semver::Version::parse(&m.version)? > semver::Version::parse(current)?,
        "Already up to date"
    );
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
pub fn check(url: &str, key: &str, current: &str) -> Result<Manifest> {
    validate_endpoint(url)?;
    let response = client()?.get(url).send()?.error_for_status()?;
    let mut data = vec![];
    response.take(65537).read_to_end(&mut data)?;
    ensure!(data.len() <= 65536, "Release manifest is too large");
    let m: Manifest = serde_json::from_slice(&data)?;
    verify(&m, key, current)?;
    Ok(m)
}
pub fn download(m: &Manifest, key: &str, current: &str, directory: &Path) -> Result<PathBuf> {
    verify(m, key, current)?;
    std::fs::create_dir_all(directory)?;
    let partial = directory.join("update.apk.part");
    let output = directory.join("update.apk");
    let result = (|| -> Result<()> {
        let mut response = client()?.get(&m.apk_url).send()?.error_for_status()?;
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
