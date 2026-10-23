//! Property tests of the domains' laws (`cargo test`). Each file is a child module of the one
//! it checks (`layered.rs`: `tries.rs`; `domain.rs`: `values.rs`, `rows.rs`), so it sees its
//! private parts; this module holds what they share.
//!
//! `PROPTEST_CASES=N` runs N cases per property (default 64: about a second in all).

use proptest::test_runner::Config;

/// Cases per property unless `PROPTEST_CASES` says otherwise; shrinking long enough to reach a
/// minimal program of operations (their indices shrink one at a time).
pub(crate) fn config() -> Config {
    let cases = std::env::var("PROPTEST_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(64);
    Config { cases, failure_persistence: None, max_shrink_iters: 20_000, ..Config::default() }
}
