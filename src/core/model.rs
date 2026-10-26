//! Plain data shared by every layer: rule metadata, findings, fix plans and the inputs rules look at.

use serde::{Deserialize, Serialize};

/// How a rule behaves once enabled. `Fix` only applies deterministic, verified edits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuleMode {
    Off,
    Warn,
    Block,
    Fix,
}

impl RuleMode {
    pub fn as_str(self) -> &'static str {
        match self {
            RuleMode::Off => "off",
            RuleMode::Warn => "warn",
            RuleMode::Block => "block",
            RuleMode::Fix => "fix",
        }
    }

    pub fn parse(s: &str) -> Option<RuleMode> {
        match s {
            "off" => Some(RuleMode::Off),
            "warn" => Some(RuleMode::Warn),
            "block" => Some(RuleMode::Block),
            "fix" => Some(RuleMode::Fix),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warning,
    Error,
}

/// Risk of enabling the rule; `--all` never enables a `High` risk rule in a modifying mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Priority {
    P0,
    P1,
    P2,
}

/// Whether the rule actually ships. Only `Implemented` rules may be advertised as available.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Availability {
    Implemented,
    Experimental,
    Planned,
}

/// What a rule can inspect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TargetKind {
    /// A commit message (commit-msg hook or `check --message`).
    CommitMessage,
    /// A file's content: working tree or, when `SourceDocument::staged`, the index.
    File,
    /// A whole change set: paths with status and line counts.
    ChangeSet,
    /// A normalized event coming from an agent host.
    AgentEvent,
}

/// Static description of a rule. One rule is one installable module (e.g. `git.ai-attribution`).
#[derive(Debug, Clone, Serialize)]
pub struct RuleMetadata {
    /// Stable id, `category.name`. Never renamed once released.
    pub id: &'static str,
    /// Rule behaviour version; bump when findings or fixes change.
    pub version: &'static str,
    /// git, text, code, docs, security or agent.
    pub category: &'static str,
    /// One sentence shown by `modules list`.
    pub summary: &'static str,
    pub risk: RiskLevel,
    pub priority: Priority,
    pub availability: Availability,
    /// Modes the rule accepts besides `Off` (always accepted).
    pub modes: &'static [RuleMode],
    /// Mode used by the `balanced` preset when the user says nothing.
    pub default_mode: RuleMode,
    pub targets: &'static [TargetKind],
    /// Providers that can trigger it: `git`, `claude`, `codex`, `gemini`, `cursor`, `cli`.
    pub providers: &'static [&'static str],
}

impl RuleMetadata {
    pub fn supports(&self, mode: RuleMode) -> bool {
        mode == RuleMode::Off || self.modes.contains(&mode)
    }

    /// Relative path of the rule documentation inside the repository.
    pub fn docs_path(&self) -> String {
        format!("docs/rules/{}.md", self.id)
    }
}

/// Something a rule observed. Coordinates are 1-based and only set when known exactly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Finding {
    pub rule_id: String,
    /// Derived from the effective mode: warn → Warning, block → Error.
    pub severity: Severity,
    /// Short, actionable; never contains secrets or whole file contents.
    pub message: String,
    /// Repository-relative path with forward slashes; `None` for commit messages / events.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    /// 1-based column counted in Unicode scalar values (chars).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<usize>,
    /// What was matched / why it matters (sanitized).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// True when the same analysis produced a `FixPlan` covering it.
    pub fixable: bool,
}

impl Finding {
    pub fn new(rule_id: &str, severity: Severity, message: impl Into<String>) -> Finding {
        Finding {
            rule_id: rule_id.to_string(),
            severity,
            message: message.into(),
            path: None,
            line: None,
            column: None,
            reason: None,
            fixable: false,
        }
    }

    pub fn at(mut self, path: &str, line: Option<usize>, column: Option<usize>) -> Finding {
        self.path = Some(path.to_string());
        self.line = line;
        self.column = column;
        self
    }

    pub fn reason(mut self, reason: impl Into<String>) -> Finding {
        self.reason = Some(reason.into());
        self
    }

    pub fn fixable(mut self) -> Finding {
        self.fixable = true;
        self
    }
}

/// Replace `len` bytes at byte offset `start` of the analysed UTF-8 text. Offsets must be char boundaries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TextEdit {
    pub start: usize,
    pub len: usize,
    pub replacement: String,
}

/// A proposed, not yet applied, change. Detectors only build plans; `core::planning` validates and applies them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FixPlan {
    pub rule_id: String,
    /// Path of the file, or `COMMIT_EDITMSG` for a commit message.
    pub target: String,
    /// What the change does, for previews.
    pub description: String,
    /// Non-overlapping edits against the analysed text.
    pub edits: Vec<TextEdit>,
    /// SHA-256 hex of the analysed text; the applier refuses to write if it changed.
    pub precondition_hash: String,
}

/// Output of one rule on one target.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct RuleResult {
    pub findings: Vec<Finding>,
    pub fixes: Vec<FixPlan>,
    /// Non-finding notes (skipped binary file, unsupported encoding…).
    pub diagnostics: Vec<String>,
}

