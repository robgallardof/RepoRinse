//! Contracts for provider integrations: agent hook adapters and installers.

use std::path::PathBuf;

use serde::Serialize;

use super::model::{Finding, NormalizedEvent};

/// Decision returned to an agent host. Adapters map it to what the host really supports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HookDecision {
    Allow,
    Warn,
    Block,
}

/// What the adapter writes back to the host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdapterResponse {
    /// Process exit code expected by the host for this decision.
    pub exit_code: i32,
    /// Exact stdout payload (often JSON), or empty.
    pub stdout: String,
    /// Message on stderr, or empty.
    pub stderr: String,
}

/// Translates one host's hook protocol to `NormalizedEvent` and back.
pub trait Adapter: Send + Sync {
    /// claude, codex, gemini, cursor.
    fn provider(&self) -> &'static str;

    /// Parses the host's stdin payload. Err carries a sanitized reason.
    fn parse(&self, payload: &str) -> Result<NormalizedEvent, String>;

    /// Renders a decision with the findings that caused it, in the host's exact protocol.
    fn render(
        &self,
        event: &NormalizedEvent,
        decision: HookDecision,
        findings: &[Finding],
    ) -> AdapterResponse;
}

/// Where an integration is installed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum InstallScope {
    /// Only the current repository / project.
    Repo,
    /// The current user (Git global config, agent user settings). CLI flag value: `user` (alias `global`).
    User,
}

/// Request coming from `reporinse install/uninstall`.
#[derive(Debug, Clone)]
pub struct InstallRequest {
    pub scope: InstallScope,
    /// Repository root for `Repo`; working directory otherwise.
    pub root: PathBuf,
    /// Rule ids selected with `--only`/`--all`, already resolved and validated.
    pub modules: Vec<String>,
    /// Plan only, write nothing.
    pub dry_run: bool,
    /// Allow replacing files that drifted from the manifest, after explicit user consent.
    pub force: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum InstallActionKind {
    CreateFile,
    UpdateFile,
    DeleteFile,
    RestoreFile,
    SetConfig,
    UnsetConfig,
    MergeJson,
    Skip,
}

/// One step of an install/uninstall plan, shown before anything is written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InstallAction {
    pub kind: InstallActionKind,
    pub target: String,
    pub description: String,
}

/// Outcome of planning or applying.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct InstallResult {
    /// Steps planned (dry-run) or performed.
    pub actions: Vec<InstallAction>,
    /// When non-empty, nothing was written.
    pub conflicts: Vec<String>,
    /// Limitations, next steps.
    pub notes: Vec<String>,
}

impl InstallResult {
    pub fn succeeded(&self) -> bool {
        self.conflicts.is_empty()
    }
}

/// Status of one provider integration, used by `status` and `doctor`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IntegrationStatus {
    pub provider: String,
    pub scope: InstallScope,
    /// The host tool exists on this machine / project.
    pub detected: bool,
    /// RepoRinse integration is present.
    pub installed: bool,
    /// Modules the integration currently runs.
    pub modules: Vec<String>,
    /// Drift, conflicts or misconfiguration found.
    pub problems: Vec<String>,
}

/// Installs RepoRinse into one provider. Must be idempotent and only remove what its manifest says it owns.
pub trait Installer: Send + Sync {
    fn provider(&self) -> &'static str;

    fn supported_scopes(&self) -> &'static [InstallScope];

    fn status(&self, scope: InstallScope, root: &std::path::Path) -> IntegrationStatus;

    fn install(&self, request: &InstallRequest) -> InstallResult;

    fn uninstall(&self, request: &InstallRequest) -> InstallResult;
}
