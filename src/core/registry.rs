//! Lookup of the rules compiled into the binary.

use std::collections::BTreeMap;

use super::rule::Rule;

pub struct RuleRegistry {
    by_id: BTreeMap<&'static str, Box<dyn Rule>>,
}

impl RuleRegistry {
    /// Panics on duplicate ids: that is a programming error caught by the registry unit test.
    pub fn new(rules: Vec<Box<dyn Rule>>) -> RuleRegistry {
        let mut by_id = BTreeMap::new();
        for rule in rules {
            let id = rule.metadata().id;
            assert!(by_id.insert(id, rule).is_none(), "duplicate rule id: {id}");
        }
        RuleRegistry { by_id }
    }

    /// All rules sorted by id.
    pub fn all(&self) -> impl Iterator<Item = &dyn Rule> {
        self.by_id.values().map(|r| r.as_ref())
    }

    pub fn find(&self, id: &str) -> Option<&dyn Rule> {
        self.by_id.get(id).map(|r| r.as_ref())
    }

    pub fn contains(&self, id: &str) -> bool {
        self.by_id.contains_key(id)
    }

    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }
}
