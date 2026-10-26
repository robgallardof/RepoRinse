use crate::core::integration::{Adapter, Installer};
pub const PROVIDER: &str = "codex";
pub fn installer() -> Option<Box<dyn Installer>> {
    Some(Box::new(super::agent_install::AgentInstaller(PROVIDER)))
}
pub fn adapter() -> Option<Box<dyn Adapter>> {
    Some(Box::new(super::agent_adapter::HostAdapter(PROVIDER)))
}
