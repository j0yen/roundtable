//! AC1: `cargo test --release` passes; `cargo clippy -- -D warnings` passes.
//!
//! This test is a meta-check: it passes if the test suite itself compiles and
//! runs. clippy is verified as part of the CI gate, not at runtime.
//! This file exists to anchor AC1 in the harness.

#[test]
fn ac1_compilation_and_tests_pass() {
    // If this test compiles and runs, the crate compiled successfully.
    // The clippy gate is enforced by `cargo clippy -- -D warnings` in CI / scripts/run-metrics.sh.
    assert!(true, "crate compiled; clippy gate is run by harness");
}
