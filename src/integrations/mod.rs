//! Provider integrations. Each provider exposes only what is implemented and verified.
pub mod agent_adapter;
pub mod agent_install;
pub mod bridge_install;

pub mod antigravity;
pub mod claude;
pub mod codex;
pub mod cursor;
pub mod gemini;
pub mod git;

use crate::core::integration::{Adapter, Installer};

pub const PROVIDERS: &[&str] = &[
    git::PROVIDER,
    claude::PROVIDER,
    codex::PROVIDER,
    gemini::PROVIDER,
    cursor::PROVIDER,
    antigravity::PROVIDER,
    "opencode",
    "deepseek",
];

pub fn installers() -> Vec<Box<dyn Installer>> {
    [
        git::installer(),
        claude::installer(),
        codex::installer(),
        gemini::installer(),
        cursor::installer(),
        antigravity::installer(),
        Some(Box::new(bridge_install::BridgeInstaller("opencode")) as Box<dyn Installer>),
        Some(Box::new(bridge_install::BridgeInstaller("deepseek")) as Box<dyn Installer>),
    ]
    .into_iter()
    .flatten()
    .collect()
}

pub fn adapters() -> Vec<Box<dyn Adapter>> {
    [
        Some(Box::new(agent_adapter::HostAdapter("opencode")) as Box<dyn Adapter>),
        Some(Box::new(agent_adapter::HostAdapter("deepseek")) as Box<dyn Adapter>),
        claude::adapter(),
        codex::adapter(),
        gemini::adapter(),
        cursor::adapter(),
    ]
    .into_iter()
    .flatten()
    .collect()
}

pub fn find_installer(provider: &str) -> Option<Box<dyn Installer>> {
    installers()
        .into_iter()
        .find(|i| i.provider().eq_ignore_ascii_case(provider))
}

pub fn find_adapter(provider: &str) -> Option<Box<dyn Adapter>> {
    adapters()
        .into_iter()
        .find(|a| a.provider().eq_ignore_ascii_case(provider))
}
