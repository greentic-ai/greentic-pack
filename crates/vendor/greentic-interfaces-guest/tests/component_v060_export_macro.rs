#![cfg(feature = "component-v0-6")]

#[test]
fn component_v060_export_macro_compiles() {
    eprintln!("running trybuild compile-pass: tests/trybuild/component_v060_export.rs");
    let t = trybuild::TestCases::new();
    t.pass("tests/trybuild/component_v060_export.rs");
    eprintln!("trybuild compile-pass completed");
}
