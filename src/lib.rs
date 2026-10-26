//! RepoRinse — Keep the code. Rinse the noise.
//!
//! Layers (see docs/architecture/overview.md):
//! - [`core`]: contracts, configuration, engine, fix planning and filesystem safety. No I/O in detectors.
//! - [`modules`]: the rules, one folder per category.
//! - [`integrations`]: Git hooks and agent hook adapters + their installers.
//! - [`cli`]: the `reporinse` command line.

pub mod cli;
pub mod core;
pub mod integrations;
pub mod modules;
