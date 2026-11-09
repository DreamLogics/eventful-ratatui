//! Compile-time diagnostics of `#[key_events]`.
#[test]
fn invalid_macro_input_is_rejected() {
    trybuild::TestCases::new().compile_fail("tests/ui/*.rs");
}

#[test]
fn valid_macro_input_compiles_and_runs() {
    trybuild::TestCases::new().pass("tests/ui-pass/*.rs");
}
