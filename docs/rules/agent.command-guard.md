# agent.command-guard

Explicit command glob policy.

Matches configured globs against a reported shell command. Default warn, empty deny list. Never executes or parses shell code. Obfuscated/equivalent commands can bypass a string policy.

```toml
[rules."agent.command-guard"]
mode = "warn"
deny = ["*git reset --hard*"]
```

Supports off/warn/block; no automatic fix. Per-rule `exclude` globs are available for file/change-set targets.

## Español

Compara patrones configurados con el comando recibido. Sin patrones no coincide nada. No ejecuta comandos ni interpreta shell; no es una frontera de seguridad.
Admite off/warn/block, sin corrección automática.
