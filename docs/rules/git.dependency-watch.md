# git.dependency-watch

Dependency manifest review.

Default warn. Reports changed Cargo, npm/Yarn/pnpm, Python, Go, Ruby, Composer manifests/lockfiles and .csproj files. This is path-based review; it does not identify package deltas, vulnerabilities or registry validity. No network or fix.

```toml
[rules."git.dependency-watch"]
mode = "warn"

```

Supports off/warn/block; no automatic fix. Per-rule `exclude` globs are available for file/change-set targets.

## Español

Informa manifiestos y lockfiles modificados por ruta. No determina diferencias de paquetes, vulnerabilidades ni validez en registros. No usa red ni modifica archivos.
Admite off/warn/block, sin corrección automática.
