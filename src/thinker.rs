//! Thinkers are the "brain" of an entity. You attach Scorers to it, and the
//! Thinker picks the right Action to run based on the resulting Scores.

use crate::{
    action::{Action, ActionCommands, ActionSpawn, ActionState},
    pickers::{Choice, ChoiceBuilder, FirstToScore, Highest, Picker},
    scorer::{Score, ScorerCommands, ScorerSpawn},
};
use bevy_asset::{Asset, Assets, Handle};
use bevy_ecs::{
    component::Component,
    entity::Entity,
    query::Without,
    system::{Commands, Query, Res},
};
use bevy_hierarchy::{DespawnRecursiveExt, PushChild};
use bevy_log as log;
use bevy_reflect::{Reflect, TypePath};
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
/// ### Example
///
/// ```
/// # use bevy::prelude::*;
/// # use big_brain::*;
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
/// pub fn init_entities(mut cmd: Commands, mut thinkers: ResMut<Assets<ThinkerSpawner>>) {
///     cmd.spawn((
///         Thirst(70.0, 2.0),
///         Hunger(50.0, 3.0),
///         thinkers.add(
///             ThinkerSpawner::first_to_score(80.0)
///                 .when(Thirsty, Drink)
///                 .when(Hungry, Eat)
///                 .when(FixedScorer::IDLE, Meander),
///         ),
///     ));
/// }
/// ```
#[derive(Component)]
pub struct Thinker {
    picker: Arc<dyn Picker>,
    choices: Vec<Choice>,
    current: Option<Action>,
    winner: Option<usize>,
    scheduled: VecDeque<Arc<dyn ActionSpawn>>,
}

impl Thinker {
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

pub fn thinker_system(
    mut cmd: Commands,
    mut query: Query<(&Actor, &mut Thinker)>,
    scores: Query<&Score>,
    mut states: Query<&mut ActionState>,
) {
    for (&actor, mut thinker) in query.iter_mut() {
        let next = thinker.picker.pick(&thinker.choices, &scores);

        if let Some(action) = thinker.current {
            let mut state = states.get_mut(action.entity()).unwrap();
            match state.clone() {
                ActionState::Executing => {
                    if thinker.has_scheduled() {
                        log::debug!("current {:?} cancel by scheduled", action);
                        state.cancel();
                    } else if let (Some(win), Some(next)) = (thinker.winner, next) {
                        if win != next {
                            log::debug!("current {:?} cancel by next", action);
                            state.cancel();
                        }
                    }
                    continue;
                }
                ActionState::Cancelled => continue,
                ActionState::Success | ActionState::Failure => {
                    log::debug!("current {:?} is done, despawn", action);
                    cmd.add(action.despawn_recursive());
                    thinker.current = None;
                    thinker.winner = None;
                }
            }
        }

        let cmd = ActionCommands::new(&mut cmd, actor);

        if let Some(action) = thinker.scheduled.pop_front() {
            let action = action.spawn(cmd);
            log::debug!("next scheduled {:?}", action);
            thinker.current = Some(action);
            thinker.winner = None;
        } else if let Some(index) = next {
            let action = thinker.choices[index].action.spawn(cmd);
            log::debug!("next picked {:?}", action);
            thinker.current = Some(action);
            thinker.winner = Some(index);
        }
    }
}

/// This is what you actually use to configure Thinker behavior.
#[derive(Clone, Asset, TypePath)]
pub struct ThinkerSpawner {
    picker: Arc<dyn Picker>,
    choices: Vec<ChoiceBuilder>,
}

impl ThinkerSpawner {
    /// Make a new [`ThinkerSpawner`] with given picker.
    pub fn new(picker: impl Picker + 'static) -> Self {
        Self {
            picker: Arc::new(picker),
            choices: Vec::new(),
        }
    }

    /// Make a new [`ThinkerSpawner`] with [`Highest`] picker.
    /// This is what you'll actually use to configure Thinker behavior.
    pub fn highest(threshold: f32) -> Self {
        Self::new(Highest { threshold })
    }

    /// Make a new [`ThinkerSpawner`] with [`FirstToScore`] picker.
    /// This is what you'll actually use to configure Thinker behavior.
    pub fn first_to_score(threshold: f32) -> Self {
        Self::new(FirstToScore { threshold })
    }

    /// Define an [`ScorerSpawn`] and [`ActionSpawn`] pair.
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
}

pub fn thinker_maintain_system(
    mut cmd: Commands,
    assets: Res<Assets<ThinkerSpawner>>,
    with_handle: Query<(Entity, &Handle<ThinkerSpawner>), Without<HasThinker>>,
    without_handle: Query<(Entity, &HasThinker), Without<Handle<ThinkerSpawner>>>,
) {
    for (actor, handle) in with_handle.iter() {
        log::debug!("Spawning Thinker for Actor({:?})", actor);

        let Some(builder) = assets.get(handle) else {
            log::error!("{:?} has broken {:?}", actor, handle);
            continue;
        };

        let parent = cmd.spawn(Actor(actor)).id();
        let choices = builder.choices.iter();

        let choices = choices.map(|ChoiceBuilder { when, then }| {
            let scorer = when.spawn(ScorerCommands::new(&mut cmd, Actor(actor)));
            let action = then.clone();
            cmd.add(PushChild {
                parent,
                child: scorer.0,
            });
            Choice { scorer, action }
        });

        let thinker = Thinker {
            picker: builder.picker.clone(),
            choices: choices.collect(),
            current: None,
            winner: None,
            scheduled: VecDeque::new(),
        };

        cmd.entity(parent).insert(thinker);
        cmd.entity(actor).insert(HasThinker(parent));
    }

    for (actor, &HasThinker(thinker)) in without_handle.iter() {
        if let Some(entity) = cmd.get_entity(thinker) {
            entity.despawn_recursive();
        }
        cmd.entity(actor).remove::<HasThinker>();
    }
}

pub fn actor_gone_cleanup(
    mut cmd: Commands,
    builders: Query<&Handle<ThinkerSpawner>>,
    query: Query<(Entity, &Actor)>,
) {
    for (child, actor) in query.iter() {
        if builders.get(actor.entity()).is_err() {
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
