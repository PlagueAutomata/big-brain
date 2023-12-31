use crate::{
    action::{ActionBuilder, ActionInner},
    scorers::{Score, ScorerBuilder},
    thinker::{Actor, Scorer},
};
use bevy::prelude::*;
use std::sync::Arc;

/// Contains different types of Considerations and Actions
#[derive(Clone)]
pub struct Choice {
    pub(crate) scorer: Scorer,
    pub(crate) action: ActionInner,
}

impl Choice {
    pub fn calculate(&self, scores: &Query<&Score>) -> Score {
        scores
            .get(self.scorer.0)
            .cloned()
            .expect("Where did the score go?")
    }
}

/// Builds a new [`Choice`].
#[derive(Clone)]
pub struct ChoiceBuilder {
    pub when: Arc<dyn ScorerBuilder>,
    pub then: Arc<dyn ActionBuilder>,
}

impl ChoiceBuilder {
    pub fn build(&self, cmd: &mut Commands, actor: Actor, parent: Entity) -> Choice {
        let scorer = self.when.spawn(cmd, actor);
        cmd.entity(parent).add_child(scorer.0);

        Choice {
            scorer,
            action: ActionInner::new(self.then.clone()),
        }
    }
}
