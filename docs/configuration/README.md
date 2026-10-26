# Configuration / Configuración

Run `reporinse init` to create `.reporinse.toml` at the Git worktree root (or current directory outside Git).
Existing files are preserved unless `--force` is supplied. `--preset` accepts `minimal`, `balanced`, `strict`, and `custom`.

Ejecuta `reporinse init` para crear `.reporinse.toml` en la raíz del repositorio Git (o el directorio actual fuera de Git).
Usa `--force` para reemplazar una configuración existente.

```toml
version = 1
preset = "balanced"
exclude = ["**/target/**"]

[providers]
git = true

[rules."git.ai-attribution"]
mode = "fix"
policy = "remove-known-ai-only"
```

Precedence, highest first: CLI overrides, repository file, user file, preset, built-ins.
Rule option tables merge by key; exclusion arrays replace the previous array.
The highest-precedence preset supplies defaults before file rule settings are applied.
`balanced` uses rule defaults; `minimal` enables only P0 defaults; `strict` uses block where supported;
`custom` starts every rule off. Explicit rule modes override preset defaults.

Precedencia: CLI > repositorio > usuario > preset > valores incorporados.
Las opciones se combinan por clave; las listas de exclusiones se reemplazan.
`custom` desactiva todas las reglas inicialmente. Los modos explícitos tienen prioridad sobre el preset.

The user file is `config.toml` inside `REPORINSE_CONFIG_HOME`, otherwise
`XDG_CONFIG_HOME/reporinse`, otherwise `%APPDATA%\reporinse` on Windows,
otherwise `~/.config/reporinse`. Missing files are allowed; unreadable or malformed files are errors.
`version` may be omitted; when present it must be `1`.

`reporinse config validate` rejects unknown keys, providers, rule IDs, invalid modes, options, and exclusion globs.
`reporinse config explain` prints effective settings and their origins; both commands accept the global `--json` flag.
Configuration errors exit with code 3. `providers.git = false` disables RepoRinse Git hook processing.
Agent provider flags are reserved configuration keys; they do not enable adapters absent from this build.

`reporinse config validate` verifica claves, proveedores, reglas, modos, opciones y patrones de exclusión.
`reporinse config explain` muestra los valores efectivos y su origen. Ambos aceptan `--json`.
Los errores de configuración devuelven código 3. `providers.git = false` desactiva los hooks de RepoRinse.
Las claves de agentes están reservadas y no implementan adaptadores por sí solas.


## Delivery options

See examples/.reporinse.toml for all implemented rule IDs. text.whitespace accepts boolean honor_editorconfig
(default true); git.diff-scope accepts positive max_files/max_lines. agent.command-guard and agent.path-scope
accept deny arrays of glob strings. Other newly added rules accept mode/exclude only. Unknown options are errors.
Agent policies start empty and match nothing. Use block explicitly to veto a matching pre-tool action.

## Español: opciones adicionales

Consulta examples/.reporinse.toml. honor_editorconfig es booleano; max_files/max_lines son enteros positivos;
deny es una lista de patrones glob. Una política vacía no coincide con ninguna acción.
