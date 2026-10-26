# Compatibility matrix: Git and agent hook hosts

Checked on **2026-10-26** against official documentation fetched that day. This file is the source of truth for
`AGENTS.md` rule 5 ("no invented integrations"). It is research, not a support claim: a provider/event becomes
*supported* only when an adapter implements it **and** a contract test built from the fixtures below passes.

Legend

- **Fact**: stated on the cited page (quoted or paraphrased closely). Every fact table row cites its source.
- **Inference**: our reading, clearly labelled. It must be confirmed by a contract test or a live run before relying on it.
- **Pending verification**: not documented, or not fetchable; do not implement behaviour that depends on it.
- Verdicts: **docs-verified** (official docs checked, blocking contract documented, verbatim fixtures available),
  **contract-only** (documented but key semantics missing or host reported not to honour them), **git-only**
  (no usable agent hook; rely on Git hooks + `reporinse check`), **unsupported**.

| Provider | Verdict | Pre-action veto | Default on hook crash/timeout | Sources |
|---|---|---|---|---|
| Git | docs-verified | yes (`pre-commit`, `commit-msg`, `pre-push`, non-zero exit) | n/a (exit status decides) | githooks, git-config, git-commit, git-push, git-rev-parse |
| Claude Code | docs-verified | yes (`PreToolUse`) | fail-open; fail-closed only with `onFailure: "block"` (v2.1.295+) | hooks, settings, settings-reference |
| Codex CLI | docs-verified (with caveats: trust review, no verbatim PreToolUse payload) | yes (`PreToolUse`) | fail-open (documented for unsupported output fields and managed MCP hooks; command-hook crash: pending) | learn.chatgpt.com/docs/hooks, config-advanced |
| Gemini CLI | docs-verified (payload fixture is a schema, not an example) | yes (`BeforeTool`) | fail-open ("Other" exit = warning); timeout outcome pending | geminicli.com/docs/hooks |
| Cursor | docs-verified | yes (`beforeShellExecution`, `beforeReadFile`, `preToolUse`) | fail-open; **invalid JSON on permission hooks blocks**; `failClosed: true` blocks on crash/timeout | cursor.com/docs/hooks |
| Antigravity | contract-only for `agy` CLI; **git-only** by default | documented (`PreToolUse` `decision`) | undocumented | antigravity.google/docs/hooks; community repro says IDE/App do not run hooks |

---

## 1. Git

Sources (checked 2026-10-26; local reference build `git version 2.55.0.windows.3`):

- https://git-scm.com/docs/githooks ("githooks last updated in 2.54.0")
- https://git-scm.com/docs/git-config ("last updated in 2.56.0")
- https://git-scm.com/docs/git-commit, https://git-scm.com/docs/git-push ("last updated in 2.55.0")
- https://git-scm.com/docs/git-rev-parse ("last updated in 2.56.0")
- Git 2.9.0 release notes, `Documentation/RelNotes/2.9.0` in https://github.com/git/git: "A new configuration variable core.hooksPath allows customizing where the hook directory is."

### 1.1 Hooks RepoRinse uses

| Hook | Stage | Can veto? | Input | Skipped by `--no-verify`? | Notes (fact unless labelled) |
|---|---|---|---|---|---|
| `pre-commit` | before | yes, non-zero exit aborts the commit | no args; content is the index | **yes** (`git commit --no-verify`/`-n`: "Bypass the pre-commit and commit-msg hooks") | Runs with `GIT_EDITOR=:` when no editor will open. |
| `prepare-commit-msg` | before | yes, non-zero aborts | `$1` message file, `$2` source (`message`, `template`, `merge`, `squash`, `commit`), `$3` commit SHA for `-c/-C/--amend` | **no**: "it is not suppressed by the --no-verify option" | Docs: "It should not be used as a replacement for the pre-commit hook." |
| `commit-msg` | before | yes, non-zero aborts | `$1` message file; may edit in place | **yes** | Invoked by `git commit` **and** `git merge`. |
| `pre-merge-commit` | before | yes | no args | yes | Not run when the merge stops on conflicts; the follow-up `git commit` runs `pre-commit` instead. |
| `pre-push` | before | yes, non-zero aborts the push | `$1` remote name, `$2` URL; stdin lines `<local-ref> SP <local-object-name> SP <remote-ref> SP <remote-object-name> LF` | **yes** (`git push --no-verify`: "the hook is bypassed completely") | Deleted ref: local ref `(delete)` and all-zero object name; new remote ref: all-zero remote object. |
| `post-rewrite` | after | no | `$1` = `amend` or `rebase`; stdin `<old> SP <new> [SP <extra>] LF` | `git commit --no-verify` docs list it separately ("Bypass the post-rewrite hook") | Not called by full-history rewriters (`fast-import`, `filter-repo`). |

Behaviour around commits (facts unless labelled):

| Scenario | What runs |
|---|---|
| `git commit --amend` | `pre-commit`, `prepare-commit-msg` (`$2=commit`, `$3=HEAD`), `commit-msg`, then `post-rewrite amend` (git-commit HOOKS section). |
| `git merge` creating a merge commit | `pre-merge-commit`, `prepare-commit-msg` (`merge`), `commit-msg`. |
| Merge with conflicts, resolved by `git commit` | `pre-commit`, `commit-msg` (githooks, pre-merge-commit section). |
| `git rebase` replaying commits | **Pending verification.** githooks documents only `pre-rebase` and `post-rewrite` for rebase. Whether `commit-msg`/`pre-commit` run for `pick`/`reword`/`edit` must be established by a git-hooks-specialist integration test. |
| `git -c core.hooksPath=/dev/null …` | All hooks disabled for that command (git-config, `core.hooksPath`). This is a documented bypass. |

Hook environment (githooks DESCRIPTION, fact): Git changes cwd to the **work tree root** (non-bare) or `$GIT_DIR` (bare);
push-side server hooks always run in `$GIT_DIR`. `GIT_DIR`, `GIT_WORK_TREE`, … are exported; a hook that runs Git in another
repository must clear them (`unset $(git rev-parse --local-env-vars)`). Hooks without the executable bit are ignored.

### 1.2 `core.hooksPath` and scopes

