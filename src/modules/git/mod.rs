//! `git.*` rules. Add each rule in its own file and list it in [`rules`].

pub mod agents;
pub mod ai_attribution;
pub mod change_rules;

use crate::core::rule::Rule;

pub fn rules() -> Vec<Box<dyn Rule>> {
    vec![
        Box::new(ai_attribution::AiAttribution),
        Box::new(change_rules::ChangeRule(true)),
        Box::new(change_rules::ChangeRule(false)),
    ]
}
