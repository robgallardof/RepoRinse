//! `git.*` rules. Add each rule in its own file and list it in [`rules`].

pub mod agents;
pub mod ai_attribution;

use crate::core::rule::Rule;

pub fn rules() -> Vec<Box<dyn Rule>> {
    vec![Box::new(ai_attribution::AiAttribution)]
}
