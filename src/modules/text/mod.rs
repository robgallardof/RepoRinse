//! Conservative text rules.
pub mod invisible_unicode;
pub mod whitespace;
use crate::core::rule::Rule;
pub fn rules() -> Vec<Box<dyn Rule>> {
    vec![
        Box::new(whitespace::Whitespace),
        Box::new(invisible_unicode::InvisibleUnicode),
    ]
}
