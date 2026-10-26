//! `git` integration: hook shims, the dispatcher behind `reporinse hook git <name>`, and the installer.
//! Contract and behaviour: docs/integrations/git.md.

pub mod command;
pub mod discovery;
pub mod hooks;
pub mod installer;
pub mod manifest;
pub mod shim;
pub mod staged;
pub mod transaction;

use crate::core::integration::{Adapter, Installer};

pub const PROVIDER: &str = "git";

pub fn installer() -> Option<Box<dyn Installer>> {
    Some(Box::new(installer::GitInstaller::new()))
}

/// Git hooks are not an agent protocol; there is no adapter.
pub fn adapter() -> Option<Box<dyn Adapter>> {
    None
}
pub mod staged_fix;
