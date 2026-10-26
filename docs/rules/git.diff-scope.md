# git.diff-scope

Change-set size review.

Default warn. Reports when staged path count exceeds max_files OR added+deleted lines exceed max_lines. Both thresholds must be positive integers. Counts content changes, not intent. No fix. Exclusions apply before counting.

```toml
[rules."git.diff-scope"]
mode = "warn"
max_files = 25
max_lines = 500
```

Supports off/warn/block; no automatic fix. Per-rule `exclude` globs are available for file/change-set targets.

## Español

Informa cambios que superan el máximo de archivos o de líneas añadidas/eliminadas. Ambos límites son enteros positivos. No juzga la intención ni modifica archivos.
Admite off/warn/block, sin corrección automática.