| Item | Fact | Source |
|---|---|---|
| Default hooks dir | `$GIT_DIR/hooks` | git-config `core.hooksPath` |
| Absolute vs relative | "The path can be either absolute or relative. A relative path is taken as relative to the directory where the hooks are run" (i.e. the work tree root, or `$GIT_DIR` when bare) | git-config |
| Disable all hooks | `core.hooksPath=/dev/null` | git-config |
| Files read, in order | `$(prefix)/etc/gitconfig` (system) → `$XDG_CONFIG_HOME/git/config` then `~/.gitconfig` (global) → `$GIT_DIR/config` (local) → `$GIT_DIR/config.worktree` (only if `extensions.worktreeConfig`) → `-c` | git-config FILES |
| Precedence | "last value found taking precedence over values read earlier" → worktree > local > global > system; `-c` overrides all | git-config FILES |
| Effective path | `git rev-parse --git-path hooks` resolves `$GIT_DIR/<path>` "and takes other path relocation variables … into account" | git-rev-parse |
| Introduced | Git 2.9.0 | RelNotes 2.9.0 |

Inferences (to be covered by tests, not assumed):

- I1. A **relative** `core.hooksPath` set in global config resolves differently in every repository and worktree (it is relative
  to each work tree root). The installer must report it as such, not normalise it.
- I2. `git rev-parse --git-path hooks` is the value to trust for "where Git will look"; it honours `core.hooksPath` in
  observed behaviour, but its doc text only mentions relocation variables. Contract test: set `core.hooksPath` at each scope
  and compare.
- I3. Linked worktrees share `$GIT_COMMON_DIR/hooks` unless `core.hooksPath` or `config.worktree` says otherwise.

### 1.3 Git: minimal fixtures

Hook invocation contract (verbatim from githooks, `pre-push`):

```text
<local-ref> SP <local-object-name> SP <remote-ref> SP <remote-object-name> LF
```

```text
refs/heads/master 67890 refs/heads/foreign 12345
```

Registration: a hook is an executable file named after the hook inside the effective hooks directory; there is no
config snippet. The RepoRinse shim (`reporinse hook git <hook-name> [args…]`) follows `docs/architecture/plan.md`.

---

## 2. Claude Code

Sources (checked 2026-10-26):

- https://code.claude.com/docs/en/hooks (fetched as `hooks.md`)
- https://code.claude.com/docs/en/settings (fetched as `settings.md`)
- https://code.claude.com/docs/en/settings-reference (`attribution`, `includeCoAuthoredBy`)
- https://cursor.com/docs/reference/third-party-hooks (Cursor also runs Claude Code hooks; see 5.5)

### 2.1 Configuration

| Item | Fact |
|---|---|
| Files | `~/.claude/settings.json` (user), `.claude/settings.json` (shared project, committable), `.claude/settings.local.json` (project local, git-excluded when Claude Code creates it), managed settings, plugin `hooks/hooks.json`, skill/subagent frontmatter. Windows: `~` = `%USERPROFILE%`; `CLAUDE_CONFIG_DIR` relocates home files. |
| Precedence (settings keys) | managed > `--settings` > project local > shared project > user. |
| Hooks merging | "Hook entries merge across settings levels rather than replacing each other." Identical handler in several settings files runs once. |
| Shape | `hooks.<Event>[] = { matcher, hooks: [ { type: "command", command, args?, timeout?, if?, shell?, async?, onFailure?, statusMessage? } ] }` |
| Matcher | `"*"`, `""` or omitted = all; only `[A-Za-z0-9_- ,|]` = exact name list; anything else = unanchored JavaScript regex. Unsupported-matcher events ignore it. |
| `if` | One permission rule (e.g. `Bash(git *)`), tool events only; best-effort; docs say use permissions, not hooks, for hard allow/deny. |
| Exec form | With `args`, `command` is spawned directly with no shell (recommended for paths with spaces). On Windows exec form needs a real `.exe`. |
| Shell form | `sh -c` (macOS/Linux), Git Bash on Windows, PowerShell when Git Bash is missing. |
| Timeout | seconds; default 600 for `command`; 30 on `UserPromptSubmit`; `SessionEnd` shares a 1.5 s budget. |
| Disable | `"disableAllHooks": true` (cannot disable managed hooks from non-managed levels); `--settings '{"disableAllHooks": true}'` for one run. |
| Parallelism | Matching hooks for an event run in parallel. |
| Environment | `CLAUDE_PROJECT_DIR`, `CLAUDE_PLUGIN_ROOT`, `CLAUDE_PLUGIN_DATA`; `$CLAUDE_CODE_REMOTE="true"` in web sessions. |
| Version gates seen | `onFailure` v2.1.295+; `scratchpad_dir` input v2.1.257+; `attribution: false` v2.1.281+; `includeCoAuthoredBy` deprecated since v2.0.62. No minimal version for hooks themselves is stated. |

### 2.2 Exit codes and failure (fact)

| Stdout \ exit | 0 | 2 | other |
|---|---|---|---|
| valid JSON object | success, fields apply | blocking error, fields read but cannot override the block | fields alone decide (unless `onFailure: "block"` → failure) |
| invalid JSON | non-blocking error | blocking error, stderr is the reason | non-blocking error |
| plain text / empty | success | blocking error, stderr is the reason | non-blocking error (first stderr line shown) |

- Exit 1 without valid JSON is **non-blocking** ("If your hook is meant to enforce a policy, use `exit 2`").
- Timeout: output discarded; on `PreToolUse` the call continues through the normal permission flow (fail-open) unless `onFailure: "block"`.
- `onFailure: "block"` makes can't-start / exit∉{0,2} / timeout / invalid output behave like exit 2. No effect on `Stop`, `SubagentStop`, `TaskCompleted`, `TeammateIdle`, async hooks.
- Windows without Git Bash: a shell-form `PreToolUse`/`PermissionRequest` hook that fails with `onFailure` unset can **block** the call ("only exit code 0 or 2 counts as this hook's verdict").
- Background-process trap: printing allow then leaving a child holding stdout makes Claude Code **block** (`refusing to honor the suspect capture`).
- Stdout must be exactly one JSON object; anything else is plain text or a parse failure. Strings capped at 10,000 chars.

### 2.3 Events relevant to RepoRinse

