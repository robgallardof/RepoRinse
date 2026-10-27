# Delivery verification — 2026-10-26

Locally executed on Windows: unit/integration tests, real temporary Git repositories, subprocess hook invocations,
generated handler execution for all four agent providers, formatting and Clippy. Provider settings and Git globals
are isolated in temporary homes. Release build and archive smoke tests are recorded in delivery-test-results.json in the source repository. Machine-specific verification logs are not bundled in release archives.

Covered: staged versus working-tree contents, partial staging refusal, foreign index lock preservation,
changed-line cleanup, EditorConfig precedence and staged snapshot, BOM/CRLF/idempotency, stale write refusal,
foreign Git hook chaining, hook migration, installer drift checks and exact restoration, malformed/deep/oversized
protocol input, no autoapproval, no post-action block/retry, message bounds and command/source non-disclosure.

CI is configured for Windows, Linux and macOS. Linux/macOS CI was not executed locally.
The declared MSRV is 1.88 because locked globset and sha2 dependencies require it; the local compiler is newer.
No live sessions inside Claude, Codex, Gemini or Cursor were claimed. Hook trust, host timeout/crash semantics
and undocumented tool payloads still need testing in the actual host applications. See compatibility-matrix.md.

Explicit boundaries: Antigravity native hooks are unavailable; guard globs are bypassable string policies;
EditorConfig resolution is repository-local and only honors trim_trailing_whitespace; no numeric-range glob
expansion is promised. File replacements are not locked against ordinary editors. Multiple providers preflight
together but only roll back their own operations. Code comment heuristics are opt-in and do not parse syntax.
P2 natural-language test-claim verification and AI detection are not shipped.

Docker (optional image), executed locally on Docker 29.8 (Windows host, Linux containers):
`python scripts/test_docker.py` built the image and passed non-root user, `check --files` JSON,
`check --staged` on a bind mount owned by another UID, `fix --message` writing back to the host file and the
Compose service with networking disabled. `install`/`uninstall` are intentionally not supported in containers.

## Temporary home paths in CI — 2026-10-26

The macOS job exposed an existing test-fixture issue: its temporary HOME used the
system `/var` alias, so the installer correctly refused a symlinked settings path.
Unix fixtures now canonicalize only their isolated test home before passing it to
children. Production link rejection and the configured real user home are unchanged.

Executed: Windows formatting, clippy and the full test suite passed; both adapter
installation tests also passed in a Linux container with TMPDIR deliberately routed
through a symlink. Docker uses a read-only source mount and a disposable build directory.
The real macOS job is verified through GitHub Actions rather than claimed as a local run.

## Español

Las verificaciones locales se ejecutan en Windows. CI de Linux/macOS está configurado, no ejecutado aquí.
Las pruebas de protocolo y procesos no sustituyen una sesión real dentro de cada aplicación de agente.
Consulta los resultados concretos en delivery-test-results.json y los límites descritos arriba.
