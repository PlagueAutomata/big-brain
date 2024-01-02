//! Thinkers are the "brain" of an entity. You attach Scorers to it, and the
//! Thinker picks the right Action to run based on the resulting Scores.

use crate::{
    action::{Action, ActionCommands, ActionInner, ActionSpawn, ActionState},
    pickers::{Choice, ChoiceBuilder, FirstToScore, Highest, Picker},
    scorer::{Score, ScorerCommands, ScorerSpawn},
};
use bevy::prelude::*;
use std::{collections::VecDeque, sync::Arc};

/// Wrapper for Actor entities. In terms of Scorers, Thinkers, and Actions,
/// this is the [`Entity`] actually _performing_ the action, rather than the
/// entity a Scorer/Thinker/Action is attached to. Generally, you will use
/// this entity when writing Queries for Action and Scorer systems.
#[derive(Debug, Clone, Component, Copy, Reflect)]
pub struct Actor(pub(crate) Entity);

impl Actor {
    pub fn entity(&self) -> Entity {
        self.0
    }
}

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
/// # #[derive(Clone, Component, Debug, ScorerSpawn)]
/// # struct Thirsty;
/// # #[derive(Clone, Component, Debug, ScorerSpawn)]
/// # struct Hungry;
/// # #[derive(Clone, Component, Debug, ActionSpawn)]
/// # struct Drink;
/// # #[derive(Clone, Component, Debug, ActionSpawn)]
/// # struct Eat;
/// # #[derive(Clone, Component, Debug, ActionSpawn)]
/// # struct Meander;
/// pub fn init_entities(mut cmd: Commands) {
///     cmd.spawn((
///         Thirst(70.0, 2.0),
///         Hunger(50.0, 3.0),
///         Thinker::build(FirstToScore::new(80.0))
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

    /// Make a new [`ThinkerBuilder`]. This is what you'll actually use to
    /// configure Thinker behavior.
    pub fn highest() -> ThinkerBuilder {
        ThinkerBuilder::new(Highest)
    }

    /// Make a new [`ThinkerBuilder`]. This is what you'll actually use to
    /// configure Thinker behavior.
    pub fn first_to_score(threshold: f32) -> ThinkerBuilder {
        ThinkerBuilder::new(FirstToScore { threshold })
    }

    pub fn schedule_action(&mut self, action: impl ActionSpawn + 'static) {
        self.scheduled_actions
            .push_back(ActionInner::new(Arc::new(action)));
    }
}

/// This is what you actually use to configure Thinker behavior. It's a plain
/// old [`ActionBuilder`], as well.
#[derive(Component, Clone)]
pub struct ThinkerBuilder {
    picker: Arc<dyn Picker>,
    idle: Option<ActionInner>,
    choices: Vec<ChoiceBuilder>,
}

impl ThinkerBuilder {
    pub(crate) fn new(picker: impl Picker + 'static) -> Self {
        Self {
            picker: Arc::new(picker),
            idle: None,
            choices: Vec::new(),
        }
    }

    /// Define an [`ActionBuilder`](crate::actions::ActionBuilder) and
    /// [`ScorerBuilder`](crate::scorers::ScorerBuilder) pair.
    pub fn when(
        mut self,
        when: impl ScorerSpawn + 'static,
        then: impl ActionSpawn + 'static,
    ) -> Self {
        self.choices.push(ChoiceBuilder {
            when: Arc::new(when),
            then: Arc::new(then),
        });
        self
    }

    /// Default `Action` to execute if the `Picker` did not pick any of the
    /// given choices.
    pub fn otherwise(mut self, otherwise: impl ActionSpawn + 'static) -> Self {
        self.idle = Some(ActionInner::new(Arc::new(otherwise)));
        self
    }
}

pub fn thinker_component_attach_system(
    mut cmd: Commands,
    query: Query<(Entity, &ThinkerBuilder), Without<HasThinker>>,
) {
    for (entity, thinker_builder) in query.iter() {
        debug!("Spawning Thinker.");

        let actor = Actor(entity);

        let thinker = cmd.spawn((actor, ActionState::Executing)).id();

        let choices = thinker_builder
            .choices
            .iter()
            .map(|ChoiceBuilder { when, then }| {
                let scorer = when.spawn(ScorerCommands::new(&mut cmd, actor));
                let action = ActionInner::new(then.clone());
                cmd.add(AddChild {
                    parent: thinker,
                    child: scorer.0,
                });
                Choice { scorer, action }
            })
            .collect();

        cmd.entity(thinker).insert(Thinker {
            // TODO: reasonable default?...
            picker: thinker_builder.picker.clone(),
            otherwise: thinker_builder.idle.clone(),
            choices,
            current_action: None,
            scheduled_actions: VecDeque::new(),
        });

        cmd.entity(entity).insert(HasThinker(thinker));
    }
}

pub fn thinker_component_detach_system(
    mut cmd: Commands,
    query: Query<(Entity, &HasThinker), Without<ThinkerBuilder>>,
) {
    for (actor, &HasThinker(thinker)) in query.iter() {
        if let Some(entity) = cmd.get_entity(thinker) {
            entity.despawn_recursive();
        }
        cmd.entity(actor).remove::<HasThinker>();
    }
}