| Event | Stage | Can veto? | Payload fields we need | Allow | Warn (non-blocking) | Block |
|---|---|---|---|---|---|---|
| `PreToolUse` | before | **yes** | `session_id`, `cwd`, `hook_event_name`, `tool_name` (`Bash`, `PowerShell`, `Write`, `Edit`, `Read`, MCP names), `tool_input.command` (Bash/PowerShell), `tool_input.file_path` (Write/Edit/Read; always absolute, backslashes on Windows), `tool_input.content`, `tool_input.old_string`/`new_string`, `tool_use_id` | exit 0, empty stdout ("no decision; normal permission flow applies") | exit 0 + `systemMessage` (shown to user) and/or `hookSpecificOutput.additionalContext` (to Claude) | exit 0 + `hookSpecificOutput.permissionDecision: "deny"` + `permissionDecisionReason` (shown to Claude); or exit 2 + stderr |
| `PostToolUse` | after | no (tool already ran) | as above + `tool_response`, `duration_ms`; Bash may carry `tool_response.bashEditDiff.changedFiles` (beta, v2.1.269+, "not to enforce a policy") | exit 0, empty | `additionalContext` / `systemMessage` | `decision: "block"` + `reason` only adds feedback next to the result; exit 2 shows stderr to Claude |
| `Stop` | lifecycle | yes, but meaning is "keep working" | `stop_hook_active`, `last_assistant_message`, `background_tasks`, `session_crons` | exit 0, empty | `hookSpecificOutput.additionalContext` (continues the conversation as feedback) | `decision: "block"` + `reason` prevents stopping; 8-consecutive-continuation cap |
| `SessionStart` | lifecycle | no | `source` (`startup`, `resume`, `clear`, `compact`, `fork`), `model?` | exit 0 | plain stdout or `additionalContext` becomes context | n/a |
| `PreCompact` | lifecycle | yes (blocks compaction) | `trigger` | not used by RepoRinse | — | — |
| `UserPromptSubmit` | lifecycle | yes | `prompt` | not used by RepoRinse | — | — |

`PermissionDecision` values: `allow` (skips the permission prompt), `deny`, `ask`, `defer` (`-p` only). Precedence across hooks:
`deny > defer > ask > allow`. Deny/ask permission rules are still evaluated whatever a hook returns. Top-level
`decision`/`reason` is deprecated for `PreToolUse` (`approve`→`allow`, `block`→`deny`).

Gaps (fact): `PreToolUse` does not fire for files referenced with `@` in the prompt, nor for `EndConversation`. A hook that
matches only `Bash` never fires on Windows hosts where the PowerShell tool replaces Bash; match `Bash|PowerShell`.

### 2.4 Commit attribution settings (fact, settings-reference)

- `attribution` (object `{commit, pr, sessionUrl}` or `false`, any settings file). `false` hides all attribution; requires
  v2.1.281+, and **earlier versions reject it and skip the whole settings file**. For files older versions read, use
  `"commit": ""`, `"pr": ""`, `"sessionUrl": false`.
- `attribution.commit` default: `Co-Authored-By: <name> <noreply@anthropic.com>` where `<name>` is the model (e.g. `Claude Sonnet 5`),
  `Claude` when the version is unknown, `Claude Code` for unrecognised models.
- `attribution.pr` default: `🤖 Generated with [Claude Code](https://claude.com/claude-code)`.
- `attribution.sessionUrl` default `true`: adds a `Claude-Session` trailer on commits from cloud/Remote Control sessions.
- `includeCoAuthoredBy`: deprecated since v2.0.62; `false` still honoured unless `attribution.commit`/`pr` is set.
- Claude is told that CLAUDE.md/memory instructions about attribution take precedence over these lines unless set in managed settings.

### 2.5 Verbatim fixtures

Payload (`PreToolUse`, Bash), https://code.claude.com/docs/en/hooks#common-input-fields:

```json
{
  "session_id": "abc123",
  "prompt_id": "550e8400-e29b-41d4-a716-446655440000",
  "transcript_path": "/home/user/.claude/projects/.../transcript.jsonl",
  "cwd": "/home/user/my-project",
  "scratchpad_dir": "/tmp/claude-1000/-home-user-my-project/abc123/scratchpad",
  "permission_mode": "default",
  "hook_event_name": "PreToolUse",
  "tool_name": "Bash",
  "tool_input": {
    "command": "npm test",
    "description": "Run test suite",
    "timeout": 120000,
    "run_in_background": false
  },
  "tool_use_id": "toolu_01ABC123..."
}
```

