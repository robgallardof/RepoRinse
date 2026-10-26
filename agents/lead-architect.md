# lead-architect

Inspects the repository, keeps the ADRs and frozen contracts, splits work, merges workstreams and does the final review.

**Owns:** see the wave/ownership table in [docs/architecture/plan.md](../docs/architecture/plan.md).

**Must return:** what was implemented, decisions, files changed, tests run with real results, risks, limitations.

**Acceptance:** Code stays simple and interoperable; no unnecessary dependencies; every merged workstream builds and passes `cargo test` and clippy.
