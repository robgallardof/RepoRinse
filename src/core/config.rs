//! Configuration loading, presets and precedence (CLI > repo > user > preset > built-ins).
//!
//! FROZEN API (other workstreams code against it): `CliOverrides`, `EffectiveConfig::{built_in, settings,
//! is_excluded, origin}`, `load`, `ConfigError`. The core-rules-engine workstream owns the implementation:
//! reading `.reporinse.toml` and the user file, presets, validation and origins.

use std::collections::BTreeMap;
use std::path::Path;

use super::model::RuleMode;
use super::registry::RuleRegistry;
use super::rule::RuleSettings;

/// Name of the repository configuration file, at the repository root.
pub const REPO_FILE: &str = ".reporinse.toml";

/// Values given on the command line; highest precedence.
#[derive(Debug, Clone, Default)]
pub struct CliOverrides {
    /// Force a preset instead of the configured one.
    pub preset: Option<String>,
    /// Per-rule mode overrides (`--mode rule=warn` style), applied last.
    pub modes: BTreeMap<String, RuleMode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError {
    /// File the problem comes from, if any.
    pub file: Option<String>,
    pub message: String,
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.file {
            Some(file) => write!(f, "{file}: {}", self.message),
            None => write!(f, "{}", self.message),
        }
    }
}

impl std::error::Error for ConfigError {}

/// Fully resolved configuration.
#[derive(Debug, Clone)]
pub struct EffectiveConfig {
    pub preset: String,
    /// Global exclusion globs (repository-relative, forward slashes).
    pub exclude: Vec<String>,
    /// Providers enabled in the config (`[providers]`).
    pub providers: BTreeMap<String, bool>,
    rules: BTreeMap<String, RuleSettings>,
    /// `"rules.<id>.mode"` / `"exclude"` … → where the value came from (`built-in`, `preset:balanced`, a path, `cli`).
    origins: BTreeMap<String, String>,
}

impl EffectiveConfig {
    /// Every rule at its `default_mode`, no exclusions: the `balanced` preset before any file is read.
    pub fn built_in(registry: &RuleRegistry) -> EffectiveConfig {
        let mut rules = BTreeMap::new();
        let mut origins = BTreeMap::new();
        for rule in registry.all() {
            let m = rule.metadata();
            rules.insert(m.id.to_string(), RuleSettings::with_mode(m.default_mode));
            origins.insert(format!("rules.{}.mode", m.id), "built-in".to_string());
        }
        EffectiveConfig {
            preset: "balanced".to_string(),
            exclude: Vec::new(),
            providers: BTreeMap::from([("git".to_string(), true)]),
            rules,
            origins,
        }
    }

    /// Effective settings for a rule; unknown ids are `Off`.
    pub fn settings(&self, rule_id: &str) -> RuleSettings {
        self.rules
            .get(rule_id)
            .cloned()
            .unwrap_or_else(|| RuleSettings::with_mode(RuleMode::Off))
    }

    /// Replaces a rule's settings (used by loaders and tests).
    pub fn set(&mut self, rule_id: &str, settings: RuleSettings, origin: &str) {
        self.origins
            .insert(format!("rules.{rule_id}.mode"), origin.to_string());
        self.rules.insert(rule_id.to_string(), settings);
    }

    /// Where a value came from, e.g. `origin("rules.text.whitespace.mode")`.
    pub fn origin(&self, key: &str) -> Option<&str> {
        self.origins.get(key).map(String::as_str)
    }

    pub fn origins(&self) -> &BTreeMap<String, String> {
        &self.origins
    }

    /// True when `path` (repository-relative, forward slashes) is excluded globally or for this rule.
    pub fn is_excluded(&self, rule_id: &str, path: &str) -> bool {
        let rule_exclude = self
            .rules
            .get(rule_id)
            .map(|s| s.exclude.as_slice())
            .unwrap_or(&[]);
        self.exclude
            .iter()
            .chain(rule_exclude)
            .any(|pattern| glob_matches(pattern, path))
    }
}

fn glob_matches(pattern: &str, path: &str) -> bool {
    globset::GlobBuilder::new(pattern)
        .literal_separator(true)
        .build()
        .map(|g| g.compile_matcher().is_match(path))
        .unwrap_or(false)
}

/// Loads the effective configuration for `repo_root` (or only user/built-ins when `None`).
/// Current implementation: built-ins + CLI overrides; file layers land with the core-rules-engine workstream.
pub fn load(
    registry: &RuleRegistry,
    _repo_root: Option<&Path>,
    overrides: &CliOverrides,
) -> Result<EffectiveConfig, ConfigError> {
    let mut config = EffectiveConfig::built_in(registry);
    if let Some(preset) = &overrides.preset {
        config.preset = preset.clone();
    }
    for (id, mode) in &overrides.modes {
        if !registry.contains(id) {
            return Err(ConfigError {
                file: None,
                message: format!("unknown rule '{id}'"),
            });
        }
        let mut settings = config.settings(id);
        settings.mode = *mode;
        config.set(id, settings, "cli");
    }
    Ok(config)
}
