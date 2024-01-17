//! Defines Action-related functionality. This module includes the
//! ActionBuilder trait and some Composite Actions for utility.
use crate::thinker::Actor;
use bevy::{ecs::query::WorldQuery, prelude::*, utils::all_tuples};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, Reflect)]
pub struct Action(pub(crate) Entity);

impl Action {
    pub fn entity(&self) -> Entity {
        self.0
    }

    #[must_use]
    pub(crate) fn despawn_recursive(&self) -> DespawnRecursive {
        DespawnRecursive { entity: self.0 }
    }
}

pub struct ActionCommands<'w, 's, 'a> {
    cmd: &'a mut Commands<'w, 's>,
    actor: Actor,
}

impl<'w, 's, 'a> ActionCommands<'w, 's, 'a> {
    #[inline]
    pub(crate) fn new(cmd: &'a mut Commands<'w, 's>, actor: Actor) -> Self {
        Self { cmd, actor }
    }

    #[inline]
    pub fn spawn(&mut self, bundle: impl Bundle) -> Action {
        let bundle = (self.actor, ActionState::Executing, bundle);
        Action(self.cmd.spawn(bundle).id())
    }

    #[inline]
    pub fn add_child(&mut self, Action(parent): Action, builder: &dyn ActionSpawn) {
        let Action(child) = builder.spawn(ActionCommands::new(self.cmd, self.actor));
        self.cmd.add(AddChild { parent, child })
    }
}

/// The current state for an Action. These states are changed by a combination
/// of the Thinker that spawned it, and the actual Action system executing the
/// Action itself.
///
/// Action system implementors should be mindful of taking appropriate action
/// on all of these states, and be particularly careful when ignoring
/// variants.
#[derive(Debug, Clone, Component, Eq, PartialEq, Reflect, Default)]
#[component(storage = "SparseSet")]
pub enum ActionState {
    /// The action has ongoing execution. The associated Thinker will try to
    /// keep executing this Action as-is until it changes state or it gets
    /// Cancelled.
    #[default]
    Executing,

    /// An ongoing Action has been cancelled. The Thinker might set this
    /// action for you, so for Actions that execute for longer than a single
    /// tick, **you must check whether the Cancelled state was set** and
    /// change do either Success or Failure. Thinkers will wait on Cancelled
    /// actions to do any necessary cleanup work, so this can hang your AI if
    /// you don't look for it.
    Cancelled,

    /// The Action was a success. This is used by Composite Actions to
    /// determine whether to continue execution.
    Success,

    /// The Action failed. This is used by Composite Actions to determine
    /// whether to halt execution.
    Failure,
}

impl ActionState {
    /// Sets state to [`ActionState::Success`].
    #[inline]
    pub fn success(&mut self) {
        *self = Self::Success;
    }

    /// Sets state to [`ActionState::Failure`].
    #[inline]
    pub fn failure(&mut self) {
        *self = Self::Failure;
    }

    /// Sets state to [`ActionState::Cancelled`].
    #[inline]
    pub fn cancel(&mut self) {
        *self = Self::Cancelled;
    }

    /// Returns true if the state is a [`ActionState::Executing`] value.
    #[inline]
    pub fn is_executing(&self) -> bool {
        matches!(self, Self::Executing)
    }

    /// Returns true if the state is a [`ActionState::Cancelled`] value.
    #[inline]
    pub fn is_cancelled(&self) -> bool {
        matches!(self, Self::Cancelled)
    }

    /// Returns true if the state is a [`ActionState::Success`] value.
    #[inline]
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success)
    }

    /// Returns true if the state is a [`ActionState::Failure`] value.
    #[inline]
    pub fn is_failure(&self) -> bool {
        matches!(self, Self::Failure)
    }

    /// Returns true if the state is a [`ActionState::Success`] or [`ActionState::Failure`] value.
    #[inline]
    pub fn is_done(&self) -> bool {
        matches!(self, Self::Success | Self::Failure)
    }

    #[inline]
    pub(crate) fn cancel_if_executing(&mut self) {
        if matches!(self, Self::Executing) {
            *self = Self::Cancelled;
        }
    }
}

pub type ActionInner = Arc<dyn ActionSpawn>;

