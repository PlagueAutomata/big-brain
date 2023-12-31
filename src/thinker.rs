//! Thinkers are the "brain" of an entity. You attach Scorers to it, and the
//! Thinker picks the right Action to run based on the resulting Scores.

use crate::{
    action::{self, Action, ActionBuilder, ActionInner, ActionState},
    choices::{Choice, ChoiceBuilder},
    pickers::Picker,
    scorers::{Score, ScorerBuilder},
};
use bevy::{
    prelude::*,
    utils::{tracing::debug, Duration, Instant},
};
use std::{collections::VecDeque, sync::Arc};

/// Wrapper for Actor entities. In terms of Scorers, Thinkers, and Actions,
/// this is the [`Entity`] actually _performing_ the action, rather than the
/// entity a Scorer/Thinker/Action is attached to. Generally, you will use
/// this entity when writing Queries for Action and Scorer systems.
#[derive(Debug, Clone, Component, Copy, Reflect)]
pub struct Actor(pub Entity);

#[derive(Debug, Clone, Copy, Reflect)]
pub struct Scorer(pub Entity);

/// The "brains" behind this whole operation. A `Thinker` is what glues
/// together `Actions` and `Scorers` and shapes larger, intelligent-seeming
/// systems.
///
/// Note: Thinkers are also Actions, so anywhere you can pass in an Action (or
/// [`ActionBuilder`]), you can pass in a Thinker (or [`ThinkerBuilder`]).
///
/// ### Example
///
/// ```
/// # use bevy::prelude::*;
/// # use big_brain::prelude::*;
/// # #[derive(Component, Debug)]
/// # struct Thirst(f32, f32);
/// # #[derive(Component, Debug)]
/// # struct Hunger(f32, f32);
/// # #[derive(Clone, Component, Debug, ScorerBuilder)]
/// # struct Thirsty;
/// # #[derive(Clone, Component, Debug, ScorerBuilder)]
/// # struct Hungry;
/// # #[derive(Clone, Component, Debug, ActionBuilder)]
/// # struct Drink;
/// # #[derive(Clone, Component, Debug, ActionBuilder)]
/// # struct Eat;
/// # #[derive(Clone, Component, Debug, ActionBuilder)]
/// # struct Meander;
/// pub fn init_entities(mut cmd: Commands) {
///     cmd.spawn((
///         Thirst(70.0, 2.0),
///         Hunger(50.0, 3.0),
///         Thinker::build()
///             .picker(FirstToScore::new(80.0))
///             .when(Thirsty, Drink)
///             .when(Hungry, Eat)
///             .otherwise(Meander),
///     ));
/// }
/// ```
#[derive(Component)]
pub struct Thinker {
    picker: Arc<dyn Picker>,
    otherwise: Option<ActionInner>,
    choices: Vec<Choice>,
    current_action: Option<(Action, ActionInner)>,
    scheduled_actions: VecDeque<ActionInner>,
}

impl Thinker {
    /// Make a new [`ThinkerBuilder`]. This is what you'll actually use to
    /// configure Thinker behavior.
    pub fn build(picker: impl Picker + 'static) -> ThinkerBuilder {
        ThinkerBuilder::new(picker)
    }

    pub fn schedule_action(&mut self, action: impl ActionBuilder + 'static) {
        self.scheduled_actions
            .push_back(ActionInner::new(Arc::new(action)));
    }
}

/// This is what you actually use to configure Thinker behavior. It's a plain
/// old [`ActionBuilder`], as well.
#[derive(Component, Clone)]
pub struct ThinkerBuilder {
    picker: Arc<dyn Picker>,
    otherwise: Option<ActionInner>,
    choices: Vec<ChoiceBuilder>,
}

impl ThinkerBuilder {
    pub(crate) fn new(picker: impl Picker + 'static) -> Self {
        Self {
            picker: Arc::new(picker),
            otherwise: None,
            choices: Vec::new(),
        }
    }

    /// Define an [`ActionBuilder`](crate::actions::ActionBuilder) and
    /// [`ScorerBuilder`](crate::scorers::ScorerBuilder) pair.
    pub fn when(
        mut self,
        when: impl ScorerBuilder + 'static,
        then: impl ActionBuilder + 'static,
    ) -> Self {
        self.choices.push(ChoiceBuilder {
            when: Arc::new(when),
            then: Arc::new(then),
        });
        self
    }

