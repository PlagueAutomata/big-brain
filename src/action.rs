//! Defines Action-related functionality. This module includes the
//! ActionBuilder trait and some Composite Actions for utility.
use crate::thinker::Actor;
use bevy::prelude::*;
use std::sync::Arc;

pub mod concurent;
pub mod steps;

#[derive(Debug, Clone, Copy, Reflect)]
pub struct Action(pub Entity);

impl Action {
    pub fn entity(&self) -> Entity {
        self.0
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
    /// Action requested. The Action-handling system should start executing
    /// this Action ASAP and change the status to the next state.
    #[default]
    Requested,

    /// The action has ongoing execution. The associated Thinker will try to
    /// keep executing this Action as-is until it changes state or it gets
    /// Cancelled.
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
    #[inline]
    pub(crate) fn request(&mut self) {
        *self = Self::Requested;
    }

    #[inline]
    pub(crate) fn execute(&mut self) {
        *self = Self::Requested;
    }

    #[inline]
    pub(crate) fn cancel(&mut self) {
        *self = Self::Cancelled;
    }

    #[inline]
    pub(crate) fn done_success(&mut self) {
        *self = Self::Success;
    }

    #[inline]
    pub(crate) fn done_failure(&mut self) {
        *self = Self::Failure;
    }

    #[inline]
    pub(crate) fn cancel_if_running(&mut self) {
        if matches!(self, Self::Requested | Self::Executing) {
            *self = Self::Cancelled;
        }
    }

    #[inline]
    pub(crate) fn is_success(&self) -> bool {
        matches!(self, Self::Success)
    }

    #[inline]
    pub(crate) fn is_failure(&self) -> bool {
        matches!(self, Self::Failure)
    }

    #[inline]
    pub(crate) fn is_done(&self) -> bool {
        matches!(self, Self::Success | Self::Failure)
    }
}

#[derive(Clone, Eq, PartialEq)]
pub(crate) struct ActionId;

#[derive(Clone)]
pub struct ActionInner {
    id: Arc<ActionId>,
    builder: Arc<dyn ActionBuilder>,
}

impl ActionInner {
    pub fn new(builder: Arc<dyn ActionBuilder>) -> Self {
        let id = Arc::new(ActionId);
        Self { id, builder }
    }

    pub fn id_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.id, &other.id)
    }

    pub fn spawn(&self, cmd: &mut Commands, actor: Actor) -> Action {
        self.builder.spawn(cmd, actor)
    }
}

/// Trait that must be defined by types in order to be `ActionBuilder`s.
/// `ActionBuilder`s' job is to spawn new `Action` entities on demand. In
/// general, most of this is already done for you, and the only method you
/// really have to implement is `.build()`.
///
/// The `build()` method MUST be implemented for any `ActionBuilder`s you want
/// to define.
#[reflect_trait]
pub trait ActionBuilder: Send + Sync {
    /// MUST insert your concrete Action component into the Scorer [`Entity`],
    /// using `cmd`. You _may_ use `actor`, but it's perfectly normal to just
    /// ignore it.
    ///
    /// In most cases, your `ActionBuilder` and `Action` can be the same type.
    /// The only requirement is that your struct implements `Debug`,
    /// `Component, `Clone`. You can then use the derive macro `ActionBuilder`
    /// to turn your struct into a `ActionBuilder`
    ///
    /// ### Example
    ///
    /// Using the derive macro (the easy way):
    ///
    /// ```
    /// # use bevy::prelude::*;
    /// # use big_brain::prelude::*;
    /// #[derive(Debug, Clone, Component, ActionBuilder)]
    /// #[action_label = "MyActionLabel"] // Optional. Defaults to type name.
    /// struct MyAction;
    /// ```
    ///
    /// Implementing it manually:
    ///
    /// ```
    /// # use bevy::prelude::*;
    /// # use big_brain::prelude::*;
    /// #[derive(Debug)]
    /// struct MyBuilder;
    /// #[derive(Debug, Component)]
    /// struct MyAction;
    ///
    /// impl ActionBuilder for MyBuilder {
    ///   fn build(&self, cmd: &mut Commands, action: Entity, actor: Entity) {
    ///     cmd.entity(action).insert(MyAction);
    ///   }
    /// }
    /// ```
    //fn build(&self, cmd: &mut Commands, action: Entity, actor: Entity);

    /// Spawns a new Action Component, using the given ActionBuilder. This is
    /// useful when you're doing things like writing composite Actions.
    fn spawn(&self, cmd: &mut Commands, actor: Actor) -> Action;
}
