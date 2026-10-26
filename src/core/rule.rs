//! The detector contract every module implements.

use super::model::{
    ChangeSet, NormalizedEvent, RuleMetadata, RuleMode, RuleResult, Severity, SourceDocument,
};

/// Effective settings of one rule after built-ins, preset, user, repo and CLI layers are merged.
#[derive(Debug, Clone, PartialEq)]
pub struct RuleSettings {
    pub mode: RuleMode,
    /// Rule-specific options: the `[rules."<id>"]` table minus `mode` and `exclude`.
    pub options: toml::Table,
    /// Extra glob patterns excluded for this rule only.
    pub exclude: Vec<String>,
}

impl RuleSettings {
    pub fn with_mode(mode: RuleMode) -> RuleSettings {
        RuleSettings {
            mode,
            options: toml::Table::new(),
            exclude: Vec::new(),
        }
    }

    pub fn str(&self, name: &str) -> Option<&str> {
        self.options.get(name).and_then(|v| v.as_str())
    }

    pub fn bool(&self, name: &str) -> Option<bool> {
        self.options.get(name).and_then(|v| v.as_bool())
    }

    pub fn int(&self, name: &str) -> Option<i64> {
        self.options.get(name).and_then(|v| v.as_integer())
    }

    pub fn strings(&self, name: &str) -> Vec<String> {
        self.options
            .get(name)
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Severity a finding gets under the current mode (`Fix` reports as a warning until applied).
    pub fn severity(&self) -> Severity {
        match self.mode {
            RuleMode::Block => Severity::Error,
            RuleMode::Off => Severity::Info,
            RuleMode::Warn | RuleMode::Fix => Severity::Warning,
        }
    }

    pub fn wants_fix(&self) -> bool {
        self.mode == RuleMode::Fix
    }
}

/// Everything a rule may look at. Read-only: rules never touch the filesystem or Git themselves.
#[derive(Debug, Clone, Copy)]
pub struct RuleContext<'a> {
    pub settings: &'a RuleSettings,
    /// Set for `TargetKind::CommitMessage` and `TargetKind::File`.
    pub document: Option<&'a SourceDocument>,
    /// Set for `TargetKind::ChangeSet`.
    pub changes: Option<&'a ChangeSet>,
    /// Set for `TargetKind::AgentEvent`.
    pub event: Option<&'a NormalizedEvent>,
}

impl<'a> RuleContext<'a> {
    pub fn for_document(
        settings: &'a RuleSettings,
        document: &'a SourceDocument,
    ) -> RuleContext<'a> {
        RuleContext {
            settings,
            document: Some(document),
            changes: None,
            event: None,
        }
    }

    pub fn for_changes(settings: &'a RuleSettings, changes: &'a ChangeSet) -> RuleContext<'a> {
        RuleContext {
            settings,
            document: None,
            changes: Some(changes),
            event: None,
        }
    }

    pub fn for_event(settings: &'a RuleSettings, event: &'a NormalizedEvent) -> RuleContext<'a> {
        RuleContext {
            settings,
            document: None,
            changes: None,
            event: Some(event),
        }
    }
}

/// A pure detector. `analyze` must be deterministic, side-effect free and fast; any change goes into
/// `RuleResult::fixes`, and only when the settings are in `Fix` mode.
pub trait Rule: Send + Sync {
    fn metadata(&self) -> &RuleMetadata;

    /// Called once per target whose kind is in `metadata().targets`.
    fn analyze(&self, ctx: &RuleContext<'_>) -> RuleResult;

    /// Validates rule options; returns human-readable problems (used by `config validate`).
    fn validate_options(&self, _options: &toml::Table) -> Vec<String> {
        Vec::new()
    }
}
