use super::{Action, ActionBuilder, ActionState};
use crate::thinker::Actor;
use bevy::prelude::*;
use std::sync::Arc;

/// Configures what mode the [`Concurrently`] action will run in.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Reflect)]
pub enum ConcurrentMode {
    /// Reaches success when any of the concurrent actions reaches [`ActionState::Success`].
    Race,
    /// Reaches success when all of the concurrent actions reach [`ActionState::Success`].
    Join,
}

/// [`ActionBuilder`] for the [`Concurrently`] component. Constructed through
/// `Concurrently::build()`.
#[derive(Reflect)]
pub struct ConcurrentlyBuilder {
    mode: ConcurrentMode,
    #[reflect(ignore)]
    actions: Vec<Arc<dyn ActionBuilder>>,
}

impl ConcurrentlyBuilder {
    /// Add an action to execute. Order does not matter.
    pub fn push(mut self, action_builder: impl ActionBuilder + 'static) -> Self {
        self.actions.push(Arc::new(action_builder));
        self
    }

    /// Sets the [`ConcurrentMode`] for this action.
    pub fn mode(mut self, mode: ConcurrentMode) -> Self {
        self.mode = mode;
        self
    }
}

impl ActionBuilder for ConcurrentlyBuilder {
    fn spawn(&self, cmd: &mut Commands, actor: Actor) -> Action {
        let children: Vec<_> = self
            .actions
            .iter()
            .map(|action| action.spawn(cmd, actor).0)
            .collect();

        Action(
            cmd.spawn((
                actor,
                ActionState::default(),
                Concurrently { mode: self.mode },
            ))
            .push_children(&children[..])
            .id(),
        )
    }
}

/// Composite Action that executes a number of Actions concurrently. Whether
/// this action succeeds depends on its [`ConcurrentMode`]:
///
/// * [`ConcurrentMode::Join`] (default) succeeds when **all** of the actions
///   succeed.
/// * [`ConcurrentMode::Race`] succeeds when **any** of the actions succeed.
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
/// # struct MyOtherAction;
/// # fn main() {
/// Thinker::build()
///     .when(
///         MyScorer,
///         Concurrently::build()
///             .push(MyAction)
///             .push(MyOtherAction)
///         )
/// # ;
/// # }
/// ```
///
#[derive(Component, Debug, Reflect)]
pub struct Concurrently {
    mode: ConcurrentMode,
}

impl Concurrently {
    /// Construct a new [`ConcurrentlyBuilder`] to define the actions to take.
    pub fn build() -> ConcurrentlyBuilder {
        ConcurrentlyBuilder {
            mode: ConcurrentMode::Join,
            actions: Vec::new(),
        }
    }
}

/// System that takes care of executing any existing [`Concurrently`] Actions.
pub fn concurrent_system(
    query: Query<(Entity, &Concurrently, &Children)>,
    mut states: Query<&mut ActionState>,
) {
    for (this_entity, this, actions) in query.iter() {
        match this.mode {
            ConcurrentMode::Join => exec_join(this_entity, actions, &mut states),
            ConcurrentMode::Race => exec_race(this_entity, actions, &mut states),
        }
    }
}

fn exec_join(this_entity: Entity, actions: &Children, states: &mut Query<&mut ActionState>) {
    use ActionState::*;

    match states.get_mut(this_entity).unwrap().clone() {
        Requested => {
            for &child in actions.iter() {
                states.get_mut(child).unwrap().request();
            }
            states.get_mut(this_entity).unwrap().execute();
        }
        Executing => {
            let mut all_success = true;
            let mut failed_index = None;

            for (index, &child_entity) in actions.iter().enumerate() {
                let mut child = states.get_mut(child_entity).unwrap();
                all_success &= child.is_success();
                match *child {
                    Failure => failed_index = Some(index),
                    Requested | Executing if failed_index.is_some() => child.cancel(),
                    _ => (),
                }
            }

            if all_success {
                *states.get_mut(this_entity).unwrap() = Success;
            } else if let Some(index) = failed_index {
                for &child in actions.iter().take(index) {
                    states.get_mut(child).unwrap().cancel_if_running();
                }
                states.get_mut(this_entity).unwrap().done_failure();
            }
        }
        Cancelled => {
            let mut any_err = Success;
            let mut all_done = true;

            for &child_entity in actions.iter() {
                let mut child = states.get_mut(child_entity).unwrap();
                all_done &= child.is_done();
                match *child {
                    Failure => any_err = Failure,
                    Requested | Executing => child.cancel(),
                    Success | Cancelled => (),
                }
            }

            if let Some(any_err) = all_done.then_some(any_err) {
                *states.get_mut(this_entity).unwrap() = any_err;
            }
        }
        Success | Failure => {}
    }
}

fn exec_race(this_entity: Entity, actions: &Children, states: &mut Query<&mut ActionState>) {
    use ActionState::*;

    match states.get_mut(this_entity).unwrap().clone() {
        Requested => {
            for &child in actions.iter() {
                states.get_mut(child).unwrap().request();
            }
            states.get_mut(this_entity).unwrap().execute();
        }
        Executing => {
            let mut all_failure = true;
            let mut succeed_index = None;

            for (index, &child_entity) in actions.iter().enumerate() {
                let mut child = states.get_mut(child_entity).unwrap();
                all_failure &= child.is_failure();
                match *child {
                    Success => succeed_index = Some(index),
                    Requested | Executing if succeed_index.is_some() => child.cancel(),
                    _ => (),
                }
            }

            if all_failure {
                *states.get_mut(this_entity).unwrap() = Failure;
            } else if let Some(index) = succeed_index {
                for &child in actions.iter().take(index) {
                    states.get_mut(child).unwrap().cancel_if_running();
                }
                states.get_mut(this_entity).unwrap().done_success();
            }
        }
        Cancelled => {
            let mut any_ok = Failure;
            let mut all_done = true;

            for &child_entity in actions.iter() {
                let mut child = states.get_mut(child_entity).unwrap();
                all_done &= child.is_done();
                match *child {
                    Success => any_ok = Success,
                    Requested | Executing => child.cancel(),
                    Failure | Cancelled => (),
                }
            }

            if let Some(any_ok) = all_done.then_some(any_ok) {
                *states.get_mut(this_entity).unwrap() = any_ok;
            }
        }
        Success | Failure => {}
    }
}
