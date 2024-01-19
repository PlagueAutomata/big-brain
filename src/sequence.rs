use crate::{
    action::{Action, ActionCommands, ActionSpawn, ActionState, ActionsList},
    thinker::Actor,
};
use bevy_ecs::{
    component::Component,
    entity::Entity,
    query::Without,
    system::{Commands, Query},
    world::Mut,
};
use bevy_hierarchy::{AddChild, Children};
use bevy_log as log;
use bevy_reflect::Reflect;
use std::sync::Arc;

/// Configures what mode the [`Sequence`] action will run in.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Reflect)]
pub enum SequenceMode {
    /// Reaches success when any of the concurrent actions reaches [`ActionState::Success`].
    Race,
    /// Reaches success when all of the concurrent actions reach [`ActionState::Success`].
    Join,
    /// Composite Action that executes a series of steps in sequential order, as
    /// long as each step results in a `Success`ful [`ActionState`].
    Step,
}

/// [`ActionSpawn`] for the [`Sequence`] component.
pub struct SequenceSpawner {
    mode: SequenceMode,
    actions: Vec<Arc<dyn ActionSpawn>>,
}

impl ActionSpawn for SequenceSpawner {
    fn spawn(&self, mut cmd: ActionCommands) -> Action {
        let action = cmd.spawn(Sequence {
            mode: self.mode,
            active_step: 0,
            steps: self.actions.clone(),
        });

        match self.mode {
            SequenceMode::Join | SequenceMode::Race => {
                for child in &self.actions {
                    cmd.add_child(action, child.as_ref());
                }
            }
            SequenceMode::Step => {
                if let Some(child) = self.actions.first() {
                    cmd.add_child(action, child.as_ref());
                }
            }
        }

        action
    }
}

/// Composite Action that executes a number of Actions concurrently. Whether
/// this action succeeds depends on its [`SequenceMode`]:
///
/// * [`SequenceMode::Join`] succeeds when **all** of the actions
///   succeed.
/// * [`SequenceMode::Race`] succeeds when **any** of the actions succeed.
///
/// ### Example
///
/// ```
/// # use bevy::prelude::*;
/// # use big_brain::*;
/// # #[derive(Debug, Clone, Component, ScorerSpawn)]
/// # struct MyScorer;
/// # #[derive(Debug, Clone, Component, ActionSpawn)]
/// # struct MyAction;
/// # #[derive(Debug, Clone, Component, ActionSpawn)]
/// # struct MyOtherAction;
/// # fn main() {
/// ThinkerSpawner::highest(0.0)
///     .when(MyScorer, Sequence::join((MyAction, MyOtherAction)))
/// # ;
/// # }
/// ```
///
#[derive(Component)]
pub struct Sequence {
    mode: SequenceMode,
    active_step: usize,
    steps: Vec<Arc<dyn ActionSpawn>>,
}

impl Sequence {
    /// Construct a new [`SequenceSpawner`] to define the actions to take.
    pub fn join<B: ActionsList>(actions: B) -> SequenceSpawner {
        SequenceSpawner {
            mode: SequenceMode::Join,
            actions: ActionsList::build(actions),
        }
    }

    /// Construct a new [`SequenceSpawner`] to define the actions to take.
    pub fn race<B: ActionsList>(actions: B) -> SequenceSpawner {
        SequenceSpawner {
            mode: SequenceMode::Race,
            actions: ActionsList::build(actions),
        }
    }

    /// Construct a new [`SequenceSpawner`] to define the actions to take.
    pub fn step<B: ActionsList>(actions: B) -> SequenceSpawner {
        SequenceSpawner {
            mode: SequenceMode::Step,
            actions: ActionsList::build(actions),
        }
    }
}

/// System that takes care of executing any existing [`Concurrently`] Actions.
pub fn sequence_system(
    mut cmd: Commands,
    mut query: Query<(Entity, &mut ActionState, &mut Sequence, &Children, &Actor)>,
    mut states: Query<&mut ActionState, Without<Sequence>>,
) {
    for (parent, this_state, sequence, actions, &actor) in query.iter_mut() {
        let mode = sequence.mode;
        log::trace!("start {:?} {:?}", mode, parent);
        match mode {
            SequenceMode::Join => exec_join(this_state, actions, &mut states),
            SequenceMode::Race => exec_race(this_state, actions, &mut states),
            SequenceMode::Step => exec_step(
                this_state,
                actions,
                &mut states,
                &mut cmd,
                parent,
                sequence,
                actor,
            ),
        }
        log::trace!("end {:?} {:?}", mode, parent);
    }
}

