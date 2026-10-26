pub mod agent_artifacts;
use crate::core::rule::Rule;
pub fn rules() -> Vec<Box<dyn Rule>> {
    vec![Box::new(agent_artifacts::AgentArtifacts)]
}
