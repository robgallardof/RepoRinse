# text.whitespace

Reports trailing ASCII spaces and tabs. Modes: `off`, `warn` (default), `block`, `fix`.
Only file targets are inspected. Markdown (`md`, `markdown`, `mdx`) runs of two or more spaces
are preserved because they may be hard breaks. Newlines, BOM and missing final newline are preserved.
Honors repository-local EditorConfig trim_trailing_whitespace (default honor_editorconfig=true). Unknown options are rejected. Explicit files use the whole document; staged input uses added/modified lines.

```toml
[rules."text.whitespace"]
mode = "fix"
exclude = ["generated/**"]
```

Use `reporinse check --files a.txt`, `check --staged`, or `fix --files a.txt --dry-run`.
Fix applies only rules explicitly configured in `fix` mode. `fix --staged` updates the index and worktree only when they have identical original bytes and no unstaged changes.
Files must be UTF-8, at most 8 MiB, inside the repository, and reached without symlinks.
Binary files and Git internals are refused. Fixes recheck original bytes before replacing a file.
This check does not provide locking against concurrent writers; avoid editing during a fix.

## Español

Detecta espacios y tabuladores al final de línea. Conserva los saltos de Markdown de dos o más
espacios, BOM, CRLF y la ausencia de salto final. El modo predeterminado es `warn`.
Para modificar, configura `mode = "fix"` y usa `fix --files`; `--dry-run` muestra el diff.
`check --staged` analiza el índice sin modificarlo. La corrección del índice rechaza archivos parcialmente preparados.
Respeta trim_trailing_whitespace de EditorConfig. Rechaza binarios, rutas externas, enlaces y archivos mayores de 8 MiB.
