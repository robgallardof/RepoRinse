# docs.agent-artifacts

Newly staged agent document review.

Default warn. Reports added PLAN.md, SUMMARY.md, implementation_plan.md, walkthrough.md, task.md, agent-notes.md and claude-summary.md outside docs/ and .github/. Case-insensitive names. Renamed/modified/deleted files are not reported. No content attribution, link-graph analysis or deletion. Exclude legitimate paths.

```toml
[rules."docs.agent-artifacts"]
mode = "warn"

```

Supports off/warn/block; no automatic fix. Per-rule `exclude` globs are available for file/change-set targets.

## Español

Revisa nombres de planes/resúmenes Markdown recién añadidos al índice fuera de docs/ y .github/. Solo informa; no identifica autoría ni borra archivos. Excluye rutas legítimas.
Admite off/warn/block, sin corrección automática.