/// Trait that must be defined by types in order to be `ActionBuilder`s.
/// `ActionBuilder`s' job is to spawn new `Action` entities on demand. In
/// general, most of this is already done for you, and the only method you
/// really have to implement is `.build()`.
///
/// The `build()` method MUST be implemented for any `ActionBuilder`s you want
/// to define.
pub trait ActionSpawn: Send + Sync {
    //  /// MUST insert your concrete Action component into the Scorer [`Entity`],
    //  /// using `cmd`. You _may_ use `actor`, but it's perfectly normal to just
    //  /// ignore it.
    //  ///
    //  /// In most cases, your `ActionBuilder` and `Action` can be the same type.
    //  /// The only requirement is that your struct implements `Debug`,
    //  /// `Component, `Clone`. You can then use the derive macro `ActionBuilder`
    //  /// to turn your struct into a `ActionBuilder`
    //  ///
    //  /// ### Example
    //  ///
    //  /// Using the derive macro (the easy way):
    //  ///
    //  /// ```
    //  /// # use bevy::prelude::*;
    //  /// # use big_brain::prelude::*;
    //  /// #[derive(Debug, Clone, Component, ActionBuilder)]
    //  /// #[action_label = "MyActionLabel"] // Optional. Defaults to type name.
    //  /// struct MyAction;
    //  /// ```
    //  ///
    //  /// Implementing it manually:
    //  ///
    //  /// ```
    //  /// # use bevy::prelude::*;
    //  /// # use big_brain::prelude::*;
    //  /// #[derive(Debug)]
    //  /// struct MyBuilder;
    //  /// #[derive(Debug, Component)]
    //  /// struct MyAction;
    //  ///
    //  /// impl ActionBuilder for MyBuilder {
    //  ///   fn build(&self, cmd: &mut Commands, action: Entity, actor: Entity) {
    //  ///     cmd.entity(action).insert(MyAction);
    //  ///   }
    //  /// }
    //  /// ```
    //  //fn build(&self, cmd: &mut Commands, action: Entity, actor: Entity);

    /// Spawns a new Action Component, using the given ActionBuilder. This is
    /// useful when you're doing things like writing composite Actions.
    fn spawn(&self, cmd: ActionCommands) -> Action;
}

pub trait ActionsList {
    fn build(actions: Self) -> Vec<Arc<dyn ActionSpawn>>;
}

impl<T: ActionSpawn + 'static> ActionsList for T {
    fn build(action: Self) -> Vec<Arc<dyn ActionSpawn>> {
        vec![Arc::new(action)]
    }
}

macro_rules! impl_actions_list {
    ($(($Type:ident, $index:ident)),*) => {
        impl<$($Type),*> ActionsList for ($($Type,)*) where $($Type: ActionSpawn + 'static),* {
            fn build(($($index,)*): Self) -> Vec<Arc<dyn ActionSpawn>> {
                vec![ $(Arc::new($index),)* ]
            }
        }
    }
}

all_tuples!(impl_actions_list, 1, 15, Type, index);

#[derive(WorldQuery)]
#[world_query(mutable)]
pub struct ActionQuery {
    state: &'static mut ActionState,
    actor: &'static Actor,
}

impl<'w> ActionQueryItem<'w> {
    pub fn actor(&self) -> Entity {
        self.actor.entity()
    }

    pub fn state(&self) -> ActionState {
        self.state.clone()
    }

    pub fn is_executing(&self) -> bool {
        self.state.is_executing()
    }

    pub fn is_cancelled(&self) -> bool {
        self.state.is_cancelled()
    }

    pub fn is_done(&self) -> bool {
        self.state.is_done()
    }

    pub fn cancel(&mut self) {
        self.state.cancel();
    }

    pub fn success(&mut self) {
        self.state.success();
    }

    pub fn failure(&mut self) {
        self.state.failure();
    }

    pub fn failure_if_cancelled(&mut self) {
        if self.state.is_cancelled() {
            self.state.failure()
        }
    }
}

impl<'w> ActionQueryReadOnlyItem<'w> {
    pub fn actor(&self) -> Entity {
        self.actor.entity()
    }

    pub fn state(&self) -> ActionState {
        self.state.clone()
    }

    pub fn is_executing(&self) -> bool {
        self.state.is_executing()
    }

    pub fn is_cancelled(&self) -> bool {
        self.state.is_cancelled()
    }

    pub fn is_done(&self) -> bool {
        self.state.is_done()
    }
}
