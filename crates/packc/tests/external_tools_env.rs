use packc::external_tools::resolve;

#[test]
fn resolve_honours_translator_env_override() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    // SAFETY: integration test, single-threaded within this test binary;
    // the var is removed before the assertion runs.
    unsafe {
        std::env::set_var("GREENTIC_I18N_TRANSLATOR_BIN", tmp.path());
    }
    let got = resolve("greentic-i18n-translator");
    unsafe {
        std::env::remove_var("GREENTIC_I18N_TRANSLATOR_BIN");
    }
    assert_eq!(got.as_deref(), Some(tmp.path()));
}
