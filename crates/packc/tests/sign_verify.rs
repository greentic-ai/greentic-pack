#![forbid(unsafe_code)]

use std::fs;
use std::path::Path;

use ed25519_dalek::pkcs8::{EncodePrivateKey, EncodePublicKey};
use ed25519_dalek::SigningKey;
use packc::{manifest, sign_pack_dir, verify_pack_dir, VerifyOptions};
use pkcs8::LineEnding;
use rand::rngs::OsRng;
use tempfile::tempdir;

fn write_file(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent");
    }
    fs::write(path, contents).expect("write file");
}

#[test]
fn sign_and_verify_pack_manifest() {
    let temp = tempdir().expect("temp dir");
    let pack_dir = temp.path();

    const PACK_TOML: &str = "[package]\nname = \"demo\"\n\n[metadata]\ndescription = \"demo\"\n";
    write_file(&pack_dir.join("pack.toml"), PACK_TOML);

    write_file(&pack_dir.join("flows/main.flow"), "start: node");

    let mut rng = OsRng;
    let signing_key = SigningKey::generate(&mut rng);

    let private_pem = signing_key
        .to_pkcs8_pem(LineEnding::LF)
        .expect("encode private key");
    let public_pem = signing_key
        .verifying_key()
        .to_public_key_pem(LineEnding::LF)
        .expect("encode public key");

    let first_signature = sign_pack_dir(pack_dir, private_pem.as_str(), None).expect("sign pack");
    assert_eq!(first_signature.alg, "ed25519");
    assert!(first_signature.digest.starts_with("sha256:"));
    assert!(!first_signature.sig.is_empty());

    // Ensure the manifest now contains the signature block.
    let manifest_signature = manifest::read_signature(pack_dir)
        .expect("read signature")
        .expect("signature present");
    assert_eq!(manifest_signature.sig, first_signature.sig);

    let verified = verify_pack_dir(
        pack_dir,
        VerifyOptions {
            public_key_pem: Some(public_pem.as_str()),
            allow_unsigned: false,
        },
    )
    .expect("verify signature");
    assert_eq!(verified.sig, first_signature.sig);

    // Re-sign without modifying the pack – the signature should be deterministic.
    let second_signature =
        sign_pack_dir(pack_dir, private_pem.as_str(), None).expect("re-sign pack");
    assert_eq!(second_signature.sig, first_signature.sig);
    assert_eq!(second_signature.digest, first_signature.digest);
}
