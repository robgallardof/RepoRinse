# Threat model

Scope: the `reporinse` CLI, its Git hook shims, agent hook adapters and installers (as specified in
`docs/architecture/plan.md`). Date: 2026-10-26. Host behaviour cited here is documented in
[`docs/research/compatibility-matrix.md`](../research/compatibility-matrix.md).

## Statement of limits

**Local hooks are not a security boundary.** Git hooks are skipped by `git commit --no-verify`, `git push --no-verify`,
`git -c core.hooksPath=/dev/null`, a missing executable bit, or simply another clone. Agent hooks are skipped by
`disableAllHooks` (Claude Code), `[features] hooks = false` and untrusted-hook review (Codex), `hooksConfig.enabled`
(Gemini), untrusted workspaces (Cursor), hosts that do not execute them (Antigravity IDE/App, per community report), tool
paths the host does not route through hooks (Claude `@` file mentions, Codex hosted tools), and shell commands that write files
without the edit tool. Every host defaults to **fail-open** on hook crash or timeout. RepoRinse is a hygiene and early-warning
tool. Enforcement belongs to CI, branch protection and server-side hooks. Documentation and CLI output must never call a
local block "secure", "guaranteed" or a "sandbox".

## Assets

| Asset | Why it matters |
|---|---|
| A1 Working tree and index content | Uncommitted work can be destroyed by a bad fix; it may not exist anywhere else. |
| A2 Git history and commit metadata | Human co-authors, sign-offs, licences must survive; history is never rewritten. |
| A3 Git and agent configuration (`core.hooksPath`, existing hooks, Husky/Lefthook, `settings.json`, `hooks.json`, `config.toml`) | Breaking it breaks the user's other tooling or silently disables their own guards. |
| A4 Secrets that pass through RepoRinse (env, command lines in agent payloads, file contents) | Logs, diagnostics and hook output can leak them (Codex spills large hook output to disk). |
| A5 Files outside the repository | Path tricks must never let a fix or installer write there. |
| A6 Availability of the user's workflow | A hang or crash inside a hook blocks commits or agent turns. |

## Trust boundaries and inputs

| Boundary | Input | Trust |
|---|---|---|
| B1 Repository content | files, `.reporinse.toml`, `.editorconfig`, `.gitattributes`, `.claude/`, `.cursor/`, `.codex/`, `.gemini/`, `.agents/` | **Untrusted** (a cloned repo is attacker-controlled). |
| B2 Agent host → adapter | stdin JSON payload (command text, paths, transcripts paths) | **Untrusted** (model-generated, possibly prompt-injected). |
| B3 Git → hook shim | argv, stdin ref lines, environment (`GIT_DIR`, `GIT_INDEX_FILE`, …), message file | Semi-trusted (Git), but content is user/agent-authored. |
| B4 User configuration | `$REPORINSE_CONFIG_HOME` / XDG / `%APPDATA%` config, CLI flags | Trusted as the user's intent, still validated. |
| B5 Effective hooks directory | `core.hooksPath` from system/global/local/worktree scope; may point anywhere, relative per worktree | Untrusted location; may be shared with other tools. |
| B6 RepoRinse manifest | record of files/config RepoRinse installed (hash, previous state) | Trusted only after hash verification; may be edited by others. |

## Threats and mitigations

