# RepoRinse

Keep the code. Rinse the noise. [Español](README.es.md)

A local Rust CLI for cleaning known agent attribution, checking repository text and applying explicit agent policies.
No AI calls, network services or telemetry at runtime.

## Build and start

Requires Rust **1.88+** and Git. Python 3 is needed only for packaging.

```sh
cargo build --release --locked
./target/release/reporinse init
./target/release/reporinse check --staged
```

On Windows use `target\release\reporinse.exe`. Keep the binary at a stable path: installers register its absolute location.
`python scripts/package.py` creates an offline archive and SHA-256 checksum in `dist/`.
Extract it and use its PowerShell 7/shell installer, or copy the executable into a directory on PATH.

```sh
reporinse config validate
reporinse check --files src/main.rs
reporinse check --staged
reporinse status
reporinse doctor
```

`check` never writes. Choose one input: `--files`, `--message`, or `--staged`.
Exit codes: warnings/success 0, blocking findings 1, usage 2, configuration 3, conflicts 4.
Use `--json` for structured output and `--quiet` to suppress nonblocking findings.

## Available rules

| Rule | Default | Behavior |
|---|---|---|
| git.ai-attribution | fix | Removes known agent trailers and footers; keeps human co-authors |
| text.whitespace | warn | Trailing spaces/tabs; preserves Markdown hard breaks |
| text.invisible-unicode | warn | Reports selected invisible/bidi controls without deletion |
| docs.agent-artifacts | warn | Reviews newly staged plan/summary Markdown outside docs roots |
| git.diff-scope | warn | Reviews changes above 25 files or 500 lines; configurable |
| git.dependency-watch | warn | Reviews changed dependency manifests and lockfiles |
| code.duplicate-comments | off | Opt-in review of consecutive identical line comments |
| agent.command-guard | warn | Configured command globs; empty policy matches nothing |
| agent.path-scope | warn | Configured path globs; empty policy matches nothing |

Use `reporinse rule explain <id>` for bundled documentation and [example configuration](examples/.reporinse.toml).

## Fix text

Enable a fix explicitly in `.reporinse.toml`:

```toml
[rules."text.whitespace"]
mode = "fix"
honor_editorconfig = true
```

```sh
reporinse fix --files src/main.rs --dry-run
reporinse fix --files src/main.rs
reporinse fix --staged --dry-run
reporinse fix --staged
```

Staged operations inspect added/modified lines; explicit files use their whole contents.
EditorConfig `trim_trailing_whitespace=false` suppresses whitespace cleanup; staged operations read staged EditorConfig.
BOM, line endings and final-newline state are preserved. Staged fixes refuse partial staging and checkout encoding
differences, lock the index and use a temporary index. Git hooks never modify staged files.
Binary/unsupported files, files above 8 MiB, symlinks/reparse points, external paths and stale inputs are refused.
Avoid concurrent editing: ordinary editors do not honor Git's index lock.

## Hooks

```sh
reporinse install --provider git --all --dry-run
reporinse install --provider git --all --yes
reporinse install --provider claude,codex,gemini --all --yes
reporinse uninstall --provider git,claude,codex,gemini --all --yes
```

`--provider cursor` is also supported. Scope: `repo` (default) or `user`. Selection: `--only`, `--except`, `--interactive`.
Multiple providers are preflighted before writes; each provider rolls back its own failed writes.
Existing Git hooks run first. Agent settings are merged. Manifests restore original bytes and refuse drift on uninstall.
Inspect changed settings before considering `--force`.

| Provider | Installed events | Setup |
|---|---|---|
| Git | commit-msg, pre-commit | Git available |
| Claude | PreToolUse, PostToolUse | Reload settings/session if needed |
| Codex | PreToolUse, PostToolUse | Review each hook in /hooks; trust project hooks |
| Gemini | BeforeTool, AfterTool | Reload settings/session if needed |
| Cursor | beforeShellExecution, beforeReadFile, afterFileEdit | Trusted workspace |
| Antigravity | Git only | Install --provider git; no native-hook claim |

Cursor may also load Claude hooks: choose one for overlapping rules to avoid duplicate execution.
Adapters never autoapprove Claude/Codex actions, retry turns or hide results. Blocks apply only before actions.
Invalid input/configuration fails open with a sanitized diagnostic. Guard globs are explicit string policies,
not a shell parser or replacement for host approvals/sandboxing. Codex patch paths and undocumented Cursor Write formats
are not guessed. See [integration docs](docs/integrations/claude.md).

## Verify

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
python scripts/package.py
```

Tests isolate Git and provider settings in temporary directories. CI targets Windows, Linux and macOS.
[Verification and limits](docs/development/verification.md) distinguishes local execution from configured CI and live hosts.
See [configuration](docs/configuration/README.md), [security](SECURITY.md), and [changelog](CHANGELOG.md).
