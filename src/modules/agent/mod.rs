pub mod guards;
use crate::core::rule::Rule;
pub fn rules() -> Vec<Box<dyn Rule>> {
    vec![
        Box::new(guards::Guard(true)),
        Box::new(guards::Guard(false)),
    ]
}
