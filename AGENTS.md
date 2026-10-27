# AGENTS.md — working on RepoRinse

Guidelines for any coding agent (or human) changing **this** repository.

## What RepoRinse is

A local, deterministic CLI that keeps repositories clean (AI attribution trailers, whitespace noise,
invisible Unicode, agent artifacts) and offers optional guardrails for coding agents. It is **not** an AI
detector and never calls an AI, network service or telemetry endpoint at runtime.

## Map

| Path | Purpose |
|---|---|
| `src/core/` | contracts (`model.rs`, `rule.rs`, `integration.rs`), config, engine, fix planning, filesystem safety |
| `src/modules/<category>/` | rules; one file per rule, listed in that folder's `mod.rs` |
| `src/integrations/<provider>/` | Git hooks and agent adapters + installers |
| `src/cli/` | `reporinse` command line; one file per command in `commands/` |
| `tests/` | integration and end-to-end tests (unit tests live next to the code) |
| `docs/` | rules, integrations, configuration, architecture, ADRs, research |
| `agents/` | role descriptions used to build RepoRinse with sub-agents |

Ownership and frozen contracts: [docs/architecture/plan.md](docs/architecture/plan.md).

## Commands

```sh
cargo build                         # debug build
cargo test                          # unit + integration tests (no network, no global git config)
cargo clippy --all-targets -- -D warnings
cargo fmt --check
cargo run -- modules list           # run the CLI
```

## Rules for changes

1. **Detectors are pure.** A rule's `analyze` reads its `RuleContext` and returns findings and fix plans. No I/O.
2. **Safe by default.** Report before modifying. A fix must be deterministic, idempotent (`fix(fix(x)) == fix(x)`),
   preserve encoding/BOM/newlines and be refused if the file changed since analysis.
3. **Never** run `git add -A`, rewrite history, touch global Git config outside an isolated test `HOME`, override an
   existing `core.hooksPath`, or delete files the manifest does not own.
4. **Tests touch only temp dirs.** Use `tempfile`, set `HOME`, `XDG_CONFIG_HOME`, `GIT_CONFIG_GLOBAL`,
   `GIT_CONFIG_NOSYSTEM=1` for any test that runs `git`.
5. **No invented integrations.** A provider/event is supported only when its official documentation was checked
   (`docs/research/compatibility-matrix.md`) and a contract test covers it.
6. **No over-engineering.** No database, server, async runtime, plugin system or MCP server.
7. **Docs follow code.** README and docs only describe commands that run in the current build.
8. Commit messages: detailed, no AI attribution trailers (RepoRinse's own `git.ai-attribution` would remove them).

## Commit language

All new Git commit subjects and bodies must be written in English, including merge
and squash messages. Keep the repository's existing message conventions.
This applies to all contributors and coding agents, and only to new commits.
Do not rewrite existing history to translate commit messages.
