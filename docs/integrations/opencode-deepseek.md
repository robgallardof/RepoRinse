# OpenCode and DeepSeek Harness

Project-only native bridges (no user-wide settings or model credentials):

```sh
reporinse install --provider opencode,deepseek --all --dry-run
reporinse install --provider opencode,deepseek --all --yes
# OpenCode auto-loads .opencode/plugins/reporinse.js.
# DeepSeek requires this explicit overlay:
dsh web --patch .dsh/reporinse.cordis.yml
reporinse uninstall --provider opencode,deepseek --all --yes
```

Set explicit policies in `.reporinse.toml`; empty policies match nothing:

```toml
version = 1
[rules."agent.command-guard"]
mode = "block"
deny = ["*never-run-this*"]
[rules."agent.path-scope"]
mode = "block"
deny = ["*private.key"]
```

OpenCode **1.19.4** uses documented 1.x `tool.execute.before`/`tool.execute.after`
callbacks. The bridge maps `bash`, `read`, `write`, `edit`, and `filePath` into structured
CLI input. A pre-tool deny throws; an after-tool callback cannot veto an action.
OpenCode 2.x has a different plugin API and is not covered by this adapter.

DeepSeek Harness **0.2.1-rc.2** uses its native `tools/pre-execute` waterfall and returns
`kind: deny` before dispatch. It supports `bash`, `pwsh` and `str_replace_editor`
(and file tools with documented `path`/`file_path` arguments). Policies resolve against the
installed repository root; changing a session directory does not retarget this bridge.
No SDK compatibility shim or model call is needed. Other tools keep normal host behavior.

Both bridges spawn an absolute RepoRinse executable with a structured argument list,
`windowsHide`/no shell and JSON stdin. They never evaluate the command being inspected.
Timeout, unavailable executable or malformed hook output fail open. Output is capped at
1 MiB. No approval/host permissions are changed. Warnings currently do not add host UI
messages; use `reporinse check` for a report. Blocking is opt-in through rule mode.
Owned plugin files and the DeepSeek overlay are installed transactionally, never replace
foreign files, and refuse uninstall on drift. Uninstall preserves unrelated plugins.

The ordinary Git hooks also run when either agent invokes Git. They remove recognized
OpenCode/DeepSeek trailers and standalone generated footers while preserving human co-authors.

## Evidence and reproduction

- `cargo test`: real Git commits, actual CLI subprocesses, dry run, idempotence, drift,
  foreign-file refusal and uninstall.
- `python scripts/test_agent_hosts.py`: installed OpenCode callbacks → real CLI, allowed
  and blocked commands/paths, non-blocking after callback; real DeepSeek tool runtime
  proves the denied body never ran and the allowed body ran exactly once.
- `OPENCODE_BIN` additionally runs real OpenCode project startup; no model turn is claimed.
- Node 24.9+; `cargo build`; set `AGENT_HOST_MODULES` to isolated node_modules containing
  `@deepseek-ai/dsh@0.2.1-rc.2`. `agent-hosts.yml` reproduces the probe on Linux.
- Local Windows probe passed. Live model inference: **NOT EXECUTED**, no DeepSeek API key available.

Official contracts checked 2026-10-26:
[OpenCode plugins](https://opencode.ai/docs/plugins/),
[DeepSeek tools runtime](https://github.com/deepseek-ai/deepseek-harness/tree/master/packages/core/tools).
