# RepoRinse

Conserva el código. Limpia el ruido. [English](README.md)

CLI local en Rust para quitar atribución conocida de agentes en commits, revisar texto y aplicar políticas explícitas.
No usa IA, servicios de red ni telemetría durante su ejecución.

## Compilar y usar

Necesitas Rust **1.88+** y Git. Python 3 solo se usa para empaquetar.

```sh
cargo build --release --locked
```

Ejecutable: `target/release/reporinse`, o `target\release\reporinse.exe` en Windows.
`python scripts/package.py` crea el paquete offline y su SHA-256 en `dist/`.
Incluye instaladores PowerShell 7/shell. Conserva el ejecutable en una ruta estable: los hooks registran su ruta absoluta.

```sh
reporinse init
reporinse config validate
reporinse check --files src/main.rs
reporinse check --staged
reporinse status
reporinse doctor
```

`check` no escribe. Elige una entrada: `--files`, `--message` o `--staged`.
Códigos: advertencias/éxito 0, bloqueos 1, uso incorrecto 2, configuración 3, conflictos 4.
`--json` devuelve resultados estructurados; `--quiet` oculta hallazgos que no bloquean.

## Reglas

| Regla | Modo inicial | Función |
|---|---|---|
| git.ai-attribution | fix | Quita trailers/pies conocidos de agentes; conserva coautores humanos |
| text.whitespace | warn | Espacios/tabuladores finales; conserva saltos de Markdown |
| text.invisible-unicode | warn | Informa controles invisibles/bidireccionales sin borrarlos |
| docs.agent-artifacts | warn | Revisa planes/resúmenes Markdown recién preparados fuera de docs |
| git.diff-scope | warn | Informa más de 25 archivos o 500 líneas; configurable |
| git.dependency-watch | warn | Informa manifiestos y lockfiles modificados |
| code.duplicate-comments | off | Revisa comentarios consecutivos idénticos al activarla |
| agent.command-guard | warn | Patrones de comandos configurados; sin patrones no coincide nada |
| agent.path-scope | warn | Patrones de rutas configurados; sin patrones no coincide nada |

Consulta `reporinse rule explain <id>` y el [ejemplo de configuración](examples/.reporinse.toml).

## Corregir

Activa la corrección en `.reporinse.toml`:

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

El índice se revisa/corrige solo en líneas añadidas/modificadas; los archivos explícitos se revisan completos.
Se respeta `trim_trailing_whitespace=false` de EditorConfig y se usa su versión preparada al analizar el índice.
Se conservan BOM, CRLF y ausencia de salto final. Se rechazan archivos parcialmente preparados, diferencias de
codificación del checkout, enlaces, rutas externas, binarios y archivos mayores de 8 MiB.
La corrección bloquea el índice y usa archivos temporales; evita editar simultáneamente.

## Integraciones

```sh
reporinse install --provider git --all --dry-run
reporinse install --provider git --all --yes
reporinse install --provider claude,codex,gemini --all --yes
reporinse uninstall --provider git,claude,codex,gemini --all --yes
```

También admite `--provider cursor`, alcances `repo`/`user`, selección `--only`/`--except` e interacción en terminal.
Cada proveedor revierte sus propias escrituras fallidas. Conserva hooks existentes y restaura bytes originales
mediante un manifest. Si detecta cambios posteriores se detiene; inspecciona antes de usar `--force`.

Codex requiere aprobar cada hook en `/hooks` y confiar en el proyecto. RepoRinse no cambia esa confianza.
Cursor puede cargar también hooks de Claude: elige una integración para evitar ejecuciones duplicadas.
Antigravity queda cubierto por `--provider git`; no se instalan hooks nativos sin verificar.
Las políticas solo bloquean antes de la acción cuando se configuran en `block`; son patrones de texto,
no un parser de shell ni un sustituto de permisos/sandbox. No se adivinan formatos no documentados.

## Verificación

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
python scripts/package.py
```

Pruebas aisladas en directorios temporales. CI preparado para Windows, Linux y macOS.
El [reporte de verificación](docs/development/verification.md) distingue pruebas ejecutadas, CI configurado y sesiones
reales de agentes. Más información: [seguridad](SECURITY.md), [configuración](docs/configuration/README.md).