Registration (exec form, fail-closed), https://code.claude.com/docs/en/hooks#block-the-action-when-a-hook-fails:

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          {
            "type": "command",
            "command": "node",
            "args": ["${CLAUDE_PROJECT_DIR}/.claude/hooks/check-command.js"],
            "onFailure": "block"
          }
        ]
      }
    ]
  }
}
```

Deny output, https://code.claude.com/docs/en/hooks#decision-control:

```json
{
  "hookSpecificOutput": {
    "hookEventName": "PreToolUse",
    "permissionDecision": "deny",
    "permissionDecisionReason": "Database writes are not allowed"
  }
}
```

---

## 3. Codex CLI (OpenAI)

Sources (checked 2026-10-26):

- https://developers.openai.com/codex/hooks → **308 redirect** to https://learn.chatgpt.com/docs/hooks (fetched `.md`)
- https://developers.openai.com/codex/config-advanced (`notify`), https://developers.openai.com/codex/config-reference
- https://developers.openai.com/plugins/build/plugins (plugin-bundled hooks)
- Latest release seen: `rust-v0.164.0` (2026-10-23), https://github.com/openai/codex/releases. The hooks page states no minimal version.

**Fact: Codex has real lifecycle hooks with a pre-tool veto (`PreToolUse`), distinct from `notify`.** `notify` runs an external
program only for `agent-turn-complete`, receives a single JSON **argv** argument (`type`, `thread-id`, `turn-id`, `cwd`,
`input-messages`, `last-assistant-message`) and cannot block anything. Do not treat `notify` as a guardrail.

### 3.1 Configuration

| Item | Fact |
|---|---|
| Files | `~/.codex/hooks.json`, `~/.codex/config.toml` (`[hooks]`), `<repo>/.codex/hooks.json`, `<repo>/.codex/config.toml`; plugin `hooks/hooks.json` or `hooks` in `.codex-plugin/plugin.json`; managed `requirements.toml`. |
| Merging | All matching hooks from all layers run; higher layers do not replace lower ones; `hooks.json` + inline `[hooks]` in one layer are merged with a startup warning. |
| Trust | Non-managed hooks must be reviewed and trusted in `/hooks`; trust is recorded **against the hook's hash**, so a changed hook is skipped until re-trusted. Project `.codex/` hooks load only in trusted projects. `--dangerously-bypass-hook-trust` skips this for one invocation. |
| Enable | `[features] hooks = true` by default; `codex_hooks` is a deprecated alias. `allow_managed_hooks_only` restricts to managed hooks. |
| Handlers | `type: "command"` and `"mcp_tool"` run; `prompt`/`agent` are parsed but skipped. `commandWindows` / `command_windows` is a Windows override. |
| Timeout | seconds; default **600**; `SessionEnd`/`Interrupt` default 1 s, max 3 s. |
| cwd | session `cwd`; docs recommend resolving repo hooks from `git rev-parse --show-toplevel`. |
| Concurrency | matching command hooks for an event run concurrently; `async: true` hooks cannot block. |
| Matcher | regex; `"*"`, `""` or omitted = all. `PreToolUse`/`PostToolUse` match `tool_name`; `apply_patch` also matches `Edit`/`Write`. |
| Coverage | Shell and unified exec (`Bash`), `apply_patch`, MCP tools, other local function tools. Hosted tools (`WebSearch`) are not covered. "Treat tool hooks as a useful guardrail, not a complete enforcement boundary." |

### 3.2 Exit codes and failure

| Case | Behaviour | Status |
|---|---|---|
| exit 0, no output | success, Codex continues | fact |
| exit 0, JSON | event-specific fields apply | fact |
| exit 2 + stderr | block (`PreToolUse`, `UserPromptSubmit`), feedback (`PostToolUse`), continue (`Stop`, `SubagentStop`) | fact |
| `PreToolUse` returns `continue`, `stopReason`, `suppressOutput`, `ask`, legacy `approve` | "marks the hook run as failed, reports the error, and continues the tool call" (fail-open) | fact |
| Managed MCP hook error/timeout/malformed response | "can fail the hook without blocking the tool" | fact (managed MCP hooks) |
| Command hook crash, other exit codes, timeout | **pending verification**; inference: fail-open, consistent with the above | inference |
| `Stop` / `SubagentStop` exit 0 with plain text | invalid; JSON required when stdout is non-empty | fact |

### 3.3 Events relevant to RepoRinse

| Event | Stage | Can veto? | Payload fields we need | Allow | Warn | Block |
|---|---|---|---|---|---|---|
| `PreToolUse` | before | **yes** | `session_id`, `cwd`, `hook_event_name`, `turn_id`, `tool_name` (`Bash`, `apply_patch`, `mcp__…`), `tool_use_id`, `tool_input.command` (Bash **and** apply_patch: the patch text), `permission_mode` | exit 0, no output | exit 0, `{"systemMessage": "…"}` (supported for `PreToolUse`); `hookSpecificOutput.additionalContext` adds model context | exit 0, `hookSpecificOutput.permissionDecision: "deny"` + `permissionDecisionReason`; or legacy `{"decision":"block","reason":…}`; or exit 2 + stderr |
| `PostToolUse` | after | no | + `tool_response` | exit 0, no output | `systemMessage`, `hookSpecificOutput.additionalContext` | `decision: "block"` replaces the tool result with feedback; does not undo side effects |
| `PermissionRequest` | before (approval) | yes (`decision.behavior: "deny"`) | `tool_name`, `tool_input` | — | — | not used by RepoRinse in v1 |
| `SessionStart` | lifecycle | no (`continue:false` ends the turn after compact) | `source` | exit 0 | plain stdout / `additionalContext` | n/a |
| `Stop` | lifecycle | "block" = continue working | `stop_hook_active`, `last_assistant_message` | exit 0, no output | `systemMessage` | `decision: "block"` creates a continuation prompt |

Inference: for `apply_patch`, file paths are not separate fields; the adapter must parse them from the patch text in
`tool_input.command`. The patch grammar is not documented on the hooks page → **pending verification** with a captured payload.

### 3.4 Verbatim fixtures

Payload. The hooks page has no verbatim `PreToolUse` example; the only full payload is `SessionEnd`
(https://learn.chatgpt.com/docs/hooks#sessionend):

```json
{
  "session_id": "thr_123",
  "transcript_path": "/workspace/.codex/rollout.jsonl",
  "cwd": "/workspace",
  "hook_event_name": "SessionEnd",
  "reason": "other"
}
```

The `PreToolUse` fixture must be **constructed** from the documented field table (common fields + `turn_id`, `tool_name`,
`tool_use_id`, `tool_input.command`, `permission_mode`) and labelled as constructed in `tests/fixtures/agents/`. The exact wire
schema lives in https://github.com/openai/codex/tree/main/codex-rs/hooks/schema/generated (the page warns that `main` may be
ahead of the release).

Registration, excerpt of the inline TOML example (https://learn.chatgpt.com/docs/hooks#config-shape):

```toml
[[hooks.PreToolUse]]
matcher = "^Bash$"

