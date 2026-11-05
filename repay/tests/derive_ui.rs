//! Compile-fail checks: misuse of the derive must give a clear, well-placed error.
//!
//! The expected errors follow one rustc's wording, so this only runs in the
//! repository, where `.cargo/config.toml` sets `REPAY_UI_TESTS`. From the published
//! crate it skips.

#[test]
fn derive_errors() {
    if std::env::var_os("REPAY_UI_TESTS").is_none() {
        eprintln!("skipping: compile-error snapshots run only in the repository");
        return;
    }
    trybuild::TestCases::new().compile_fail("tests/ui/*.rs");
}
