//! Compile-fail tests for `#[derive(Model)]` rules. Refresh with `TRYBUILD=overwrite`.

#[test]
fn derive_rules_are_enforced_at_compile_time() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/*.rs");
}
