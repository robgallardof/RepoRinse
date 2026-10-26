# Contributing

Use stable Rust and Git. Run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`
and `cargo test --locked`. Python 3 is only required for release packaging.

Detectors are pure; filesystem and provider operations belong in the orchestration layer.
Every rule needs positive, negative and exclusion tests. Every fixer needs idempotency and stale-input tests.
Provider claims must link official documentation and have protocol contract tests.
Do not change global settings or use real repositories in tests. See [AGENTS.md](AGENTS.md).

Keep changes focused, describe observable behavior and test results, and omit AI attribution trailers.
