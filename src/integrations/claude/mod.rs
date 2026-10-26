//! `claude` integration. Returns `None` until it is implemented and verified against the official contract
//! (docs/research/compatibility-matrix.md).

use crate::core::integration::{Adapter, Installer};

pub const PROVIDER: &str = "claude";

pub fn installer() -> Option<Box<dyn Installer>> {
    None
}

pub fn adapter() -> Option<Box<dyn Adapter>> {
    None
}