| Id | Threat | Mitigations (required) | Test owner |
|---|---|---|---|
| T1 | **Path traversal** in fixes or installers: `..`, absolute paths, Windows drive-relative (`C:foo`), UNC (`\\server\share`), `\\?\` prefixes, alternate data streams (`file:stream`), reserved names (`CON`, `NUL`), trailing dots/spaces, mixed separators. | Canonicalize every write target and require it to be inside the canonical repo root (or the specific host config dir for installers); reject UNC/device/ADS forms outright; compare case-insensitively on Windows/macOS; never build paths from agent payloads for writing (agent paths are read-only data). | security-reviewer |
| T2 | **Symlink/junction escape**: a tracked symlink, or a directory swapped for a link between check and write (TOCTOU). | Do not follow symlinks on write; open with no-follow semantics where available; re-verify file identity (hash + metadata) immediately before replace; write via temp file in the same directory + atomic rename; refuse when the target is a link, a junction, or a hardlink count > 1. | security-reviewer |
| T3 | **Destructive fixes**: fix corrupts encoding/BOM/EOL, breaks multi-line strings or Markdown, touches binary/generated/vendor files, is not idempotent. | Fixes are plans applied by `planning::apply` with the analysed hash; refuse if the file changed; detect binary (NUL byte / invalid UTF-8 policy) and skip; preserve BOM and EOL style; property test `fix(fix(x)) == fix(x)`; `check` is the default, `fix` is explicit. | core-rules-engine |
| T4 | **Staged vs unstaged corruption**: fixing the working tree while the index holds different content, or staging other changes. | In hooks read content **from the index**; if a path has both staged and unstaged changes, do not auto-fix (warn with instructions); never run `git add -A`/`git add .`; restage only paths RepoRinse itself fixed and only when the working copy equalled the index before the fix. | git-hooks-specialist |
| T5 | **Clobbering foreign hooks/config**: overwriting `core.hooksPath`, Husky/Lefthook hooks, truncating `settings.json`, duplicating entries. | Detect `core.hooksPath` at all scopes and `git rev-parse --git-path hooks`; never set or override `core.hooksPath` silently (exit `CONFLICT`); chain with explicit order; JSON/TOML merges are structural (parse, add own entry, preserve unknown keys and order where the format allows), idempotent, with a manifest (version, previous content hash, date); uninstall removes only manifest-owned entries whose hash still matches. | git-hooks-specialist, agent-adapters |
| T6 | **Secrets in logs/output**: commands like `curl -H "Authorization: …"`, `export AWS_SECRET…`, URLs with credentials, `.env` contents echoed in findings, debug logs, or hook stdout that Codex spills to disk. | Never log raw payloads; redact before rendering (token patterns, `key=value` for secret-ish keys, URL userinfo, high-entropy strings); truncate excerpts; no telemetry, no network; debug logs opt-in and redacted too. | security-reviewer |
| T7 | **Malicious `.reporinse.toml`** (B1): code execution, huge files, pathological globs/regexes, includes outside the repo, unknown keys. | Config is data only: no commands, hooks, plugins, includes or environment interpolation; size limit (e.g. 256 KiB) and depth limit; unknown keys are errors; globs compiled with a bounded engine (`globset`), count limit (e.g. 1,000 patterns) and length limit; regexes, if ever allowed, use the linear-time `regex` crate with size limits; repo config cannot enable `fix` modes for paths outside the repo or relax user-level `block` settings without being reported in `status`. | core-rules-engine |
| T8 | **JSON payload abuse** (B2): oversized stdin, deep nesting, duplicate keys, invalid UTF-8, wrong types, NUL bytes, prompt-injected instructions in command text. | Read stdin with a hard cap (e.g. 1 MiB) and timeout; `serde_json` with typed structs and a depth limit; unknown/malformed → fail-open Allow with sanitized stderr (or `{"permission":"allow"}` for Cursor permission hooks); command text and paths are **data, never executed or interpreted as instructions**; no shell is spawned from payload content. | agent-adapters |
| T9 | **Recursion hook → CLI → hook**: RepoRinse runs `git` inside a Git hook, or a host runs RepoRinse twice (Cursor also executes `.claude/settings.json` hooks). | Hooks never run `git commit`/`git push`/`git merge`; read-only Git calls clear inherited `GIT_*` only when targeting another repo; set `REPORINSE_HOOK_DEPTH` and exit Allow when ≥ 1; detect Cursor payloads in the Claude adapter (matrix M3); installers warn when both Cursor and Claude entries exist. | agent-adapters, git-hooks-specialist |
| T10 | **Bypass** via `--no-verify`, `core.hooksPath=/dev/null`, disabled host hooks, alternate agent paths (`@` mentions, shell writes instead of edit tools, hosted tools, subagents, other clones). | Accepted residual risk: document it; respect `--no-verify`; do not abuse `prepare-commit-msg` (not suppressed by `--no-verify`) to override the user's explicit bypass; offer the same rules in `reporinse check` for CI where they can be enforced. | documentation-ux |
| T11 | **Hook latency / DoS** (A6): huge repos, giant files, many findings, slow hosts with long default timeouts (Codex 600 s). | Process only staged/changed files in hooks; per-file size cap; bounded output (matrix M7); installers set explicit short timeouts in each host's unit; no network. | core-rules-engine, agent-adapters |
| T12 | **Hook shim injection**: shim scripts built with unquoted paths (spaces, `$`, quotes, Unicode) or relying on `PATH` resolution that an attacker can shadow (repo-local `node_modules/.bin`, current directory on Windows). | Shims call an absolute path to the binary; quote with a tested quoting routine per shell (sh, PowerShell); exec-form registration where the host supports it (Claude `command`+`args`); LF line endings for sh shims; no `eval`. | git-hooks-specialist |
| T13 | **Malicious host config in a cloned repo** (`.claude/settings.json`, `.cursor/hooks.json`, `.codex/`, `.gemini/settings.json`, `.agents/hooks.json` shipping their own hooks). | Out of RepoRinse's control (hosts gate it with trust prompts: Codex hash trust, Gemini fingerprint warning, Cursor trusted workspace). RepoRinse installers must never mark anything trusted on the user's behalf and must never execute commands found in these files during inspection or `doctor`. | agent-adapters |
| T14 | **Manifest tampering** (B6): edited manifest makes uninstall delete foreign files. | Uninstall removes a file or entry only when its current hash equals the manifest hash recorded at install; otherwise report drift and stop (`CONFLICT`). | git-hooks-specialist |
| T15 | **Attribution false positives** (A2): removing a human co-author named "Claude" or a legal attribution. | Match trailer key + known bot e-mail/exact generator line only; never by name; never touch `Signed-off-by`, licence headers, `Reviewed-by`. | git-hooks-specialist |

## Fail-open vs fail-closed policy

| Situation | Default | Rationale |
|---|---|---|
| RepoRinse internal error, bad payload, timeout in an agent hook | fail-open (Allow, message on stderr) | Hosts already default to fail-open; a crashing guard must not stop the user's work. |
| Config invalid in a Git hook | block with `CONFIG_ERROR` (3) and a clear message, bypassable with `--no-verify` | The user asked for checks; silently skipping hides misconfiguration. |
| Opt-in fail-closed | Claude `onFailure: "block"`, Cursor `failClosed: true` only | The only documented switches; other hosts report "fail-closed unavailable". |

## Out of scope

Server-side enforcement, malware detection, secret scanning as a security control, sandboxing agent commands, verifying
natural-language claims of agents.
