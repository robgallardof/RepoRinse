# code.duplicate-comments

Consecutive duplicate comment review.

Default off. Supports warn/block only. Reports consecutive identical trimmed // or # comment lines in supported source extensions. It is a lexical heuristic, not a language parser, and may match multiline string contents. Explicit file checks inspect whole files; staged checks inspect changed lines. Exclude fixtures and intentional repetition. Never deletes comments.

```toml
[rules."code.duplicate-comments"]
mode = "off"

```

Supports off/warn/block; no automatic fix. Per-rule `exclude` globs are available for file/change-set targets.

## Español

Desactivada inicialmente. Heurística léxica para comentarios de línea consecutivos idénticos; puede coincidir dentro de cadenas multilínea. Excluye fixtures y repeticiones intencionales. Nunca borra comentarios.
Admite off/warn/block, sin corrección automática.
