# Git integration

RepoRinse runs inside Git through small POSIX `sh` hook shims that call `reporinse hook git <hook> [args…]`.
[Español](#español)

```sh
reporinse install --provider git --scope repo --only git.ai-attribution   # one repository
reporinse install --provider git --scope user --all                        # every repository of this user
reporinse install --provider git --dry-run                                 # show the plan, write nothing
reporinse uninstall --provider git --scope repo                            # remove only what RepoRinse owns
```

Without `--all`/`--only`, `install` selects `--all`: every module that runs on Git, excluding high-risk modules
whose default mode modifies files. `--except` removes ids from `--all`. Unknown ids are a usage error (exit 2).
`--interactive` needs a terminal (exit 2 otherwise). Other providers answer "not supported by this build yet"
(exit 69).

## Hooks

| Hook | Installed when a selected module targets | What it does |
|---|---|---|
| `commit-msg` | commit messages (`git.ai-attribution`) | Reads the message file, runs the rules, applies `fix` plans, writes the file back atomically keeping its newline style and BOM, prints what it changed on stderr. Exit 1 only when a rule in `block` mode matched. |
| `pre-commit` | files or change sets | Checks the **staged** content (read from the index with `git cat-file`, never the working tree). Never modifies files or the index; fix plans are dropped. Exit 1 when a `block` rule matched. |

There is no `pre-push` hook: no shipped rule needs one. Git invokes `commit-msg` for `git commit` (including
`--amend`), `git merge` merge commits and `git rebase` steps that create a message; `prepare-commit-msg` is not
used.

Configuration or I/O problems **fail open**: the hook prints the problem and exits 0, because a cleanup rule must
never make commits impossible. Rules in `block` mode are the only way RepoRinse fails a commit.

## Shims

Each shim is LF-only `sh`, marked `# reporinse-managed-hook v1`, and:

1. sets `REPORINSE_HOOK_ACTIVE=1`; if it was already set (an outer RepoRinse shim is running), the RepoRinse step
   is skipped, so a chain never runs the rules twice or recurses;
2. runs the hook it replaced (`<hook>.reporinse-chained`, see below) first, with the same arguments and stdin;
   a non-zero exit stops there and is returned unchanged, and RepoRinse does not run;
3. (user scope) runs the repository's own `.git/hooks/<hook>` the same way;
4. runs the binary recorded at install time (absolute path), else `reporinse` from `PATH`, as
   `reporinse hook --only <modules> git <hook> "$@"` via `exec`, so its exit code is the hook's. If neither exists
   it prints `reporinse: binary not found …` and exits 0 (fail open).

`--only` in the shim is the module list recorded in the manifest, so the hooks run exactly the modules you
installed. Re-run `install` after adding modules.

## Where hooks go

### Repo scope

RepoRinse asks Git: `git rev-parse --path-format=absolute --git-path hooks`, run from the worktree top. This
honours a local `core.hooksPath` (absolute, or relative to the worktree top, as Git resolves it) and, in linked
worktrees, the shared hooks directory of the main repository. It also lists `git config --show-scope --get-all
core.hooksPath` to know where that value comes from.

- RepoRinse **never sets, changes or unsets** an existing `core.hooksPath`.
- If the effective value comes from the **global or system** config, a repo-scope install would change every
  repository: it is refused (exit 4) with options.
- Bare repositories are refused (no commits are made there). Outside a repository: exit 4 with a message.
- A hook file that is **versioned** (e.g. `.githooks/commit-msg` with `core.hooksPath=.githooks`) is never
  modified: exit 4, and the message shows the line to add yourself: `reporinse hook git commit-msg "$@"`.

### User scope (`--scope user`, alias `global`)

If no global or system `core.hooksPath` exists, RepoRinse creates `<home>/git-hooks` and runs
`git config --global core.hooksPath <home>/git-hooks`, where `<home>` is `$REPORINSE_CONFIG_HOME`, else
`$XDG_CONFIG_HOME/reporinse`, else `%APPDATA%\reporinse` on Windows, else `~/.config/reporinse`.

A global `core.hooksPath` makes Git ignore each repository's `.git/hooks`. The user-scope shims therefore also run
`$(git rev-parse --git-common-dir)/hooks/<hook>`, so per-repository hooks keep working. A repository that sets its
own `core.hooksPath` (e.g. Husky) still uses that one; install RepoRinse there with `--scope repo`.

If a global or system `core.hooksPath` already exists and is not RepoRinse's, install stops with exit 4 and
explains the options; nothing is overridden. (If that directory holds the no-ai-coauthor hook, the message says
how to retire it: [migration guide](../migration/no-ai-couathor.md).)

## Existing hooks: chaining

Chaining strategy: a foreign hook with the same name is **renamed in place** to `<hook>.reporinse-chained`
(a rename keeps its bytes, mode and timestamps) and the RepoRinse shim takes its name. The shim runs it first.
Plain `#!/bin/sh` and `#!/bin/bash` (`/usr/bin/env` forms too) hooks are run by that shell with `$0` set to the
original hook path, because some managers find their files from `$0`; other hooks are executed directly.

| Tool | Detected by | Behaviour |
|---|---|---|
| Husky v9 | `.husky/` dir, `core.hooksPath=.husky/_` | `.husky/_/<hook>` is chained; your `.husky/<hook>` scripts still run through Husky's `h`. `npm install` / `husky` regenerates `.husky/_`: run `reporinse install` again afterwards. |
| Husky v6–v8 (`core.hooksPath=.husky`, versioned hooks) | versioned hook files | refused (exit 4); add `reporinse hook git commit-msg "$1"` to `.husky/commit-msg`. |
| Lefthook | `lefthook.yml` (any of its names), hook content | chained; `lefthook install` rewrites hooks: run `reporinse install` again. |
| pre-commit | `.pre-commit-config.yaml`, "File generated by pre-commit" | chained; `pre-commit install` moves foreign hooks to `*.legacy`: run `reporinse install` again. |
| no-ai-coauthor | hook content | chained (both run); see the migration guide. |

If a tool regenerates a hook over the RepoRinse shim, `uninstall` reports that file as drift instead of deleting
it (see below).

## Manifest

Repo scope: `<git-common-dir>/reporinse/git-manifest.json` (inside `.git`, so never committed and shared by
linked worktrees). User scope: `<home>/git-hooks/.reporinse-manifest.json`.

```json
{
  "format": 1,
  "tool": "reporinse",
  "version": "0.1.0",
  "provider": "git",
  "scope": "repo",
  "hooks_dir": "C:/work/app/.git/hooks",
  "binary": "C:/tools/reporinse.exe",
  "modules": ["git.ai-attribution"],
  "installed_at": "2026-10-26T12:00:00Z",
  "files": [{ "hook": "commit-msg", "path": "commit-msg", "sha256": "…" }],
  "chained": [{ "hook": "commit-msg", "original": "commit-msg", "stored_as": "commit-msg.reporinse-chained",
                "sha256": "…", "mode": 493, "tool": "husky" }],
  "config": [{ "scope": "global", "key": "core.hooksPath", "value": "…/git-hooks", "previous": null }],
  "detected": ["husky"]
}
```

- **Install is idempotent**: when nothing changed, no file (manifest included) is rewritten.
- **Upgrade** rewrites only RepoRinse files whose hash still matches the manifest.
- A RepoRinse file that changed since install is a conflict (exit 4) unless `--force`.
- Every install/uninstall runs as a transaction: if a step fails, the finished steps are undone.

## Uninstall

`reporinse uninstall --provider git [--scope repo|user]` removes each shim whose SHA-256 still matches, moves
chained hooks back to their original names (byte-for-byte, same mode), restores the configuration (`previous:
null` → unset), deletes the manifest and, when empty, `.git/reporinse` (repo scope) or `<home>/git-hooks` (user
scope).

- Drifted shims or chained hooks → exit 4 listing them; nothing is removed. `--force` (with `--yes`, or confirmed
  on a terminal) removes/restores them anyway.
- A config value someone changed since install is left as is (reported).
- `uninstall --only <ids>` removes those modules and keeps the others installed (hooks no longer needed go away).

## `--no-verify` and limits

`git commit --no-verify` (and `git merge --no-verify`) skips `pre-commit` and `commit-msg`; RepoRinse honours
that. Client-side hooks are a convenience, **not a security control**: they can be skipped, deleted or not
installed in a clone. Enforce policies on the server or in CI if you need them.

## Windows notes

- Git for Windows runs hooks with its bundled `sh`, so the shims are `sh` scripts with LF endings; no PowerShell
  hook exists. Run `reporinse install` from PowerShell, cmd or Git Bash alike.
- The binary path is recorded with forward slashes (`C:/Users/…/reporinse.exe`), quoted, so spaces and non-ASCII
  characters work (covered by tests: every test runs under a path with spaces and accents).
- The executable bit is not needed on Windows; on Unix shims are written with mode `0755`.

## Exit codes

0 ok · 1 blocked by a `block` rule (hooks) · 2 usage · 4 conflict needing a decision · 69 provider not supported
yet · 70 internal error.

---

## Español

RepoRinse se integra con Git mediante pequeños scripts `sh` POSIX que llaman a `reporinse hook git <hook>`.

```sh
reporinse install --provider git --scope repo --only git.ai-attribution   # un repositorio
reporinse install --provider git --scope user --all                        # todos los repos del usuario
reporinse install --provider git --dry-run                                 # ver el plan sin escribir nada
reporinse uninstall --provider git --scope repo                            # quitar solo lo de RepoRinse
```

### Hooks

- `commit-msg`: limpia el mensaje (reglas en modo `fix`), lo reescribe de forma atómica respetando saltos de
  línea y BOM, e informa en stderr. Sale con 1 solo si una regla en modo `block` coincide.
- `pre-commit`: revisa el contenido **staged** (del índice, nunca del árbol de trabajo). Jamás modifica archivos
  ni el índice.
- No hay `pre-push`: ninguna regla lo necesita todavía.
- Errores de configuración o de E/S **no bloquean** el commit (fail-open), se informan y el hook sale con 0.

### Alcance repo y usuario

- **repo**: instala en el directorio que Git usa de verdad (`git rev-parse --git-path hooks`), respetando un
  `core.hooksPath` local absoluto o relativo y los worktrees enlazados. Nunca cambia ni borra `core.hooksPath`.
  Si ese valor viene del config global/sistema, se rechaza (código 4). Repos bare: rechazados. Hooks versionados:
  no se tocan (código 4) y se indica la línea a añadir a mano.
- **user** (`global`): si no hay `core.hooksPath` global ni de sistema, crea `<home>/git-hooks` y fija
  `git config --global core.hooksPath`. Como eso oculta `.git/hooks` de cada repo, los shims de usuario también
  ejecutan `.git/hooks/<hook>` del repositorio. Si ya existe un `core.hooksPath` global ajeno: conflicto, código
  4, con opciones; nunca se sobrescribe.

### Encadenado

Un hook ajeno con el mismo nombre se renombra en su sitio a `<hook>.reporinse-chained` y se ejecuta **primero**
con los mismos argumentos y stdin. Si falla, su código de salida se propaga y RepoRinse no corre. Los hooks
`sh`/`bash` simples se ejecutan con `$0` igual a la ruta original (Husky localiza sus archivos con `$0`).
Husky v9, Lefthook, pre-commit y no-ai-coauthor se detectan y se avisa de que, si regeneran sus hooks, hay que
volver a ejecutar `reporinse install`.

### Manifest y desinstalación

El manifest (`.git/reporinse/git-manifest.json` en repo; `<home>/git-hooks/.reporinse-manifest.json` en usuario)
guarda versión, alcance, archivos con SHA-256, hooks encadenados con su SHA-256 y modo, claves de config con su
valor previo, módulos y fecha. Reinstalar no cambia nada si no hay cambios; desinstalar borra solo archivos cuyo
hash coincide, restaura byte a byte los hooks encadenados y la config previa. Archivos modificados → conflicto
(código 4) salvo `--force`. Cada operación es transaccional: si un paso falla, se deshace lo hecho.

### `--no-verify` y Windows

`git commit --no-verify` omite los hooks: son una ayuda local, **no un control de seguridad**. En Windows, Git
for Windows ejecuta los hooks con su `sh`; los shims usan LF y guardan la ruta del binario con barras `/`, entre
comillas, así que rutas con espacios y acentos funcionan (todas las pruebas corren en rutas así).