impl RuleResult {
    pub fn empty() -> RuleResult {
        RuleResult::default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum NewlineStyle {
    None,
    Lf,
    Crlf,
    Mixed,
}

/// How the text was stored, so a fix can write it back byte-compatible.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TextFormat {
    /// Only `utf-8` is decoded and fixable today; anything else is reported and skipped.
    pub encoding: &'static str,
    pub has_bom: bool,
    pub newlines: NewlineStyle,
    pub ends_with_newline: bool,
}

impl TextFormat {
    pub fn detect(text: &str, has_bom: bool) -> TextFormat {
        let bytes = text.as_bytes();
        let (mut crlf, mut lf) = (0usize, 0usize);
        for (i, b) in bytes.iter().enumerate() {
            if *b == b'\n' {
                if i > 0 && bytes[i - 1] == b'\r' {
                    crlf += 1;
                } else {
                    lf += 1;
                }
            }
        }
        let newlines = match (crlf, lf) {
            (0, 0) => NewlineStyle::None,
            (_, 0) => NewlineStyle::Crlf,
            (0, _) => NewlineStyle::Lf,
            _ => NewlineStyle::Mixed,
        };
        TextFormat {
            encoding: "utf-8",
            has_bom,
            newlines,
            ends_with_newline: text.ends_with('\n'),
        }
    }
}

/// Immutable view of a text a rule analyses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceDocument {
    pub kind: TargetKind,
    /// Repository-relative path with forward slashes, or `COMMIT_EDITMSG`.
    pub path: String,
    /// Decoded text without BOM.
    pub text: String,
    pub format: TextFormat,
    /// True when `text` comes from the Git index, not the working tree.
    pub staged: bool,
}

impl SourceDocument {
    pub const COMMIT_MESSAGE_PATH: &'static str = "COMMIT_EDITMSG";

    pub fn commit_message(text: impl Into<String>) -> SourceDocument {
        let text = text.into();
        let format = TextFormat::detect(&text, false);
        SourceDocument {
            kind: TargetKind::CommitMessage,
            path: Self::COMMIT_MESSAGE_PATH.into(),
            text,
            format,
            staged: false,
        }
    }

    pub fn file(
        path: impl Into<String>,
        text: impl Into<String>,
        has_bom: bool,
        staged: bool,
    ) -> SourceDocument {
        let text = text.into();
        let format = TextFormat::detect(&text, has_bom);
        SourceDocument {
            kind: TargetKind::File,
            path: path.into(),
            text,
            format,
            staged,
        }
    }

    /// Extension in lower case without the dot, or empty.
    pub fn extension(&self) -> String {
        std::path::Path::new(&self.path)
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ChangeStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
    TypeChanged,
    Unknown,
}

/// One path of a change set (staged diff, commit range or agent edit batch).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChangedPath {
    pub path: String,
    pub status: ChangeStatus,
    pub added_lines: usize,
    pub deleted_lines: usize,
    pub binary: bool,
}

/// Input for rules that look at a change as a whole (`TargetKind::ChangeSet`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChangeSet {
    pub paths: Vec<ChangedPath>,
    pub staged: bool,
}

/// When the host fires the event relative to the action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EventStage {
    /// Before the action runs; the host may honour a veto.
    Before,
    /// After the action ran; only reporting or follow-up is possible.
    After,
    /// Session lifecycle (start, stop, compact…).
    Lifecycle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum EventAction {
    Unknown,
    ShellCommand,
    FileRead,
    FileWrite,
    SessionStart,
    SessionStop,
}

/// Provider-independent view of an agent hook event. Adapters build it from the host JSON;
/// rules never see the raw payload. Command text and paths are data, never executed by RepoRinse.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NormalizedEvent {
    /// claude, codex, gemini, cursor.
    pub provider: String,
    /// Host event name, e.g. `PreToolUse`, `beforeShellExecution`.
    pub original_event: String,
    pub stage: EventStage,
    pub action: EventAction,
    pub working_directory: Option<String>,
    /// Files the action touches, as reported by the host.
    pub paths: Vec<String>,
    /// Shell command text for `EventAction::ShellCommand`.
    pub command: Option<String>,
    pub session_id: Option<String>,
    /// Host tool name, e.g. `Bash`, `Write`.
    pub tool_name: Option<String>,
}

/// Process exit codes. Documented in docs/configuration/exit-codes.md; never change their meaning.
pub mod exit_codes {
    /// Nothing blocking was found (warnings may have been printed).
    pub const OK: i32 = 0;
    /// At least one finding from a rule in `block` mode.
    pub const BLOCKED: i32 = 1;
    /// Wrong command line usage (clap also uses 2).
    pub const USAGE: i32 = 2;
    /// Configuration missing, invalid or unreadable.
    pub const CONFIG_ERROR: i32 = 3;
    /// Installation conflict needing a human decision (foreign hook, core.hooksPath, drifted file).
    pub const CONFLICT: i32 = 4;
    /// Feature recognised but not implemented in this build.
    pub const NOT_IMPLEMENTED: i32 = 69;
    /// Unexpected internal error.
    pub const INTERNAL: i32 = 70;
}