pub fn actor_gone_cleanup(
    mut cmd: Commands,
    builders: Query<&ThinkerBuilder>,
    query: Query<(Entity, &Actor)>,
) {
    for (child, &Actor(actor)) in query.iter() {
        if builders.get(actor).is_err() {
            // Actor is gone. Let's clean up.
            if let Some(entity) = cmd.get_entity(child) {
                entity.despawn_recursive();
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

pub fn thinker_system(
    mut cmd: Commands,
    mut query: Query<(&mut ActionState, &Actor, &mut Thinker)>,
    scores: Query<&Score>,
    mut states: Query<&mut ActionState, Without<Thinker>>,
) {
    use ActionState::*;

    for (mut thinker_state, &actor, mut thinker) in query.iter_mut() {
        match *thinker_state {
            Executing => {
                if let Some(action) = thinker.picker.pick(&thinker.choices, &scores) {
                    // Think about what action we're supposed to be taking. We do this
                    // every tick, because we might change our mind.
                    // ...and then execute it (details below).
                    thinker.exec_picked_action(&mut cmd, actor, action, &mut states, true);
                } else if let Some(action) = thinker.schedule(&states) {
                    let new_action = action.spawn(ActionCommands::new(&mut cmd, actor));
                    debug!("scheduled {:?}", new_action);
                    thinker.current_action = Some((new_action, action));
                } else if let Some(default_action) = thinker.otherwise.clone() {
                    // Otherwise, let's just execute the default one! (if it's there)
                    thinker.exec_picked_action(&mut cmd, actor, default_action, &mut states, false);
                } else if let Some((action, _)) = thinker.current_action.as_ref() {
                    if states.get(action.entity()).unwrap().is_done() {
                        debug!("current {:?} is done, despawn", action);
                        cmd.add(action.despawn_recursive());
                        thinker.current_action = None;
                    }
                }
            }
            Cancelled => {
                debug!("Thinker cancelled. Cleaning up.");
                if let Some(current) = thinker.current_action.as_ref().map(|(c, _)| *c) {
                    debug!("Cancelling current action because thinker was cancelled.");
                    let mut current_state = states.get_mut(current.entity()).unwrap();
                    match *current_state {
                        Executing => {
                            debug!("Action is still executing. Attempting to cancel it before wrapping up Thinker cancellation.\nParent thinker was cancelled. Cancelling action.");
                            current_state.cancel();
                        }
                        Cancelled => debug!("Current action already cancelled."),
                        Success | Failure => {
                            debug!("Action already wrapped up on its own. Cleaning up action in Thinker.");
                            cmd.add(current.despawn_recursive());
                            thinker.current_action = None;
                        }
                    }
                } else {
                    debug!("No current thinker action. Wrapping up Thinker as Succeeded.");
                    thinker_state.success();
                }
            }
            Success | Failure => {}
        }
    }
}

impl Thinker {
    fn schedule(
        &mut self,
        states: &Query<&mut ActionState, Without<Thinker>>,
    ) -> Option<ActionInner> {
        if let Some((action, _)) = self.current_action.as_ref() {
            if !states.get(action.entity()).unwrap().is_done() {
                return None;
            }
        }

        self.scheduled_actions.pop_front()
    }

    #[allow(clippy::too_many_arguments)]
    fn exec_picked_action(
        &mut self,
        cmd: &mut Commands,
        actor: Actor,
        next: ActionInner,
        states: &mut Query<&mut ActionState, Without<Thinker>>,
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

        if let Some((action, current)) = &self.current_action {
            let mut current_state = states.get_mut(action.entity()).unwrap();
            let previous_done = current_state.is_done();
            if (!current.id_eq(&next) && override_current) || previous_done {
                // So we've picked a different action than we were
                // currently executing. Just like before, we grab the
                // actual Action component (and we assume it exists).
                // If the action is executing, or was requested, we
                // need to cancel it to make sure it stops.
                match *current_state {
                    Executing => {
                        debug!("still exec, cancel {:?}", action);
                        current_state.cancel();
                    }
                    Cancelled => {}
                    Success | Failure => {
                        debug!("completed, despawning {:?}", action);
                        cmd.add(action.despawn_recursive());

                        debug!("Spawning next action");

                        let next_action = next.spawn(ActionCommands::new(cmd, actor));
                        self.current_action = Some((next_action, next));
                    }
                };
            } else {
                // Otherwise, it turns out we want to keep executing
                // the same action. Just in case, we go ahead and set
                // it as requested if for some reason it had finished
                // but the Action System hasn't gotten around to
                // cleaning it up.
            }
        } else {
            // This branch arm is called when there's no
            // current_action in the thinker. The logic here is pretty
            // straightforward -- we set the action, Request it, and
            // that's it.
            debug!("No current action. Spawning new action.");

            let next_action = next.spawn(ActionCommands::new(cmd, actor));
            self.current_action = Some((next_action, next));
        }
    }
}
