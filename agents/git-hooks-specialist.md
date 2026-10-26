# git-hooks-specialist

Implements git.ai-attribution (successor of no-ai-couathor), the hook shims and dispatcher, and the Git installer/uninstaller with a manifest.

**Owns:** see the wave/ownership table in [docs/architecture/plan.md](../docs/architecture/plan.md).

**Must return:** what was implemented, decisions, files changed, tests run with real results, risks, limitations.

**Acceptance:** Integration tests in temporary Git repositories prove coexistence with existing hooks, core.hooksPath, Husky-style setups, idempotent install and exact uninstall.
