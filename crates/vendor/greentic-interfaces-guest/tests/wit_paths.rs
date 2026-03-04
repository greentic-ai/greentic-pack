#![allow(dead_code)]

#[path = "../build_support/wit_paths.rs"]
mod wit_paths;

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn prefers_exact_versioned_sibling_wit_root() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time went backwards")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("gi-guest-wit-paths-{unique}"));

    fs::create_dir_all(&root).expect("create temp root");
    let old = root.join("greentic-interfaces-0.4.88/wit");
    let exact = root.join("greentic-interfaces-0.4.96/wit");
    let manifest_dir = root.join("greentic-interfaces-guest-0.4.96");

    fs::create_dir_all(&old).expect("create old sibling wit");
    fs::create_dir_all(&exact).expect("create exact sibling wit");
    fs::create_dir_all(&manifest_dir).expect("create guest manifest dir");

    let selected = wit_paths::crates_io_sibling_wit_root(&manifest_dir, "0.4.96")
        .expect("expected a sibling wit root");

    let expected = fs::canonicalize(PathBuf::from(&exact)).expect("canonical exact path");
    assert_eq!(selected, expected);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn prefers_exact_versioned_candidate_even_when_listed_later() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time went backwards")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("gi-guest-wit-select-{unique}"));

    let old = root.join("greentic-interfaces-0.4.88/wit");
    let exact = root.join("greentic-interfaces-0.4.96/wit");
    fs::create_dir_all(&old).expect("create old sibling wit");
    fs::create_dir_all(&exact).expect("create exact sibling wit");

    let selected = wit_paths::choose_sibling_wit_root(
        vec![old, exact.clone()],
        "greentic-interfaces-",
        "0.4.96",
    )
    .expect("expected selected candidate");

    let expected = fs::canonicalize(PathBuf::from(&exact)).expect("canonical exact path");
    assert_eq!(selected, expected);

    let _ = fs::remove_dir_all(&root);
}
