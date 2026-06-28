use sha2::{Digest, Sha256};

#[test]
fn publish_metadata_pins_artifact_sha() {
    let pack_bytes = b"fake-gtpack-bytes";
    let meta = packc::store_client::publish_metadata(
        pack_bytes,
        "alice.greeter-bot",
        "Greeter Bot",
        "0.1.0",
        "A bot",
    )
    .unwrap();
    let expected = hex::encode(Sha256::digest(pack_bytes));
    assert_eq!(meta["artifactSha256"], expected);
    assert_eq!(meta["describe"]["manifestSha256"], expected); // same pin
    assert_eq!(meta["describe"]["kind"], "AgenticWorker");
}
