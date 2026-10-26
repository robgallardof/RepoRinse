# claude

Official protocol: [documentation](https://code.claude.com/docs/en/hooks), checked 2026-10-26.

Installed events: PreToolUse / PostToolUse. Repo settings: `.claude/settings.json`; user scope uses the corresponding home directory.
Claude honors CLAUDE_CONFIG_DIR; Codex honors CODEX_HOME for user scope.

Sets empty commit/pr attribution strings (restored on uninstall). Exec-form absolute executable plus args; 10-second timeout. Before-tool blocks use permissionDecision=deny. No explicit allow decision is emitted. Cursor may load these hooks too.

```sh
reporinse install --provider claude --all --dry-run
reporinse install --provider claude --all --yes
reporinse uninstall --provider claude --all --yes
```

Enable agent.command-guard or agent.path-scope policies in .reporinse.toml. Empty policies match nothing.
Block applies only before actions; later events warn. Invalid input/config fails open. Input cap: 1 MiB.
Messages contain rule IDs and generic policy notices, not commands, paths or source contents.
Install merges hooks; a sibling manifest stores original bytes. Uninstall refuses drift. No host trust is changed.
Windows shell commands use a fixed encoded PowerShell invocation; payload text is never interpolated into it.

## Español

Instala con los comandos anteriores y configura patrones explícitos; sin patrones no se bloquea nada.
Conserva ajustes previos y restaura bytes originales. El manifest detiene la desinstalación si detecta cambios.
Los contratos y comandos instalados se prueban en procesos aislados; no se afirma una sesión real en la aplicación.
