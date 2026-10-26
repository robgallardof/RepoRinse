# ADR 0001 — Rust for the CLI and engine

- **Status:** accepted
- **Date:** 2026-10-26

## Context

RepoRinse runs inside Git hooks (`commit-msg`, `pre-commit`) and agent hooks that fire on every tool call.
It must start fast, run offline, ship as one file per platform and never require a language runtime on the
user's machine. Windows 11 / PowerShell is a first-class target, together with Linux and macOS.

## Options considered

| Option | Single binary, no runtime | Hook start-up | Cross-platform CI | Notes |
|---|---|---|---|---|
| **Rust** | yes (static, ~MBs) | ~ms | `cargo build` per OS in GitHub Actions | strong text/Unicode handling, `globset`, `toml`, `serde` |
| Go | yes | ~ms | trivial cross-compilation | equally valid; not installed on the maintainer's machine |
| .NET Native AOT | yes | ~10 ms | needs native toolchain per OS | a C# skeleton was started and dropped in favour of Rust |
| Python | no (or PyInstaller ≈ 10 MB, slow start) | 100–300 ms | easy | runtime on every machine or heavy bundles |
| Node | no (or SEA bundles) | 50–100 ms | easy | same runtime problem |

## Decision

Rust (edition 2024, stable toolchain), one crate with a library (`reporinse`) and one binary (`reporinse`).

- Runtime dependencies are compiled in: `clap` (CLI), `serde`/`serde_json` (JSON I/O, agent payloads),
  `toml` (configuration), `globset` (exclusions), `sha2` (manifests, fix preconditions), `similar` (diff previews),
  `anyhow`. No network, async runtime, database or telemetry crates.
- Configuration format: **TOML** (`.reporinse.toml`). It is the convention of Rust tooling, has one unambiguous
  grammar (no YAML "Norway problem"), supports comments and maps cleanly to a versioned schema.
- Hooks installed into repositories are small POSIX `sh` shims that call the binary; there is no PowerShell
  hook because Git for Windows runs hooks through its bundled `sh`.

## Consequences

- Users download one executable per platform (x86_64 Windows/Linux/macOS, aarch64 macOS/Linux when CI covers it)
  or build with `cargo build --release`.
- Contributors need the Rust stable toolchain; `cargo test` runs everything without network access.
- MSVC build tools are required to build on Windows (already standard for Rust on Windows).
