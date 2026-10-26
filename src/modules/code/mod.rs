pub mod duplicate_comments;
use crate::core::rule::Rule;
pub fn rules() -> Vec<Box<dyn Rule>> {
    vec![Box::new(duplicate_comments::DuplicateComments)]
}
