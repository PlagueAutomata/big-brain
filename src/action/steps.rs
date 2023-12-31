use super::{Action, ActionBuilder, ActionState, ReflectActionBuilder};
use crate::thinker::Actor;
use bevy::prelude::*;
use std::sync::Arc;

/// [`ActionBuilder`] for the [`Steps`] component. Constructed through
/// `Steps::build()`.
#[derive(Reflect)]
#[reflect(ActionBuilder)]
pub struct StepsBuilder {
    #[reflect(ignore)]
    steps: Vec<Arc<dyn ActionBuilder>>,
}

impl StepsBuilder {
    /// Adds an action step. Order matters.
    pub fn step(mut self, action_builder: impl ActionBuilder + 'static) -> Self {
        self.steps.push(Arc::new(action_builder));
        self
    }
}

impl ActionBuilder for StepsBuilder {
    fn spawn(&self, cmd: &mut Commands, actor: Actor) -> Action {
        let action = cmd.spawn((actor, ActionState::default())).id();
        if let Some(step) = self.steps.first() {
            let child = step.spawn(cmd, actor);
            cmd.entity(action).add_child(child.0).insert(Steps {
                active_index: 0,
                active: child,
                steps: self.steps.clone(),
            });
        }
        Action(action)
    }
}

/// Composite Action that executes a series of steps in sequential order, as
/// long as each step results in a `Success`ful [`ActionState`].
///
/// ### Example
///
/// ```
/// # use bevy::prelude::*;
/// # use big_brain::prelude::*;
/// # #[derive(Debug, Clone, Component, ScorerBuilder)]
/// # struct MyScorer;
/// # #[derive(Debug, Clone, Component, ActionBuilder)]
/// # struct MyAction;
/// # #[derive(Debug, Clone, Component, ActionBuilder)]
/// # struct MyNextAction;
/// # fn main() {
/// Thinker::build()
///     .when(
///         MyScorer,
///         Steps::build()
///             .step(MyAction)
///             .step(MyNextAction)
///         )
/// # ;
/// # }
/// ```
#[derive(Component, Reflect)]
pub struct Steps {
    #[reflect(ignore)]
    steps: Vec<Arc<dyn ActionBuilder>>,
    active: Action,
    active_index: usize,
}

impl Steps {
    /// Construct a new [`StepsBuilder`] to define the steps to take.
    pub fn build() -> StepsBuilder {
        StepsBuilder { steps: Vec::new() }
    }
}

/// System that takes care of executing any existing [`Steps`] Actions.
pub fn steps_system(
    mut cmd: Commands,
    mut query: Query<(Entity, &Actor, &mut Steps)>,
    mut states: Query<&mut ActionState>,
) {
    use ActionState::*;
    for (this_entity, &actor, mut this) in query.iter_mut() {
        match states.get_mut(this_entity).unwrap().clone() {
            Success | Failure => {
                // Do nothing.
            }

            Requested => {
                *states.get_mut(this.active.entity()).unwrap() = Requested;
                *states.get_mut(this_entity).unwrap() = Executing;
            }

            Executing => {
                let step_state = states.get(this.active.entity()).unwrap().clone();
                match step_state {
                    // do nothing. Everything's running as it should.
                    Requested | Executing => {}

                    // Wait for the step to wrap itself up, and we'll decide what to do at that point.
                    Cancelled => {}

                    Success => {
                        let entity = this.active.entity();
                        cmd.add(DespawnRecursive { entity });

                        if this.active_index == this.steps.len() - 1 {
                            // We're done! Let's just be successful
                            *states.get_mut(this_entity).unwrap() = step_state;
                        } else {
                            this.active_index += 1;
                            this.active = this.steps[this.active_index].spawn(&mut cmd, actor);
                            cmd.entity(this_entity).add_child(this.active.entity());
                        }
                    }

                    Failure => {
                        let entity = this.active.entity();
                        cmd.add(DespawnRecursive { entity });

                        // Fail ourselves
                        *states.get_mut(this_entity).unwrap() = step_state;
                    }
                }
            }

            Cancelled => {
                // Cancel current action
                let mut step_state = states.get_mut(this.active.entity()).unwrap();
                match *step_state {
                    Requested | Executing => *step_state = Cancelled,
                    Success | Failure => *states.get_mut(this_entity).unwrap() = step_state.clone(),
                    Cancelled => (),
                }
            }
        }
    }
}
