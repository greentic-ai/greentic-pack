#![forbid(unsafe_code)]

use std::fs;
use std::path::Path;

use ed25519_dalek::SigningKey;
use ed25519_dalek::pkcs8::{EncodePrivateKey, EncodePublicKey};
use packc::{VerificationError, VerifyOptions, manifest, sign_pack_dir, verify_pack_dir};
use pkcs8::LineEnding;
use tempfile::tempdir;
use toml::Value;

fn write_file(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent");
    }
    fs::write(path, contents).expect("write file");
}

const TEST_SECRET_KEY: [u8; 32] = [0x42; 32];

#[test]
fn tampering_is_detected() {
    let temp = tempdir().expect("temp dir");
    let pack_dir = temp.path();

    const PACK_TOML: &str = "[package]\nname = \"demo\"\n\n[metadata]\ndescription = \"demo\"\n";
    write_file(&pack_dir.join("pack.toml"), PACK_TOML);

    write_file(&pack_dir.join("flows/main.flow"), "start: node");
    write_file(&pack_dir.join(".git/config"), "dummy");
    write_file(&pack_dir.join("target/cache.bin"), "cache");
    write_file(&pack_dir.join("ignored/secret.txt"), "secret");
    write_file(&pack_dir.join(".packignore"), "ignored/**\n");

    let signing_key = SigningKey::from_bytes(&TEST_SECRET_KEY);

    let private_pem = signing_key
        .to_pkcs8_pem(LineEnding::LF)
        .expect("encode private key");
    let public_pem = signing_key
        .verifying_key()
        .to_public_key_pem(LineEnding::LF)
        .expect("encode public key");

    sign_pack_dir(pack_dir, private_pem.as_str(), None).expect("sign pack");

    // Baseline verification succeeds.
    verify_pack_dir(
        pack_dir,
        VerifyOptions {
            public_key_pem: Some(public_pem.as_str()),
            allow_unsigned: false,
        },
    )
    .expect("verify baseline");

    // Changes under .git/ and target/ do not affect verification.
    write_file(&pack_dir.join(".git/config"), "modified");
    write_file(&pack_dir.join("target/cache.bin"), "changed");

    verify_pack_dir(
        pack_dir,
        VerifyOptions {
            public_key_pem: Some(public_pem.as_str()),
            allow_unsigned: false,
        },
    )
    .expect("verify after ignored dirs change");

    // Updates to .packignore-ignored files should not affect verification either.
    write_file(&pack_dir.join("ignored/secret.txt"), "changed");

    verify_pack_dir(
        pack_dir,
        VerifyOptions {
            public_key_pem: Some(public_pem.as_str()),
            allow_unsigned: false,
        },
    )
    .expect("verify after packignore change");

    // Tamper with tracked flow.
    write_file(&pack_dir.join("flows/main.flow"), "start: node\nmodified");

    let err = verify_pack_dir(
        pack_dir,
        VerifyOptions {
            public_key_pem: Some(public_pem.as_str()),
            allow_unsigned: false,
        },
    )
    .expect_err("digest mismatch expected");

    let verify_err = err
        .downcast::<VerificationError>()
        .expect("verification error");
    assert!(matches!(
        verify_err,
        VerificationError::DigestMismatch { .. }
    ));

    // Restore original flow.
    write_file(&pack_dir.join("flows/main.flow"), "start: node");

    // Corrupt the signature value directly.
    corrupt_signature(pack_dir, |sig| sig.sig = "A".repeat(sig.sig.len()));

    let err = verify_pack_dir(
        pack_dir,
        VerifyOptions {
            public_key_pem: Some(public_pem.as_str()),
            allow_unsigned: false,
        },
    )
    .expect_err("invalid signature expected");

    let verify_err = err
        .downcast::<VerificationError>()
        .expect("verification error");
    assert!(matches!(
        verify_err,
        VerificationError::InvalidSignature { .. }
    ));

    // Remove the signature block entirely.
    remove_signature(pack_dir);

    let err = verify_pack_dir(
        pack_dir,
        VerifyOptions {
            public_key_pem: Some(public_pem.as_str()),
            allow_unsigned: false,
        },
    )
    .expect_err("missing signature expected");

    let verify_err = err
        .downcast::<VerificationError>()
        .expect("verification error");
    assert!(matches!(verify_err, VerificationError::MissingSignature));

    let synthetic = verify_pack_dir(
        pack_dir,
        VerifyOptions {
            public_key_pem: None,
            allow_unsigned: true,
        },
    )
    .expect("unsigned allowed");
    assert_eq!(synthetic.alg, "none");
    assert!(synthetic.sig.is_empty());
}

fn corrupt_signature<F>(pack_dir: &Path, mut mutate: F)
where
    F: FnMut(&mut manifest::PackSignature),
{
    let manifest_path = manifest::manifest_path(pack_dir).expect("manifest path");
    let mut doc = read_manifest(&manifest_path);

    let mut signature = manifest::read_signature(pack_dir)
        .expect("read signature")
        .expect("signature exists");

    mutate(&mut signature);
    set_signature_value(&mut doc, &signature);

    fs::write(
        &manifest_path,
        toml::to_string_pretty(&doc).expect("serialize"),
    )
    .expect("write manifest");
}

fn remove_signature(pack_dir: &Path) {
    let manifest_path = manifest::manifest_path(pack_dir).expect("manifest path");
    let mut doc = read_manifest(&manifest_path);

    if let Some(table) = doc.as_table_mut()
        && let Some(greentic) = table.get_mut("greentic")
        && let Some(section) = greentic.as_table_mut()
    {
        section.remove("signature");
        if section.is_empty() {
            table.remove("greentic");
        }
    }

    fs::write(
        &manifest_path,
        toml::to_string_pretty(&doc).expect("serialize"),
    )
    .expect("write manifest");
}

fn set_signature_value(doc: &mut Value, signature: &manifest::PackSignature) {
    let table = doc.as_table_mut().expect("table");
    let greentic = table
        .entry("greentic".to_string())
        .or_insert_with(|| Value::Table(toml::map::Map::new()));
    let greentic_table = greentic.as_table_mut().expect("greentic table");
    let value = Value::try_from(signature.clone()).expect("serialize signature");
    greentic_table.insert("signature".to_string(), value);
}

fn read_manifest(path: &Path) -> Value {
    let source = fs::read_to_string(path).expect("read manifest");
    let table: toml::value::Table = toml::from_str(&source).expect("parse manifest");
    Value::Table(table)
}
