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

#[derive(Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FileConfig {
    version: Option<u32>,
    preset: Option<String>,
    exclude: Option<Vec<String>>,
    #[serde(default)]
    providers: BTreeMap<String, bool>,
    #[serde(default)]
    rules: BTreeMap<String, toml::Table>,
}

fn problem(file: Option<&Path>, message: impl Into<String>) -> ConfigError {
    ConfigError {
        file: file.map(|p| p.display().to_string()),
        message: message.into(),
    }
}

fn read_layer(path: &Path) -> Result<Option<FileConfig>, ConfigError> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(problem(Some(path), e.to_string())),
    };
    let layer: FileConfig =
        toml::from_str(&text).map_err(|e| problem(Some(path), e.to_string()))?;
    if layer.version.is_some_and(|v| v != 1) {
        return Err(problem(
            Some(path),
            "unsupported configuration version (expected 1)",
        ));
    }
    if let Some(preset) = &layer.preset {
        validate_preset(preset).map_err(|e| problem(Some(path), e))?;
    }
    Ok(Some(layer))
}

pub fn validate_preset(preset: &str) -> Result<(), String> {
    if matches!(preset, "minimal" | "balanced" | "strict" | "custom") {
        Ok(())
    } else {
        Err(format!("unknown preset '{preset}'"))
    }
}

fn validate_globs(patterns: &[String]) -> Result<(), String> {
    for pattern in patterns {
        globset::GlobBuilder::new(pattern)
            .literal_separator(true)
            .build()
            .map_err(|e| format!("invalid exclusion '{pattern}': {e}"))?;
    }
    Ok(())
}

fn user_file() -> Option<std::path::PathBuf> {
    use std::env::var_os;
    use std::path::PathBuf;
    if let Some(dir) = var_os("REPORINSE_CONFIG_HOME") {
        return Some(PathBuf::from(dir).join("config.toml"));
    }
    if let Some(dir) = var_os("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(dir).join("reporinse/config.toml"));
    }
    if cfg!(windows)
        && let Some(dir) = var_os("APPDATA")
    {
        return Some(PathBuf::from(dir).join("reporinse/config.toml"));
    }
    var_os("HOME")
        .or_else(|| var_os("USERPROFILE"))
        .map(|dir| PathBuf::from(dir).join(".config/reporinse/config.toml"))
}

fn merge_layer(
    config: &mut EffectiveConfig,
    registry: &RuleRegistry,
    layer: FileConfig,
    path: &Path,
) -> Result<(), ConfigError> {
    let origin = path.display().to_string();
    if let Some(exclude) = layer.exclude {
        validate_globs(&exclude).map_err(|e| problem(Some(path), e))?;
        config.exclude = exclude;
        config.origins.insert("exclude".into(), origin.clone());
    }
    for (provider, enabled) in layer.providers {
        if !matches!(
            provider.as_str(),
            "git" | "claude" | "codex" | "gemini" | "cursor" | "antigravity"
        ) {
            return Err(problem(
                Some(path),
                format!("unknown provider '{provider}'"),
            ));
        }
        config
            .origins
            .insert(format!("providers.{provider}"), origin.clone());
        config.providers.insert(provider, enabled);
    }
    for (id, mut table) in layer.rules {
        let rule = registry
            .find(&id)
            .ok_or_else(|| problem(Some(path), format!("unknown rule '{id}'")))?;
        let mut settings = config.settings(&id);
        if let Some(mode) = table.remove("mode") {
            let mode = mode
                .as_str()
                .and_then(RuleMode::parse)
                .ok_or_else(|| problem(Some(path), format!("invalid mode for '{id}'")))?;
            if !rule.metadata().supports(mode) {
                return Err(problem(Some(path), format!("unsupported mode for '{id}'")));
            }
            settings.mode = mode;
            config
                .origins
                .insert(format!("rules.{id}.mode"), origin.clone());
        }
        if let Some(exclude) = table.remove("exclude") {
            settings.exclude = exclude
                .try_into()
                .map_err(|e| problem(Some(path), format!("invalid exclude for '{id}': {e}")))?;
            validate_globs(&settings.exclude).map_err(|e| problem(Some(path), e))?;
            config
                .origins
                .insert(format!("rules.{id}.exclude"), origin.clone());
        }
        for (key, value) in table {
            config
                .origins
                .insert(format!("rules.{id}.{key}"), origin.clone());
            settings.options.insert(key, value);
        }
        let errors = rule.validate_options(&settings.options);
        if !errors.is_empty() {
            return Err(problem(Some(path), format!("{id}: {}", errors.join("; "))));
        }
        config.rules.insert(id, settings);
    }
    Ok(())
}

/// Loads built-ins, preset, user and repository layers, then CLI overrides.
pub fn load(
    registry: &RuleRegistry,
    repo_root: Option<&Path>,
    overrides: &CliOverrides,
) -> Result<EffectiveConfig, ConfigError> {
    let mut layers = Vec::new();
    let paths = user_file()
        .into_iter()
        .chain(repo_root.map(|p| p.join(REPO_FILE)));
    for path in paths {
        if let Some(layer) = read_layer(&path)? {
            layers.push((path, layer));
        }
    }
    let mut config = EffectiveConfig::built_in(registry);
    let (preset, origin) = overrides
        .preset
        .as_ref()
        .map(|p| (p.clone(), "cli".to_string()))
        .or_else(|| {
            layers.iter().rev().find_map(|(path, layer)| {
                layer
                    .preset
                    .as_ref()
                    .map(|p| (p.clone(), path.display().to_string()))
            })
        })
        .unwrap_or_else(|| ("balanced".into(), "built-in".into()));
    validate_preset(&preset).map_err(|e| problem(None, e))?;
    for rule in registry.all() {
        let m = rule.metadata();
        let mode = match preset.as_str() {
            "custom" => RuleMode::Off,
            "minimal" if m.priority != super::model::Priority::P0 => RuleMode::Off,
            "strict" if m.supports(RuleMode::Block) => RuleMode::Block,
            _ => m.default_mode,
        };
        config.set(
            m.id,
            RuleSettings::with_mode(mode),
            &format!("preset:{preset}"),
        );
    }
    config.preset = preset;
    config.origins.insert("preset".into(), origin);
    for (path, layer) in layers {
        merge_layer(&mut config, registry, layer, &path)?;
    }
    for (id, mode) in &overrides.modes {
        let rule = registry
            .find(id)
            .ok_or_else(|| problem(None, format!("unknown rule '{id}'")))?;
        if !rule.metadata().supports(*mode) {
            return Err(problem(None, format!("unsupported mode for '{id}'")));
        }
        let mut settings = config.settings(id);
        settings.mode = *mode;
        config.set(id, settings, "cli");
    }
    Ok(config)
}
