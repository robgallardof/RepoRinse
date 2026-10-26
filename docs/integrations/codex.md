# codex

Official protocol: [documentation](https://learn.chatgpt.com/docs/hooks), checked 2026-10-26.

Installed events: PreToolUse / PostToolUse. Repo settings: `.codex/hooks.json`; user scope uses the corresponding home directory.
Claude honors CLAUDE_CONFIG_DIR; Codex honors CODEX_HOME for user scope.

Review/trust each hook in /hooks; trust is hash-based and must be renewed after changes. Project hooks require a trusted project. notify is not used. Patch-file path extraction remains unverified. 10-second timeout.

```sh
reporinse install --provider codex --all --dry-run
reporinse install --provider codex --all --yes
reporinse uninstall --provider codex --all --yes
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
