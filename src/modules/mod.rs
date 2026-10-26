//! Every rule that ships in the binary. Each category keeps its own list so modules stay independent.

pub mod agent;
pub mod code;
pub mod docs;
pub mod git;
pub mod security;
pub mod text;

use crate::core::registry::RuleRegistry;
use crate::core::rule::Rule;

pub fn built_in() -> Vec<Box<dyn Rule>> {
    let mut all = Vec::new();
    all.extend(git::rules());
    all.extend(text::rules());
    all.extend(docs::rules());
    all.extend(code::rules());
    all.extend(security::rules());
    all.extend(agent::rules());
    all
}

pub fn registry() -> RuleRegistry {
    RuleRegistry::new(built_in())
}

#[cfg(test)]
mod tests {
    #[test]
    fn ids_are_unique_and_well_formed() {
        let registry = super::registry();
        for rule in registry.all() {
            let m = rule.metadata();
            assert!(
                m.id.starts_with(&format!("{}.", m.category)),
                "{} must start with its category",
                m.id
            );
            assert!(
                m.id.chars()
                    .all(|c| c.is_ascii_lowercase() || c == '.' || c == '-'),
                "bad id {}",
                m.id
            );
            assert!(
                m.supports(m.default_mode),
                "{} default mode not supported",
                m.id
            );
        }
    }
}
