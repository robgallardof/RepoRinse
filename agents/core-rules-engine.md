# core-rules-engine

Implements configuration loading, presets, precedence, the engine, safe file I/O and the P0 text/docs rules with their commands (check, fix, init, config, rule explain).

**Owns:** see the wave/ownership table in [docs/architecture/plan.md](../docs/architecture/plan.md).

**Must return:** what was implemented, decisions, files changed, tests run with real results, risks, limitations.

**Acceptance:** Unit tests and adversarial fixtures for each rule; fixes are idempotent and never change significant content.