    /// Default `Action` to execute if the `Picker` did not pick any of the
    /// given choices.
    pub fn otherwise(mut self, otherwise: impl ActionBuilder + 'static) -> Self {
        self.otherwise = Some(ActionInner::new(Arc::new(otherwise)));
        self
    }
}

impl ActionBuilder for ThinkerBuilder {
    fn spawn(&self, cmd: &mut Commands, actor: Actor) -> Action {
        debug!("Spawning Thinker.");

        let action_ent = cmd.spawn_empty().id();

        let choices = self
            .choices
            .iter()
            .map(|choice| choice.build(cmd, actor, action_ent))
            .collect();

        cmd.entity(action_ent).insert((
            actor,
            ActionState::Requested,
            Thinker {
                // TODO: reasonable default?...
                picker: self.picker.clone(),
                otherwise: self.otherwise.clone(),
                choices,
                current_action: None,
                scheduled_actions: VecDeque::new(),
            },
        ));

        Action(action_ent)
    }
}

pub fn thinker_component_attach_system(
    mut cmd: Commands,
    q: Query<(Entity, &ThinkerBuilder), Without<HasThinker>>,
) {
    for (entity, thinker_builder) in q.iter() {
        let Action(thinker) = thinker_builder.spawn(&mut cmd, Actor(entity));
        cmd.entity(entity).insert(HasThinker(thinker));
    }
}

pub fn thinker_component_detach_system(
    mut cmd: Commands,
    q: Query<(Entity, &HasThinker), Without<ThinkerBuilder>>,
) {
    for (actor, HasThinker(thinker)) in q.iter() {
        if let Some(ent) = cmd.get_entity(*thinker) {
            ent.despawn_recursive();
        }
        cmd.entity(actor).remove::<HasThinker>();
    }
}

pub fn actor_gone_cleanup(
    mut cmd: Commands,
    actors: Query<&ThinkerBuilder>,
    q: Query<(Entity, &Actor)>,
) {
    for (child, Actor(actor)) in q.iter() {
        if actors.get(*actor).is_err() {
            // Actor is gone. Let's clean up.
            if let Some(ent) = cmd.get_entity(child) {
                ent.despawn_recursive();
            }
        }
    }
}

#[derive(Component, Debug, Reflect)]
pub struct HasThinker(Entity);

impl HasThinker {
    pub fn entity(&self) -> Entity {
        self.0
    }
}

pub struct ThinkerIterations {
    index: usize,
    max_duration: Duration,
}

impl Default for ThinkerIterations {
    fn default() -> Self {
        Self::new(Duration::from_millis(10))
    }
}

impl ThinkerIterations {
    pub fn new(max_duration: Duration) -> Self {
        Self {
            index: 0,
            max_duration,
        }
    }
}