[[hooks.PreToolUse.hooks]]
type = "command"
command = '/usr/bin/python3 "$(git rev-parse --show-toplevel)/.codex/hooks/pre_tool_use_policy.py"'
timeout = 30
statusMessage = "Checking Bash command"
```

Deny output (https://learn.chatgpt.com/docs/hooks#pretooluse):

```json
{
  "hookSpecificOutput": {
    "hookEventName": "PreToolUse",
    "permissionDecision": "deny",
    "permissionDecisionReason": "Destructive command blocked by hook."
  }
}
```

Codex commit attribution: a `Co-authored-by` trailer naming `Codex` is reported in https://github.com/openai/codex/issues/14994
(user report, 2026-03-17). No attribution key appears in config-reference on 2026-10-26 → **pending verification**.

---

## 4. Gemini CLI

Sources (checked 2026-10-26):

- https://geminicli.com/docs/hooks/reference/ and https://geminicli.com/docs/hooks/ (rendered pages; text matches the repo
  sources `docs/hooks/reference.md` and `docs/hooks/index.md` in https://github.com/google-gemini/gemini-cli)
- `docs/hooks/writing-hooks.md`, `docs/hooks/best-practices.md`, `docs/reference/tools.md`, `docs/reference/configuration.md` (same repo)
- Latest release seen: `v0.67.0-nightly.20261025` (2026-10-25). No minimal version for hooks is stated.

### 4.1 Configuration

| Item | Fact |
|---|---|
| Files (precedence high→low) | `.gemini/settings.json` (project) → `~/.gemini/settings.json` (user) → `/etc/gemini-cli/settings.json` (system) → extensions. |
| Shape | `hooks.<Event>[] = { matcher?, sequential?, hooks: [ { type: "command", command, name?, timeout?, description? } ] }` |
| Matcher | tool events: regex on tool name; lifecycle events: exact string; `"*"`/`""` = all. MCP tools: `mcp_<server>_<tool>`. |
| Timeout | **milliseconds**, default 60000. |
| Toggle | `hooksConfig.enabled` (default `true`, restart required), `hooksConfig.disabled` (list of hook names). `/hooks panel`, `/hooks enable-all`, `/hooks disable <name>`. |
| Env | "sanitized environment": `GEMINI_PROJECT_DIR`, `GEMINI_PLANS_DIR`, `GEMINI_SESSION_ID`, `GEMINI_CWD`, `CLAUDE_PROJECT_DIR` alias. |
| Trust | Project hooks are "untrusted by default"; identity = `name` + `command`; first sight or change shows a warning, then the hook runs and is marked trusted. |
| Tool names / args | `run_shell_command` (`command`, `description`, `dir_path`, `is_background`), `write_file` (`file_path`, `content`), `replace` (`file_path`, `old_string`, `new_string`, `instruction`, `allow_multiple`), `read_file` (`file_path`, `start_line`, `end_line`). |

### 4.2 Exit codes and failure (fact)

| Exit | Meaning |
|---|---|
| 0 | stdout parsed as JSON; "Preferred for all logic", including blocks via `{"decision":"deny"}` |
| 2 | "System Block": action aborted, stderr is the reason |
| other | "Warning. A non-fatal failure occurred; the CLI continues" (fail-open) |
| non-JSON stdout | "parsing will fail. The CLI will default to 'Allow' and treat the entire output as a `systemMessage`" |
| timeout | **pending verification** (only the default and how to raise it are documented) |

### 4.3 Events relevant to RepoRinse

| Event | Stage | Can veto? | Payload fields we need | Allow | Warn | Block |
|---|---|---|---|---|---|---|
| `BeforeTool` | before | **yes** | `session_id`, `cwd`, `hook_event_name`, `timestamp`, `tool_name`, `tool_input.command`/`dir_path` (shell), `tool_input.file_path` (write/replace/read), `mcp_context?` | exit 0, `{"decision": "allow"}` | exit 0, `{"decision":"allow","systemMessage":"…"}` | exit 0, `{"decision":"deny","reason":"…"}` (reason goes to the agent as a tool error; turn continues); or exit 2 + stderr |
| `AfterTool` | after | no veto; `deny` **hides** the real tool output | + `tool_response` (`llmContent`, `returnDisplay`, `error?`) | exit 0, `{}` | `hookSpecificOutput.additionalContext`, `systemMessage` | do not use (see mapping) |
| `BeforeAgent` | lifecycle (prompt) | yes | `prompt` | not used | — | — |
| `AfterAgent` | lifecycle (turn end) | `deny` forces a retry | `prompt`, `prompt_response`, `stop_hook_active` | not used in v1 | — | — |
| `SessionStart` | lifecycle | no ("Startup is never blocked") | `source` (`startup`, `resume`, `clear`) | exit 0 | `systemMessage`, `additionalContext` | n/a |
| `SessionEnd` | lifecycle | no; CLI does not wait | `reason` | exit 0 | `systemMessage` | n/a |

### 4.4 Verbatim fixtures

Payload. The reference gives a schema, not an example (https://geminicli.com/docs/hooks/reference/#base-input-schema):

```typescript
{
  "session_id": string,      // Unique ID for the current session
  "transcript_path": string, // Absolute path to session transcript JSON
  "cwd": string,             // Current working directory
  "hook_event_name": string, // The firing event (for example "BeforeTool")
  "timestamp": string        // ISO 8601 execution time
}
```

`BeforeTool` adds `tool_name`, `tool_input`, `mcp_context`, `original_request_name`. The contract-test fixture must be
constructed from this schema plus the `run_shell_command` argument list, and labelled as constructed.

Registration (https://geminicli.com/docs/hooks/#configuration-schema):

```json
{
  "hooks": {
    "BeforeTool": [
      {
        "matcher": "write_file|replace",
        "hooks": [
          {
            "name": "security-check",
            "type": "command",
            "command": "$GEMINI_PROJECT_DIR/.gemini/hooks/security.sh",
            "timeout": 5000
          }
        ]
      }
    ]
  }
}
```

Deny output (writing-hooks guide, "Block secrets in commits"):

```json
{
  "decision": "deny",
  "reason": "Security Policy: Potential secret detected in content.",
  "systemMessage": "🔒 Security scanner blocked operation"
}
```

---

## 5. Cursor

Sources (checked 2026-10-26): https://cursor.com/docs/hooks (fetched `hooks.md`), https://cursor.com/docs/reference/third-party-hooks.
No minimal version is stated; the `cursor_version` field example is `"1.7.2"` (inference: not a minimum).

### 5.1 Configuration

| Item | Fact |
|---|---|
| Files | `<project>/.cursor/hooks.json` (needs a trusted workspace), `~/.cursor/hooks.json`; enterprise `/Library/Application Support/Cursor/hooks.json`, `/etc/cursor/hooks.json`, `C:\ProgramData\Cursor\hooks.json`; team (dashboard); plugins. |
| Merge | All matching hooks from all sources run; `deny` > `ask` > `allow` regardless of source; `user_message`/`agent_message` concatenated. Priority Enterprise → Team → Project → User. |
| Shape | `{ "version": 1, "hooks": { "<event>": [ { command, type?, timeout?, matcher?, failClosed?, loop_limit? } ] } }`; `version` is required. |
| cwd | project hooks run from the project root (use `.cursor/hooks/x.sh`); user hooks run from `~/.cursor/`. |
| Timeout | seconds; "platform default" (value not stated). |
| Windows | desktop app runs hook commands through PowerShell and pipes JSON in. |
| Matcher | regex; `preToolUse` against tool type (`Shell`, `Read`, `Write`, `Grep`, `Delete`, `Task`, `MCP:<tool>`); `beforeShellExecution` against the full command string. |
| Third-party | Cursor **also loads Claude Code hooks** from `.claude/settings.local.json`, `.claude/settings.json`, `~/.claude/settings.json` when "Include Third-Party Plugins, Skills, and Other Configs" is on (**default on**), mapping `PreToolUse`→`preToolUse`, `Bash`→`Shell`, `Edit`→`Write`, and accepting Claude's nested `hookSpecificOutput`. |

### 5.2 Exit codes and failure (fact)

| Case | Behaviour |
|---|---|
| exit 0 | use JSON output. For permission hooks (`beforeShellExecution`, `beforeMCPExecution`, `beforeReadFile`, `beforeTabFileRead`, `subagentStart`, `preToolUse`), **invalid JSON or a schema mismatch blocks the action** |
| exit 2 | block (= `permission: "deny"`) |
| other exit, crash, timeout | "Hook failed, action proceeds (fail-open by default)" |
| `failClosed: true` | crash, timeout, non-zero exit, **no output** block instead |

### 5.3 Events relevant to RepoRinse

| Event | Stage | Can veto? | Payload fields we need | Allow | Warn | Block |
|---|---|---|---|---|---|---|
| `beforeShellExecution` | before | **yes** (`allow`/`deny`/`ask`) | common: `conversation_id`, `generation_id`, `hook_event_name`, `cursor_version`, `workspace_roots`; event: `command`, `cwd`, `sandbox` | exit 0, `{"permission":"allow"}` | exit 0, `{"permission":"allow","user_message":"…","agent_message":"…"}` | exit 0, `{"permission":"deny","user_message":"…","agent_message":"…"}` |
| `preToolUse` | before | **yes** (`allow`/`deny`; `ask` "not enforced … today") | `tool_name`, `tool_input` (`Shell`: `command`, `working_directory`; other tools' shapes not documented), `tool_use_id`, `cwd` | `{"permission":"allow"}` | allow + messages (display on allow: inference) | `{"permission":"deny",…}` |
| `beforeReadFile` | before | yes | `file_path` (absolute), `content`, `attachments` | `{"permission":"allow"}` | — | `{"permission":"deny","user_message":"…"}` |
| `afterFileEdit` | after | no | `file_path` (absolute), `edits[]` (`old_string`, `new_string`) | exit 0 | no output fields documented | n/a |
| `stop` | lifecycle | no veto; `followup_message` auto-continues (loop_limit default 5) | `status`, `loop_count` | exit 0, `{}` | — | n/a |
| `sessionStart` | lifecycle | no (fire-and-forget) | `session_id`, `is_background_agent`, `composer_mode` | exit 0 | `additional_context` | n/a |

### 5.4 Verbatim fixtures

Payload (`preToolUse`), https://cursor.com/docs/hooks#pretooluse:

```json
{
  "tool_name": "Shell",
  "tool_input": { "command": "npm install", "working_directory": "/project" },
  "tool_use_id": "abc123",
  "cwd": "/project",
  "model": "claude-opus-4-7-thinking-max",
  "model_id": "claude-opus-4-7",
  "model_params": [
    { "id": "thinking", "value": "true" },
    { "id": "context", "value": "1m" },
    { "id": "effort", "value": "max" }
  ],
  "agent_message": "Installing dependencies..."
}
```

`beforeShellExecution` input (https://cursor.com/docs/hooks#beforeshellexecution--beforemcpexecution), placeholders as in the docs:

```json
{
  "command": "<full terminal command>",
  "cwd": "<current working directory>",
  "sandbox": false
}
```

Registration (https://cursor.com/docs/hooks#command-based-hooks):

```json
{
  "version": 1,
  "hooks": {
    "beforeShellExecution": [
      {
        "command": "./scripts/approve-network.sh",
        "timeout": 30,
        "matcher": "curl|wget|nc"
      }
    ]
  }
}
```

### 5.5 Cross-host interaction (fact + inference)

- Fact: Cursor runs hooks registered in `.claude/settings.json` by default.
- Inference: if RepoRinse is installed for both Claude Code and Cursor in one repo, Cursor runs RepoRinse twice per action, and
  the **Claude adapter receives Cursor-shaped payloads** (`cursor_version`, `tool_name: "Shell"`). See mapping rule M3.

---

## 6. Antigravity

Sources (checked 2026-10-26):

- https://antigravity.google/docs/hooks (JavaScript-rendered; read with a headless browser. `/docs/ide/hooks` redirects to `/docs/hooks?tab=ide`)
- https://antigravity.google/llms.txt (index lists "Hooks")
- Community report: https://discuss.ai.google.dev/t/do-antigravity-ide-2-0-actually-execute-plugin-hooks-pretooluse-posttooluse-or-is-that-cli-only-right-now/176814
  (2026-08-01 to 2026-08-06; independent reproduction states hooks execute only in the `agy` CLI 1.1.10, not in IDE 2.1.1 or the 2.0 desktop app 2.5.0)

### 6.1 Facts

| Item | Fact |
|---|---|
| Files | workspace `.agents/hooks.json`; global `~/.gemini/config/hooks.json`; CLI also plugin `hooks.json`. CLI: hooks in `~/.gemini/antigravity-cli/settings.json` are not loaded. |
| Shape | top-level keys are **hook names**; each maps to `enabled?` and event arrays. Tool events: `{ matcher, hooks: [ { type?, command, timeout? } ] }`; non-tool events: a list of handlers directly. |
| Events | `SessionStart`, `PreToolUse`, `PostToolUse`, `PreInvocation`, `PostInvocation`, `Stop` |
| Matcher | regex on tool name for tool events; ignored for others. |
| Timeout | seconds, default 30. |
| I/O | stdin JSON, stdout JSON, **camelCase** fields. Common: `conversationId`, `workspacePaths`, `transcriptPath`, `artifactDirectoryPath`, `modelName`. |
| `PreToolUse` input | `toolCall.name`, `toolCall.args`, `stepIdx`. `run_command` args: `CommandLine`, `Cwd`, `WaitMsBeforeAsync`; `write_to_file`/`replace_file_content`/`multi_replace_file_content`: `TargetFile`; `view_file`: `AbsolutePath`. |
| `PreToolUse` output | `decision` **required**: `allow`, `auto_approve`, `deny`, `ask`, `force_ask`, `deny_unless_prior_grant`; `reason?`, `overwrite?`, `permissionOverrides?` |
| `PostToolUse` output | `{}` |
| `Stop` output | `decision` required; `"continue"` keeps the agent running, "Any other value allows the stop". |
| Exit codes | **not documented** |
| Crash/timeout behaviour | **not documented** |
| Version | not stated on the page; forum reports IDE v2.1.1 |

### 6.2 Verbatim fixtures

Payload (`PreToolUse`), https://antigravity.google/docs/hooks:

```json
{
  "toolCall": {
    "name": "run_command",
    "args": {
      "CommandLine": "npm test",
      "Cwd": "/workspace/project",
      "WaitMsBeforeAsync": 5000
    }
  },
  "stepIdx": 19,
  "conversationId": "ec33ebf9-0cba-4100-8142-c61503f6c587",
  "workspacePaths": ["/workspace/project"],
  "transcriptPath": "~/.gemini/antigravity/brain/ec33ebf9-0cba-4100-8142-c61503f6c587/.system_generated/logs/transcript.jsonl",
  "artifactDirectoryPath": "~/.gemini/antigravity/brain/ec33ebf9-0cba-4100-8142-c61503f6c587",
  "modelName": "gemini-3.6-flash-medium"
}
```

Registration, excerpt (`safety-gate` entry) of the schema example on the same page:

```json
  "safety-gate": {
    "enabled": false,
    "PreToolUse": [
      {
        "matcher": "run_command",
        "hooks": [
          {
            "command": "./scripts/safety-check.sh"
          }
        ]
      }
    ]
  },
