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
    current: Option<Action>,
    scheduled: VecDeque<ActionInner>,
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

    pub fn schedule(&mut self, action: impl ActionSpawn + 'static) {
        self.scheduled.push_back(Arc::new(action));
    }

    pub fn has_scheduled(&self) -> bool {
        !self.scheduled.is_empty()
    }

    pub fn current(&self) -> Option<Action> {
        self.current
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
        self.idle = Some(Arc::new(otherwise));
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

        let thinker = cmd.spawn(actor).id();

        let choices = thinker_builder
            .choices
            .iter()
            .map(|ChoiceBuilder { when, then }| {
                let scorer = when.spawn(ScorerCommands::new(&mut cmd, actor));
                let action = then.clone();
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
            current: None,
            scheduled: VecDeque::new(),
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
    mut query: Query<(&Actor, &mut Thinker)>,
    scores: Query<&Score>,
    states: Query<&ActionState>,
) {
    for (&actor, mut thinker) in query.iter_mut() {
        if let Some(action) = thinker.current() {
            if states.get(action.entity()).unwrap().is_done() {
                debug!("current {:?} is done, despawn", action);
                cmd.add(action.despawn_recursive());
                thinker.current = None;
            } else {
                // wait for success or failure
                continue;
            }
        }

        let next_action = thinker.scheduled.pop_front();
        let next_action = next_action.or_else(|| thinker.picker.pick(&thinker.choices, &scores));
        let next_action = next_action.or_else(|| thinker.otherwise.clone());

        if let Some(action) = next_action {
            let new_action = action.spawn(ActionCommands::new(&mut cmd, actor));
            debug!("next {:?}", new_action);
            thinker.current = Some(new_action);
        }
    }
}