pub fn thinker_system(
    mut cmd: Commands,
    mut iterations: Local<ThinkerIterations>,
    mut query: Query<(Entity, &Actor, &mut Thinker)>,
    scores: Query<&Score>,
    mut states: Query<&mut action::ActionState>,
) {
    use ActionState::*;

    let start = Instant::now();
    for (thinker_entity, &actor, mut thinker) in query.iter_mut().skip(iterations.index) {
        iterations.index += 1;

        match states.get_mut(thinker_entity).unwrap().clone() {
            Requested => {
                debug!("Thinker requested. Starting execution.");
                *states.get_mut(thinker_entity).unwrap() = Executing;
            }

            Executing => {
                if let Some(action) = thinker.picker.pick(&thinker.choices, &scores) {
                    // Think about what action we're supposed to be taking. We do this
                    // every tick, because we might change our mind.
                    // ...and then execute it (details below).
                    thinker.exec_picked_action(&mut cmd, actor, action, &mut states, true);
                } else if thinker.should_schedule_action(&states) {
                    debug!("Spawning scheduled action.");
                    let action = thinker.scheduled_actions.pop_front().unwrap();
                    thinker.current_action = Some((action.spawn(&mut cmd, actor), action));
                } else if let Some(default_action) = thinker.otherwise.clone() {
                    // Otherwise, let's just execute the default one! (if it's there)
                    thinker.exec_picked_action(&mut cmd, actor, default_action, &mut states, false);
                } else if let Some(Action(action)) =
                    thinker.current_action.as_ref().map(|(a, _)| *a)
                {
                    let state = states.get(action).unwrap().clone();
                    if matches!(state, Success | Failure) {
                        debug!(
                            "Action completed and nothing was picked. Despawning action entity."
                        );
                        cmd.add(DespawnRecursive { entity: action });
                        thinker.current_action = None;
                    }
                }
            }

            Success | Failure => {}

            Cancelled => {
                debug!("Thinker cancelled. Cleaning up.");
                if let Some(Action(current)) = thinker.current_action.as_ref().map(|(c, _)| *c) {
                    debug!("Cancelling current action because thinker was cancelled.");
                    let mut state = states.get_mut(current).unwrap();
                    match *state {
                        Requested | Executing => {
                            debug!("Action is still executing. Attempting to cancel it before wrapping up Thinker cancellation.\nParent thinker was cancelled. Cancelling action.");
                            *state = Cancelled;
                        }
                        Cancelled => debug!("Current action already cancelled."),
                        Success | Failure => {
                            debug!("Action already wrapped up on its own. Cleaning up action in Thinker.");
                            cmd.add(DespawnRecursive { entity: current });
                            thinker.current_action = None;
                        }
                    }
                } else {
                    debug!("No current thinker action. Wrapping up Thinker as Succeeded.");
                    *states.get_mut(thinker_entity).unwrap() = Success;
                }
            }
        }

        if iterations.index % 500 == 0 && start.elapsed() > iterations.max_duration {
            return;
        }
    }

    iterations.index = 0;
}

impl Thinker {
    fn should_schedule_action(&self, states: &Query<&mut ActionState>) -> bool {
        if self.scheduled_actions.is_empty() {
            false
        } else if let Some((action_ent, _)) = &self.current_action {
            let state = states.get(action_ent.0).unwrap();
            matches!(state, ActionState::Success | ActionState::Failure)
        } else {
            true
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn exec_picked_action(
        &mut self,
        cmd: &mut Commands,
        actor: Actor,
        next: ActionInner,
        states: &mut Query<&mut ActionState>,
        override_current: bool,
    ) {
        // If we do find one, then we need to grab the corresponding
        // component for it. The "action" that `picker.pick()` returns
        // is just a newtype for an Entity.

        // Now we check the current action. We need to check if we picked the same one as the previous tick.
        //
        // TODO: I don't know where the right place to put this is
        // (maybe not in this logic), but we do need some kind of
        // oscillation protection so we're not just bouncing back and
        // forth between the same couple of actions.

        use ActionState::*;

        if let Some((Action(action), current)) = &self.current_action {
            let mut current_state = states.get_mut(*action).expect("Couldn't find a component corresponding to the current action. This is definitely a bug.");
            let previous_done = matches!(*current_state, Success | Failure);
            if (!current.id_eq(&next) && override_current) || previous_done {
                // So we've picked a different action than we were
                // currently executing. Just like before, we grab the
                // actual Action component (and we assume it exists).
                // If the action is executing, or was requested, we
                // need to cancel it to make sure it stops.
                match *current_state {
                    Executing | Requested => {
                        debug!(
                            "Previous action is still executing. Requesting action cancellation."
                        );
                        *current_state = Cancelled;
                    }
                    Success | Failure => {
                        debug!("Previous action already completed. Despawning action entity.",);
                        // Despawn the action itself.
                        cmd.add(DespawnRecursive { entity: *action });

                        debug!("Spawning next action");
                        self.current_action = Some((next.spawn(cmd, actor), next));
                    }
                    Cancelled => {}
                };
            } else {
                // Otherwise, it turns out we want to keep executing
                // the same action. Just in case, we go ahead and set
                // it as Requested if for some reason it had finished
                // but the Action System hasn't gotten around to
                // cleaning it up.
            }
        } else {
            // This branch arm is called when there's no
            // current_action in the thinker. The logic here is pretty
            // straightforward -- we set the action, Request it, and
            // that's it.
            debug!("No current action. Spawning new action.");
            let new_action = next.spawn(cmd, actor);
            self.current_action = Some((new_action, next.clone()));
        }
    }
}
