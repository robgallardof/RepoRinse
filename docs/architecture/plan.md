# Implementation plan and ownership

Working spec: the *Prompt Maestro — RepoRinse* (2026-10-26). This file is the coordination contract between
the workstreams that build RepoRinse. The lead architect merges every workstream; nobody edits files owned by
another workstream without going through the lead.

## Frozen contracts (change only through the lead)

| File | What is frozen |
|---|---|
| `src/core/model.rs` | Rule metadata, findings, fix plans, documents, change sets, normalized events, exit codes |
| `src/core/rule.rs` | `Rule` trait, `RuleSettings`, `RuleContext` |
| `src/core/integration.rs` | `Adapter`, `Installer`, install request/result/status types |
| `src/core/planning.rs` | `hash`, `apply`, `apply_all` |
| `src/core/config.rs` | public API listed in its header (implementation is owned by core-rules-engine) |
| `src/core/engine.rs` | public API listed in its header |
| `src/cli/mod.rs` | command table; a workstream may add flags only inside its own `commands/*.rs` |
| `src/modules/mod.rs`, `src/integrations/mod.rs` | registries; each category/provider fills its own folder |

Adding a *new* public type is fine inside your own files. Changing a frozen signature: describe it in your
report; the lead applies it.

## Workstreams

| Wave | Workstream | Owns |
|---|---|---|
| 0 | lead-architect | contracts, ADRs, this plan, merges, final review |
| 1 | research-compatibility + security | `docs/research/**`, `docs/security/**` |
| 1 | core-rules-engine | `src/core/{config,engine,safety}.rs` + new `src/core/*` files, `src/modules/{text,docs}/**`, `src/cli/commands/{check,fix,init,config,rule}.rs`, `tests/core_*.rs`, `tests/text_*.rs`, `tests/docs_*.rs`, `tests/fixtures/{text,docs}/**`, `docs/rules/{text,docs}.*.md`, `docs/configuration/**` |
| 1 | git-hooks-specialist | `src/modules/git/**`, `src/integrations/git/**`, `src/cli/commands/{hook,install}.rs`, `tests/git_*.rs`, `tests/support/**`, `tests/fixtures/git/**`, `docs/rules/git.*.md`, `docs/integrations/git.md`, `docs/migration/**` |
| 2 | agent-adapters-specialist | `src/integrations/{claude,codex,gemini,cursor,antigravity}/**`, `tests/adapter_*.rs`, `tests/fixtures/agents/**`, `docs/integrations/{claude,codex,gemini,cursor,antigravity}.md` |
| 2 | cli-distribution-specialist | `src/cli/commands/{status,doctor}.rs`, `src/cli/output.rs`, `scripts/**`, `installers/**`, `.github/workflows/**`, `tests/cli_*.rs`, `tests/e2e_*.rs` |
| 2 | rules-p1 | `src/modules/code/**`, P1 rules in `text`/`git` coordinated with the lead, their tests and docs |
| 3 | security-reviewer + test-automation | `tests/security_*.rs`, `tests/regression_*.rs`, `tests/fixtures/adversarial/**`, `benches/**` |
| 3 | documentation-ux | `README.md`, `README.es.md`, `assets/**`, `docs/{getting-started,architecture/overview.md,development}/**`, `examples/**`, `skills/**`, `AGENTS.md`, `CLAUDE.md` |
| 3 | maintainer-release | `CHANGELOG.md`, `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `SECURITY.md`, `.github/ISSUE_TEMPLATE/**`, `.github/PULL_REQUEST_TEMPLATE.md`, release workflow dry-run |

## Configuration schema v1 (target; core-rules-engine implements and documents it)

```toml
version = 1
preset = "balanced"            # minimal | balanced | strict | custom
exclude = ["**/node_modules/**", "**/target/**", "**/*.min.js"]

[providers]
git = true
claude = false

[rules."git.ai-attribution"]
mode = "fix"                   # off | warn | block | fix
policy = "remove-known-ai-only"

[rules."text.whitespace"]
mode = "warn"
honor_editorconfig = true
exclude = ["docs/legacy/**"]
```

Locations and precedence: CLI > repo (`<root>/.reporinse.toml`) > user (`$REPORINSE_CONFIG_HOME`, else
`$XDG_CONFIG_HOME/reporinse/config.toml`, else `~/.config/reporinse/config.toml`; Windows `%APPDATA%\reporinse\config.toml`)
> preset > built-ins. Unknown keys are errors (`config validate`), not silently ignored.

## Hook entry points (git-hooks-specialist implements)

Installed hook shims call `reporinse hook git <hook-name> [args…]`. Agent hooks call
`reporinse hook <provider> <event>` with the host payload on stdin. The `hook` command must never prompt.

## Priorities

P0 → P1 → P2 as in the spec §3. A rule is *done* only with metadata, config, check/fix behaviour, docs EN/ES
section, positive/negative/idempotency tests and a false-positive strategy. Everything else stays `planned` and is
not advertised in the README.


## Delivery status — 2026-10-26

Core/config/check/fix, staged corrections, P0 text review, Git integration, four native agent adapters,
explicit command/path guards, P1 change review, status/doctor, offline packaging and CI are implemented.
This delivery uses code.duplicate-comments and git.dependency-watch as conservative path/lexical checks;
it does not advertise semantic comment quality or package-delta analysis. Frozen public signatures remain unchanged.
New orchestration modules: core/editorconfig, integrations/git/staged_fix, agent_adapter and agent_install.
Native Antigravity and P2 test-claim evidence checks are excluded pending host/spec verification.
See docs/development/verification.md for actual test coverage and limits.
