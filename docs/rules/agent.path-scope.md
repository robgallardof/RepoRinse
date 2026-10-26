# agent.path-scope

Explicit path glob policy.

Matches reported paths and lexical normalized repository-relative forms. Default warn, empty deny list. Operates only on paths the host reports; Codex patch paths and undocumented Cursor Write formats are not inferred. Paths are not resolved on disk.

```toml
[rules."agent.path-scope"]
mode = "warn"
deny = [".git/**", "**/.git/**"]
```

Supports off/warn/block; no automatic fix. Per-rule `exclude` globs are available for file/change-set targets.

## Español

Compara rutas informadas por el proveedor y su forma relativa normalizada. No resuelve enlaces en disco ni adivina rutas que el host no entrega.
Admite off/warn/block, sin corrección automática.
