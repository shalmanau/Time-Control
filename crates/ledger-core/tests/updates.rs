use ed25519_dalek::{Signer, SigningKey};
use ledger_core::updates::{validate_endpoint, verify, Manifest};
fn signed() -> (Manifest, String) {
    let k = SigningKey::from_bytes(&[42; 32]);
    let mut m = Manifest {
        version: "0.2.0".into(),
        version_code: 2,
        apk_url: "https://example.org/time.apk".into(),
        sha256: "ab".repeat(32),
        signature: String::new(),
    };
    m.signature = hex::encode(
        k.sign(
            format!(
                "{}\n{}\n{}\n{}",
                m.version, m.version_code, m.apk_url, m.sha256
            )
            .as_bytes(),
        )
        .to_bytes(),
    );
    (m, hex::encode(k.verifying_key().to_bytes()))
}
#[test]
fn signed_new_release_is_accepted() {
    let (m, k) = signed();
    verify(&m, &k, "0.1.0").unwrap();
}
#[test]
fn tampering_and_rollback_are_rejected() {
    let (mut m, k) = signed();
    assert!(verify(&m, &k, "0.3.0").is_err());
    m.apk_url = "https://evil.example/x.apk".into();
    assert!(verify(&m, &k, "0.1.0").is_err());
}
#[test]
fn insecure_sources_are_rejected() {
    assert!(validate_endpoint("http://example.org").is_err());
    assert!(validate_endpoint("https://user:secret@example.org").is_err());
    assert!(validate_endpoint("file:///tmp/a").is_err());
}

#[test]
fn signature_validation_does_not_trust_an_old_manifest() {
    let (mut m, k) = signed();
    ledger_core::updates::verify_signature(&m, &k).unwrap();
    m.version = "0.0.1".into();
    assert!(ledger_core::updates::verify_signature(&m, &k).is_err());
}