fn exec_join(
    mut this_state: Mut<ActionState>,
    actions: &Children,
    states: &mut Query<&mut ActionState, Without<Sequence>>,
) {
    match this_state.clone() {
        ActionState::Executing => {
            let mut all_success = true;
            let mut failed_index = None;

            for (index, &child_entity) in actions.iter().enumerate() {
                let mut child = states.get_mut(child_entity).unwrap();
                all_success &= child.is_success();
                match *child {
                    ActionState::Failure => failed_index = Some(index),
                    ActionState::Executing if failed_index.is_some() => child.cancel(),
                    ActionState::Executing | ActionState::Cancelled | ActionState::Success => (),
                }
            }

            if all_success {
                this_state.success();
            } else if let Some(index) = failed_index {
                for &child in actions.iter().take(index) {
                    states.get_mut(child).unwrap().cancel_if_executing();
                }
                this_state.failure();
            }
        }
        ActionState::Cancelled => {
            let mut any_err = false;
            let mut all_done = true;

            for &child_entity in actions.iter() {
                let mut child = states.get_mut(child_entity).unwrap();
                all_done &= child.is_done();
                match *child {
                    ActionState::Failure => any_err = true,
                    ActionState::Executing => child.cancel(),
                    ActionState::Success | ActionState::Cancelled => (),
                }
            }

            if all_done && any_err {
                this_state.failure()
            }
            if all_done && !any_err {
                this_state.success()
            }
        }
        ActionState::Success | ActionState::Failure => {}
    }
}

fn exec_race(
    mut this_state: Mut<ActionState>,
    actions: &Children,
    states: &mut Query<&mut ActionState, Without<Sequence>>,
) {
    match this_state.clone() {
        ActionState::Executing => {
            let mut all_failure = true;
            let mut succeed_index = None;

            for (index, &child_entity) in actions.iter().enumerate() {
                let mut child = states.get_mut(child_entity).unwrap();
                all_failure &= child.is_failure();
                match *child {
                    ActionState::Success => succeed_index = Some(index),
                    ActionState::Executing if succeed_index.is_some() => child.cancel(),
                    ActionState::Executing | ActionState::Cancelled | ActionState::Failure => (),
                }
            }

            if all_failure {
                this_state.failure();
            } else if let Some(index) = succeed_index {
                for &child in actions.iter().take(index) {
                    states.get_mut(child).unwrap().cancel_if_executing();
                }
                this_state.success();
            }
        }
        ActionState::Cancelled => {
            let mut any_ok = false;
            let mut all_done = true;

            for &child_entity in actions.iter() {
                let mut child = states.get_mut(child_entity).unwrap();
                all_done &= child.is_done();
                match *child {
                    ActionState::Success => any_ok = true,
                    ActionState::Executing => child.cancel(),
                    ActionState::Failure | ActionState::Cancelled => (),
                }
            }

            if all_done && any_ok {
                this_state.success()
            }
            if all_done && !any_ok {
                this_state.failure()
            }
        }
        ActionState::Success | ActionState::Failure => {}
    }
}

fn exec_step(
    mut this_state: Mut<ActionState>,
    actions: &Children,
    states: &mut Query<&mut ActionState, Without<Sequence>>,

    cmd: &mut Commands,
    parent: Entity,
    mut sequence: Mut<Sequence>,
    actor: Actor,
) {
    let Some(active) = actions.first().copied().map(Action) else {
        return;
    };

    let mut active_state = states.get_mut(active.entity()).unwrap();

    match (this_state.clone(), active_state.clone()) {
        (ActionState::Executing, ActionState::Executing | ActionState::Cancelled) => (),
        (ActionState::Executing, ActionState::Success) => {
            cmd.add(active.despawn_recursive());

            if sequence.active_step == sequence.steps.len() - 1 {
                // We're done! Let's just be successful
                this_state.success();
            } else {
                sequence.active_step += 1;
                let child =
                    sequence.steps[sequence.active_step].spawn(ActionCommands::new(cmd, actor));
                let child = child.entity();
                cmd.add(AddChild { parent, child });
            }
        }
        (ActionState::Executing, ActionState::Failure) => {
            cmd.add(active.despawn_recursive());
            this_state.failure();
        }

        (ActionState::Cancelled, ActionState::Executing) => active_state.cancel(),
        (ActionState::Cancelled, ActionState::Success) => this_state.success(),
        (ActionState::Cancelled, ActionState::Failure) => this_state.failure(),

        (_, _) => (),
    }
}