```

Verdict: **git-only by default.** An `agy` CLI adapter is possible as **contract-only/experimental** (opt-in, documented as
"exit codes and failure behaviour undocumented; IDE/App reported not to run hooks"). Never advertise blocking for Antigravity.

---

## 7. Mapping to RepoRinse (spec for the agent-adapters workstream)

Contract types: `NormalizedEvent { provider, original_event, stage: EventStage, action: EventAction, working_directory, paths,
command, session_id, tool_name }`, `HookDecision::{Allow, Warn, Block}`, `AdapterResponse { exit_code, stdout, stderr }`
(`src/core/model.rs`, `src/core/integration.rs`).

### 7.1 Rules for every adapter

| Id | Rule | Why |
|---|---|---|
| M1 | `AdapterResponse.exit_code` is the **host's** code, never `exit_codes::BLOCKED` (1). Exit 1 is a non-blocking error in Claude Code, a warning in Gemini, a fail-open failure in Cursor. | 2.2, 3.2, 4.2, 5.2 |
| M2 | Stdout is either empty or exactly one JSON object followed by `\n`; nothing else (no banners, no logs). Diagnostics go to stderr. Serialize with `serde_json`; never format JSON by hand. | Claude/Gemini parsing rules; Cursor blocks on invalid JSON |
| M3 | Detect foreign payloads: if the Claude adapter receives an object with `cursor_version`, delegate to the Cursor parser/renderer (Cursor accepts both formats). If a payload cannot be attributed, return **Allow** with empty stdout + stderr note (fail-open), except on Cursor permission hooks, where Allow must be `{"permission":"allow"}`. | 5.5 |
| M4 | Never render "approve/auto-approve" semantics. `Allow` means "no objection; the host's normal permission flow continues". | Claude `permissionDecision: "allow"` skips the permission prompt; Antigravity `auto_approve` likewise |
| M5 | Block on an `After` stage event degrades to Warn (the action already happened). Never use outputs that hide or replace tool results (Gemini `AfterTool` deny, Claude `updatedToolOutput`, Codex `PostToolUse` `continue:false`). | destructive to the agent's information |
| M6 | Lifecycle events never Block in v1 (Stop "block" means "keep working" and can loop). Warn only. | 2.3, 3.3, 4.3 |
| M7 | Messages: one line per finding `rule-id: path:line message`, max 20 findings then `… and N more`, total ≤ 2,000 chars, secrets redacted before rendering. | Claude 10k cap, Codex 2.5k-token spill to disk |
| M8 | Installers register an **absolute path** to the `reporinse` binary, exec form where the host supports it (Claude `command`+`args`), and an explicit short timeout in the host's unit (Claude s, Codex s, Gemini **ms**, Cursor s, Antigravity s). Default 10 s. | Codex default 600 s; Gemini sanitized env |
| M9 | Fail-open is the default; fail-closed is opt-in per provider where documented: Claude `"onFailure": "block"` (v2.1.295+), Cursor `"failClosed": true`. Codex, Gemini, Antigravity: no documented fail-closed switch → report "fail-closed unavailable". | 10. of the spec |
| M10 | Recursion guard: the adapter sets `REPORINSE_HOOK_DEPTH` for any child process and exits Allow immediately when it is already ≥ 1. RepoRinse never runs `git commit`/`git push` from a hook. | threat model T9 |

### 7.2 Event mapping

| Provider | Host event (matcher) | `EventStage` | `EventAction` | `command` | `paths` | `working_directory` | `session_id` | `tool_name` |
|---|---|---|---|---|---|---|---|---|
| claude | `PreToolUse` (`Bash\|PowerShell`) | Before | ShellCommand | `tool_input.command` | — | `cwd` | `session_id` | `tool_name` |
| claude | `PreToolUse` (`Write\|Edit`) | Before | FileWrite | — | `[tool_input.file_path]` | `cwd` | `session_id` | `tool_name` |
| claude | `PreToolUse` (`Read`) | Before | FileRead | — | `[tool_input.file_path]` | `cwd` | `session_id` | `tool_name` |
| claude | `PostToolUse` (`Write\|Edit`) | After | FileWrite | — | `[tool_input.file_path]` | `cwd` | `session_id` | `tool_name` |
| claude | `SessionStart` | Lifecycle | SessionStart | — | — | `cwd` | `session_id` | — |
| claude | `Stop` | Lifecycle | SessionStop | — | — | `cwd` | `session_id` | — |
| codex | `PreToolUse` (`^Bash$`) | Before | ShellCommand | `tool_input.command` | — | `cwd` | `session_id` | `tool_name` |
| codex | `PreToolUse` (`^apply_patch$`) | Before | FileWrite | — | parsed from patch text (pending) | `cwd` | `session_id` | `apply_patch` |
| codex | `PostToolUse` (`^apply_patch$`) | After | FileWrite | — | parsed from patch text (pending) | `cwd` | `session_id` | `apply_patch` |
| codex | `SessionStart` | Lifecycle | SessionStart | — | — | `cwd` | `session_id` | — |
| codex | `Stop` | Lifecycle | SessionStop | — | — | `cwd` | `session_id` | — |
| gemini | `BeforeTool` (`run_shell_command`) | Before | ShellCommand | `tool_input.command` | — | `tool_input.dir_path` else `cwd` | `session_id` | `tool_name` |
| gemini | `BeforeTool` (`write_file\|replace`) | Before | FileWrite | — | `[tool_input.file_path]` | `cwd` | `session_id` | `tool_name` |
| gemini | `BeforeTool` (`read_file`) | Before | FileRead | — | `[tool_input.file_path]` | `cwd` | `session_id` | `tool_name` |
| gemini | `AfterTool` (`write_file\|replace`) | After | FileWrite | — | `[tool_input.file_path]` | `cwd` | `session_id` | `tool_name` |
| gemini | `SessionStart` / `SessionEnd` | Lifecycle | SessionStart / SessionStop | — | — | `cwd` | `session_id` | — |
| cursor | `beforeShellExecution` | Before | ShellCommand | `command` | — | `cwd` | `conversation_id` | `"Shell"` |
| cursor | `beforeReadFile` | Before | FileRead | — | `[file_path]` | `workspace_roots[0]` | `conversation_id` | `"Read"` |
| cursor | `afterFileEdit` | After | FileWrite | — | `[file_path]` | `workspace_roots[0]` | `conversation_id` | `"Write"` |
| cursor | `preToolUse` (`Write`) | Before | FileWrite | — | **pending** (Write `tool_input` shape undocumented) | `cwd` | `conversation_id` | `tool_name` |
| cursor | `stop` | Lifecycle | SessionStop | — | — | `workspace_roots[0]` | `conversation_id` | — |
| cursor | `sessionStart` | Lifecycle | SessionStart | — | — | `workspace_roots[0]` | `session_id` | — |
| antigravity (experimental) | `PreToolUse` (`run_command`) | Before | ShellCommand | `toolCall.args.CommandLine` | — | `toolCall.args.Cwd` | `conversationId` | `toolCall.name` |
| antigravity (experimental) | `PreToolUse` (`write_to_file\|replace_file_content\|multi_replace_file_content`) | Before | FileWrite | — | `[toolCall.args.TargetFile]` | `workspacePaths[0]` | `conversationId` | `toolCall.name` |

Any other host event or tool name → `EventAction::Unknown`, decision Allow, no output (or `{"permission":"allow"}` on Cursor
permission hooks, `{"decision":"allow"}` on Gemini/Antigravity before-tool hooks).

### 7.3 Exact rendering of `HookDecision`

`<msg>` is the M7 message, JSON-escaped by the serializer. `stderr` is always empty unless stated.

**claude**

| Event | Allow | Warn | Block |
|---|---|---|---|
| `PreToolUse` | exit 0, stdout empty | exit 0, `{"systemMessage":"<msg>","hookSpecificOutput":{"hookEventName":"PreToolUse","additionalContext":"<msg>"}}` | exit 0, `{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"<msg>"}}` |
| `PostToolUse` | exit 0, empty | exit 0, `{"systemMessage":"<msg>","hookSpecificOutput":{"hookEventName":"PostToolUse","additionalContext":"<msg>"}}` | = Warn (M5) |
| `Stop` | exit 0, empty | exit 0, `{"systemMessage":"<msg>"}` | = Warn (M6) |
| `SessionStart` | exit 0, empty | exit 0, `{"systemMessage":"<msg>"}` | = Warn |

Note: Block via exit 2 is equivalent but exit 0 + JSON is preferred (structured, survives the Windows-without-Git-Bash rule).

**codex**

| Event | Allow | Warn | Block |
|---|---|---|---|
| `PreToolUse` | exit 0, empty | exit 0, `{"systemMessage":"<msg>"}` | exit 0, `{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"<msg>"}}` |
| `PostToolUse` | exit 0, empty | exit 0, `{"systemMessage":"<msg>","hookSpecificOutput":{"hookEventName":"PostToolUse","additionalContext":"<msg>"}}` | = Warn (M5) |
| `Stop`, `SessionStart` | exit 0, empty | exit 0, `{"systemMessage":"<msg>"}` | = Warn (M6) |

Never emit `continue`, `stopReason`, `suppressOutput` or `permissionDecision:"ask"` on `PreToolUse` (the run is marked failed).
Inference to confirm by contract test: combining `systemMessage` with `hookSpecificOutput.additionalContext` in one
`PostToolUse` object (both are documented for that event, separately).

**gemini**

| Event | Allow | Warn | Block |
|---|---|---|---|
| `BeforeTool` | exit 0, `{"decision":"allow"}` | exit 0, `{"decision":"allow","systemMessage":"<msg>"}` | exit 0, `{"decision":"deny","reason":"<msg>","systemMessage":"<msg>"}` |
| `AfterTool` | exit 0, `{}` | exit 0, `{"systemMessage":"<msg>","hookSpecificOutput":{"additionalContext":"<msg>"}}` | = Warn (M5) |
| `SessionStart`, `SessionEnd` | exit 0, `{}` | exit 0, `{"systemMessage":"<msg>"}` | = Warn (M6) |

Inference: the `hookSpecificOutput` for Gemini has no `hookEventName` in the docs; do not add one.

**cursor**

| Event | Allow | Warn | Block |
|---|---|---|---|
| `beforeShellExecution` | exit 0, `{"permission":"allow"}` | exit 0, `{"permission":"allow","user_message":"<msg>","agent_message":"<msg>"}` | exit 0, `{"permission":"deny","user_message":"<msg>","agent_message":"<msg>"}` |
| `preToolUse` | exit 0, `{"permission":"allow"}` | exit 0, `{"permission":"allow","user_message":"<msg>","agent_message":"<msg>"}` (display on allow: inference) | exit 0, `{"permission":"deny","user_message":"<msg>","agent_message":"<msg>"}` |
| `beforeReadFile` | exit 0, `{"permission":"allow"}` | = Allow (no warn channel documented) | exit 0, `{"permission":"deny","user_message":"<msg>"}` |
| `afterFileEdit` | exit 0, empty | exit 0, empty; `<msg>` on stderr (Hooks output channel; inference) | = Warn (M5) |
| `stop`, `sessionStart` | exit 0, `{}` | exit 0, `{}`; `<msg>` on stderr | = Warn (M6) |

Never emit `followup_message` (auto-submits a user message). Warn could alternatively use `"permission":"ask"` on
`beforeShellExecution`; that turns a warning into a human approval and is **not** the default.

**antigravity (experimental, opt-in)**

| Event | Allow | Warn | Block |
|---|---|---|---|
| `PreToolUse` | exit 0, `{"decision":"allow"}` | exit 0, `{"decision":"allow","reason":"<msg>"}` (whether `reason` shows on allow: inference) | exit 0, `{"decision":"deny","reason":"<msg>"}` |
| `PostToolUse` | exit 0, `{}` | exit 0, `{}` | = Warn (M5) |

Exit codes other than 0 must not be used (undocumented). `Stop` is not registered.

**git** (not an `Adapter`; for completeness)

| Hook | Allow | Warn | Block |
|---|---|---|---|
| `pre-commit`, `commit-msg`, `pre-push` | exit 0 | exit 0, findings on stderr | exit `exit_codes::BLOCKED` (1) or any non-zero, findings on stderr |

### 7.4 Contract tests the adapters workstream must add

1. Parse every verbatim fixture above into `NormalizedEvent` and assert each field of 7.2.
2. Golden-file test of each cell in 7.3 (exact bytes, including trailing `\n`).
3. Property test: for any `Finding` text (quotes, newlines, control chars, 100 kB), stdout is valid JSON and ≤ the M7 cap.
4. Foreign payload test (M3): a Cursor `preToolUse` fixture fed to the Claude adapter yields a Cursor-valid response.
5. Malformed / empty / 10 MB / deeply nested stdin → Allow (fail-open) with a sanitized stderr reason, never a panic.
6. Pending items (`apply_patch` grammar, Cursor `Write` input, Gemini timeout, Codex crash semantics, git rebase hooks) stay
   `planned` until a captured real payload or live test exists.


## Implementation verification addendum — 2026-10-26

Claude/Codex/Gemini/Cursor parsing and decisions have contract tests in tests/adapter_contracts.rs.
Installer restoration and drift checks are covered in tests/adapter_install.rs. Generated handler commands are
executed in tests/adapter_e2e.rs, including binary paths with spaces, accents, apostrophes and `$`.
This is synthetic protocol/subprocess verification, not a captured live agent session.

Timeout units and tool events follow official pages re-opened during delivery. Gemini uses regex `.*` and neutral
output `{}`. Cursor permission hooks require `permission: allow` as the neutral result. Claude and Codex never
receive explicit allow decisions. Native Antigravity and undocumented patch/Write paths remain unverified.
