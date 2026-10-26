# Problem evidence

Recurring problems RepoRinse addresses, with verifiable sources (all links opened on **2026-10-26**; GitHub issue
metadata read from the GitHub REST API the same day). For each problem: facts with links, inferences labelled as such,
and the **measurable signal** a deterministic rule can use. RepoRinse never tries to decide whether text "was written by AI";
it detects observable properties the user configured.

Note on sources: GitHub issues are user reports. They prove the complaint exists and recurs, not its root cause or frequency.

## 1. AI attribution trailers and footers

| Source | What it shows (fact) |
|---|---|
| https://code.claude.com/docs/en/settings-reference (`attribution`) | Claude Code adds `Co-Authored-By: <name> <noreply@anthropic.com>` to commits by default (`<name>` = model, `Claude`, or `Claude Code`), `🤖 Generated with [Claude Code](https://claude.com/claude-code)` to PR descriptions, and a `Claude-Session` trailer from cloud/Remote Control sessions. `attribution: false` (v2.1.281+) or empty strings disable them. |
| https://github.com/anthropics/claude-code/issues/29999 (closed, opened 2026-03-02) | "Co-Authored-By default is inappropriate — stop branding user work". |
| https://github.com/anthropics/claude-code/issues/7666 (closed, opened 2025-09-15) | "includeCoAuthoredBy setting does not prevent hardcoded Claude Code attribution in commit messages". |
| https://github.com/anthropics/claude-code/issues/7422 (closed, opened 2025-09-11) | Attribution added "despite explicit project instructions not to". |
| https://github.com/openai/codex/issues/14994 (open, opened 2026-03-17) | Codex's trailer names `Codex` as co-author (feature request to include the model). |
| https://aider.chat/docs/git.html | aider appends "(aider)" to the **author/committer name** by default and can add a `Co-authored-by` trailer (`--attribute-co-authored-by`). |

Inferences: settings are per machine/scope, so trailers still leak from other machines, older versions, cloud sessions and
other tools; a repository-side check is the only control the repo owner has.

Measurable signal (rule `git.ai-attribution`):

- Trailer block lines (`git interpret-trailers` semantics) whose key is `Co-authored-by`/`Co-Authored-By` (case-insensitive)
  **and** whose e-mail is on a known-bot list (`noreply@anthropic.com`, documented vendor addresses), or `Claude-Session:`.
- Footer lines exactly matching known generator phrases (`Generated with [Claude Code](…)`, with or without the emoji).
- Never match on the display name alone (a human co-author called "Claude" must survive), never rewrite author/committer
  metadata or history (aider's "(aider)" is reported, not fixed).

## 2. Trailing whitespace, blank-line and line-ending noise

| Source | What it shows (fact) |
|---|---|
| https://git-scm.com/docs/githooks (`pre-commit`) | Git's own sample pre-commit hook "prevents the introduction of … lines with trailing whitespace". |
| https://git-scm.com/docs/git-diff (`--check`) | `git diff --check` warns when changes introduce "whitespace errors" as defined by `core.whitespace` (by default trailing whitespace and space-before-tab in the indent). |
| https://editorconfig.org/ | `trim_trailing_whitespace`, `insert_final_newline`, `end_of_line` are the cross-editor convention for these choices. |
| https://github.com/anthropics/claude-code/issues/100296 (open, 2026-10-07) and https://github.com/anthropics/claude-code/issues/2805 (open, 2025-07-01) | Agent edit tools silently strip trailing whitespace; files created with CRLF on Linux. Whitespace changes can be wrong in both directions. |

Inference: whitespace "fixes" are themselves a source of noise and breakage (Markdown hard breaks, diff fixtures, heredocs),
so fixes must be scoped to changed lines and respect `.editorconfig`.

Measurable signal (rule `text.whitespace` and relatives): on **added/changed lines only**: trailing `[ \t]+$`; runs of more than
N consecutive blank lines; missing/extra final newline; a line ending that differs from the file's majority EOL. Excluded:
binary, generated, vendored, `.md` lines ending in two spaces when configured, files where `.editorconfig` says otherwise.

## 3. Invisible and bidirectional Unicode ("Trojan Source")

| Source | What it shows (fact) |
|---|---|
| https://nvd.nist.gov/vuln/detail/CVE-2021-42574 (published 2021-11-01) | Bidi control sequences let source "render different logic than the logical ordering of tokens ingested by compilers and interpreters". |
| https://nvd.nist.gov/vuln/detail/CVE-2021-42694 | Homoglyph identifiers that "render visually identical to a target identifier". |
| https://trojansource.codes/ | The original Trojan Source research and examples. |
| https://github.blog/changelog/2021-10-31-warning-about-bidirectional-unicode-text/ | GitHub added a UI warning for bidi text in files. |
| https://www.unicode.org/reports/tr9/ | UAX #9 defines the bidi controls (embeddings, overrides, isolates) and HL4 tailoring. |

Inference: invisible characters also arrive by copy/paste from chat UIs and model output (zero-width spaces, NBSP, soft hyphens);
this is a hygiene problem even without an attacker.

Measurable signal (rule `text.invisible-unicode`): presence of code points U+202A–U+202E, U+2066–U+2069, U+200E/U+200F,
U+061C (bidi); U+200B–U+200D, U+2060, U+FEFF not at offset 0, U+00AD (invisible). Report file, line, column and code point
name. Allowlist per path/locale for legitimate RTL text. Homoglyph detection is out of scope for P0.

## 4. Leftover agent plans, notes and summary Markdown

| Source | What it shows (fact) |
|---|---|
| https://forum.cursor.com/t/why-does-claude-in-cursor-keep-creating-markdown-work-summary-files/150420 | Users report agents creating "summary"/"report" Markdown files after small changes. |
| https://github.com/champi-ai/champi-tts/issues/78 (2026-10-21) | Cleanup of `SETUP_COMPLETE.md`, `IMPROVEMENTS.md`, `IMPLEMENTATION_SUMMARY.md` left at the repo root. |
| https://github.com/Logixia/logixia/issues/36 (2026-10-18) | Cleanup of `IMPLEMENTATION_SUMMARY.md` ("one-off 'implementation complete' note"), stale `todo/` files. |
| https://github.com/ChristianHandy/FleetPilot/issues/170 (2026-10-18) | Moving `IMPLEMENTATION_NOTES.md`, `IMPLEMENTATION_SUMMARY.md`, … out of the root. |
| https://code.claude.com/docs/en/hooks (`ExitPlanMode`) | Claude Code writes plans to a file on disk (`planFilePath`); Gemini exposes `GEMINI_PLANS_DIR` (https://geminicli.com/docs/hooks/). |

Inference: the three GitHub issues do not state that an agent created the files; the file-name pattern matches what users
describe for agents, but RepoRinse must treat it as a naming/location policy, not as AI attribution.

Measurable signal (rule `docs.agent-artifacts`): a **newly added** (staged or untracked) Markdown file whose path is outside
configured documentation roots and whose name matches a configurable list (`PLAN*.md`, `*SUMMARY*.md`, `IMPLEMENTATION_*.md`,
`NOTES.md`, `TODO*.md`, `SETUP_COMPLETE.md`), and that no tracked file links to. Default action: warn; never delete.

## 5. Obvious / redundant comments

| Source | What it shows (fact) |
|---|---|
| https://github.com/anthropics/claude-code/issues/65961 (open, 2026-06-07) | "Claude verbose code comments by default — ignores instructions to stop". |
| https://github.com/anthropics/claude-code/issues/61305 (closed, 2026-05-21) | "Generated code ignores repeated zero-comment instructions". |

Inference: "obvious" is subjective; a deterministic rule can only flag strong redundancy and must default to `off`/`warn`
with no automatic fix.

Measurable signal (rule `code.redundant-comment`, P1): on added lines only, a line comment immediately followed by a code line
where the comment's normalized word set (lowercase, stemmed identifiers split on case/underscore) is a subset of the code
line's tokens and the comment has ≤ 6 words. Never touches doc comments, licenses, `TODO`/`SAFETY`/`NOTE` markers.

## 6. Scope creep and unexpected dependencies

| Source | What it shows (fact) |
|---|---|
| https://arxiv.org/abs/2406.10279 | "We Have a Package for You! A Comprehensive Analysis of Package Hallucinations by Code Generating LLMs": models recommend packages that do not exist. |
| https://socket.dev/blog/slopsquatting-how-ai-hallucinations-are-fueling-a-new-class-of-supply-chain-attacks | Attackers register hallucinated package names ("slopsquatting"). |
| https://github.com/anthropics/claude-code/issues/101683 (open, 2026-10-13) | "Agent installed an end-of-life dependency with 37 published CVEs after a failed registry lookup". |
| https://github.com/anthropics/claude-code/issues/103805 (open, 2026-10-20) | Small task; agent "built unrequested infrastructure … installed software and read files far outside the project". |

Inference: RepoRinse cannot judge whether a dependency is wanted; it can make the change visible at commit time.

Measurable signal (rule `deps.manifest-change`, P1): staged diff touches a dependency manifest or lockfile (`Cargo.toml`
`[dependencies]`, `package.json` `dependencies`/`devDependencies`, `requirements*.txt`, `pyproject.toml`, `go.mod`, `*.csproj`
`PackageReference`); report added/removed package names. Scope signal: changed paths outside a configured `allowed_paths`
list. No registry lookups (no network at runtime).

## 7. Agents claiming tests passed without evidence

| Source | What it shows (fact) |
|---|---|
| https://github.com/anthropics/claude-code/issues/1501 (closed, 2025-06-02) | "Claude Code Reports False Test Results and Actions". |
| https://github.com/anthropics/claude-code/issues/22507 (closed, 2026-02-02) | "Claude fabricated test results in PR description without running tests". |
| https://github.com/anthropics/claude-code/issues/46940 (closed, 2026-04-12) | "Claude fabricates test results - reports ALL PASSED when tests are FAILING". |

Inference: a local tool cannot verify a natural-language claim. It can only check that evidence exists and is fresh.

Measurable signal (P2, opt-in): when a commit message or PR body file contains configured claim phrases ("all tests pass",
"tests passed"), require a configured evidence artifact (e.g. JUnit XML or `cargo test` log path) whose mtime is newer than the
newest staged source change and whose content reports zero failures; otherwise warn. RepoRinse itself reports only tests it ran
(spec section 10: never mark as PASSED what did not run).
