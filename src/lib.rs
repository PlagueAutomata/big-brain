//! [![crates.io](https://img.shields.io/crates/v/big-brain.svg)](https://crates.io/crates/big-brain)
//! [![docs.rs](https://docs.rs/big-brain/badge.svg)](https://docs.rs/big-brain)
//! [![Apache
//! 2.0](https://img.shields.io/badge/license-Apache-blue.svg)](./LICENSE.md)
//!
//! `big-brain` is a [Utility
//! AI](https://en.wikipedia.org/wiki/Utility_system) library for games, built
//! for the [Bevy Game Engine](https://bevyengine.org/)
//!
//! It lets you define complex, intricate AI behaviors for your entities based
//! on their perception of the world. Definitions are heavily data-driven,
//! using plain Rust, and you only need to program Scorers (entities that look
//! at your game world and come up with a Score), and Actions (entities that
//! perform actual behaviors upon the world). No other code is needed for
//! actual AI behavior.
//!
//! See [the documentation](https://docs.rs/big-brain) for more details.
//!
//! ### Features
//!
//! * Highly concurrent/parallelizable evaluation.
//! * Integrates smoothly with Bevy.
//! * Proven game AI model.
//! * Highly composable and reusable.
//! * State machine-style continuous actions/behaviors.
//! * Action cancellation.
//!
//! ### Example
//!
//! As a developer, you write application-dependent code to define
//! [`Scorers`](#scorers) and [`Actions`](#actions), and then put it all
//! together like building blocks, using [`Thinkers`](#thinkers) that will
//! define the actual behavior.
//!
//! #### Scorers
//!
//! `Scorer`s are entities that look at the world and evaluate into `Score`
//! values. You can think of them as the "eyes" of the AI system. They're a
//! highly-parallel way of being able to look at the `World` and use it to
//! make some decisions later.
//!
//! ```rust
//! use bevy::prelude::*;
//! use big_brain::*;
//! # #[derive(Component, Debug)]
//! # struct Thirst { thirst: f32 }
//!
//! #[derive(Debug, Clone, Component, ScorerSpawn)]
//! pub struct Thirsty;
//!
//! pub fn thirsty_scorer_system(
//!     thirsts: Query<&Thirst>,
//!     mut query: Query<ScorerQuery, With<Thirsty>>,
//! ) {
//!     for mut score in query.iter_mut() {
//!         if let Ok(thirst) = thirsts.get(score.actor()) {
//!             score.set(thirst.thirst);
//!         }
//!     }
//! }
//! ```
//!
//! #### Actions
//!
//! `Action`s are the actual things your entities will _do_. They are
//! connected to `ActionState`s that represent the current execution state of
//! the state machine.
//!
//! ```rust
//! use bevy::prelude::*;
//! use big_brain::*;
//! # #[derive(Component, Debug)]
//! # struct Thirst { thirst: f32 }
//!
//! #[derive(Debug, Clone, Component, ActionSpawn)]
//! pub struct Drink;
//!
//! fn drink_action_system(
//!     mut thirsts: Query<&mut Thirst>,
//!     mut query: Query<ActionQuery, With<Drink>>,
//! ) {
//!     for mut action in query.iter_mut() {
//!         let Ok(mut thirst) = thirsts.get_mut(action.actor()) else {
//!             continue;
//!         };
//!
//!         match action.state() {
//!             ActionState::Executing => {
//!                 thirst.thirst = 10.0;
//!                 action.success();
//!             }
//!             ActionState::Cancelled => action.failure(),
//!             ActionState::Success | ActionState::Failure => (),
//!         }
//!     }
//! }
//! ```
//!
//! #### Thinkers
//!
//! Finally, you can use it when define the `Thinker`, which you can attach as
//! a regular Component:
//!
//! ```rust
//! # use bevy::prelude::*;
//! # use big_brain::*;
//! # #[derive(Debug, Component)]
//! # struct Thirst(f32, f32);
//! # #[derive(Debug, Clone, Component, ScorerSpawn)]
//! # struct Thirsty;
//! # #[derive(Debug, Clone, Component, ActionSpawn)]
//! # struct Drink;
//! fn spawn_entity(cmd: &mut Commands, mut thinkers: ResMut<Assets<ThinkerSpawner>>) {
//!     cmd.spawn((
//!         Thirst(70.0, 2.0),
//!         HandleThinkerSpawner(thinkers.add(ThinkerSpawner::first_to_score(0.8).when(Thirsty, Drink))),
//!     ));
//! }
//! ```
//!
//! #### App
//!
//! Once all that's done, we just add our systems and off we go!
//!
//! ```no_run
//! # use bevy::prelude::*;
//! # use big_brain::*;
//! # fn init_entities() {}
//! # fn thirst_system() {}
//! # fn drink_action_system() {}
//! # fn thirsty_scorer_system() {}
//! fn main() {
//!     App::new()
//!         .add_plugins(DefaultPlugins)
//!         .add_plugins(BigBrainPlugin::new(Update, Update, PostUpdate, Last))
//!         .add_systems(Startup, init_entities)
//!         .add_systems(Update, thirst_system)
//!         .add_systems(PreUpdate, drink_action_system.in_set(BigBrainSet::Actions))
//!         .add_systems(PreUpdate, thirsty_scorer_system.in_set(BigBrainSet::Scorers))
//!         .run();
//! }
//! ```
//!
//! ### bevy version and MSRV
//!
//! The current version of `big-brain` is compatible with `bevy` 0.12.1.
//!
//! The Minimum Supported Rust Version for `big-brain` should be considered to
//! be the same as `bevy`'s, which as of the time of this writing was "the
//! latest stable release".
//!
//! ### Reflection
//!
//! All relevant `big-brain` types implement the bevy `Reflect` trait, so you
//! should be able to get some useful display info while using things like
//! [`bevy_inspector_egui`](https://crates.io/crates/bevy_inspector_egui).
//!
//! This implementation should **not** be considered stable, and individual
//! fields made visible may change at **any time** and not be considered
//! towards semver. Please use this feature **only for debugging**.
//!
//! ### Contributing
//!
//! 1. Install the latest Rust toolchain (stable supported).
//! 2. `cargo run --example thirst`
//! 3. Happy hacking!
//!
//! ### License
//!
//! This project is licensed under [the Apache-2.0 License](LICENSE.md).

mod action;
mod evaluator;
mod measures;
mod pickers;
mod scorer;
mod sequence;
mod thinker;

pub use big_brain_derive::{ActionSpawn, ScorerSpawn};

pub use crate::{
    action::{Action, ActionCommands, ActionQuery, ActionSpawn, ActionState},
    evaluator::{EvaluatingScorer, Evaluator, FnEvaluator, Linear, Power, Sigmoid},
    measures::{Measure, MeasuredScorer, WeightedScore},
    pickers::{FirstToScore, Highest, Picker},
    scorer::{
        AllOrNothing, CompensatedProductOfScorers, FixedScorer, ProductOfScorers, Score, Scorer,
        ScorerCommands, ScorerQuery, ScorerSpawn, ScorerSpawner, SumOfScorers, WinningScorer,
    },
    sequence::{Sequence, SequenceMode, SequenceSpawner},
    thinker::{Actor, HandleThinkerSpawner, HasThinker, Thinker, ThinkerSpawner},
};

use bevy_app::{App, Plugin};
use bevy_asset::AssetApp;
use bevy_ecs::intern::Interned;
use bevy_ecs::schedule::{IntoSystemConfigs, ScheduleLabel, SystemSet};

/// Core [`Plugin`] for Big Brain behavior. Required for any of the
/// [`Thinker`]-related magic to work.
///
/// ### Example
///
/// ```no_run
/// use bevy::prelude::*;
/// use big_brain::*;
///
/// App::new()
///     .add_plugins((DefaultPlugins, BigBrainPlugin::new(Update, Update, PostUpdate, Last)))
///     // ...insert entities and other systems.
///     .run();
#[derive(Debug, Clone)]
pub struct BigBrainPlugin {
    scorers: Interned<dyn ScheduleLabel>,
    actions: Interned<dyn ScheduleLabel>,
    sequence: Interned<dyn ScheduleLabel>,
    thinker: Interned<dyn ScheduleLabel>,
}

impl BigBrainPlugin {
    /// Create the BigBrain plugin which runs the scorers,
    /// thinker and actions in the specified schedule
    pub fn new(
        scorers: impl ScheduleLabel,
        actions: impl ScheduleLabel,
        sequence: impl ScheduleLabel,
        thinker: impl ScheduleLabel,
    ) -> Self {
        Self {
            scorers: scorers.intern(),
            actions: actions.intern(),
            sequence: sequence.intern(),
            thinker: thinker.intern(),
        }
    }
}

impl Plugin for BigBrainPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<crate::thinker::ThinkerSpawner>()
            .configure_sets(self.scorers.intern(), BigBrainSet::Scorers)
            .configure_sets(self.actions.intern(), BigBrainSet::Actions)
            .configure_sets(self.sequence.intern(), BigBrainSet::Sequence)
            .configure_sets(self.thinker.intern(), BigBrainSet::Thinker)
            .add_systems(
                self.scorers.intern(),
                (
                    crate::scorer::idle_scorer_system,
                    crate::scorer::fixed_scorer_system,
                    crate::measures::measured_scorers_system,
                    crate::scorer::all_or_nothing_system,
                    crate::scorer::sum_of_scorers_system,
                    crate::scorer::product_of_scorers_system,
                    crate::scorer::compensated_product_of_scorers_system,
                    crate::scorer::winning_scorer_system,
                    crate::evaluator::evaluating_scorer_system,
                )
                    .in_set(BigBrainSet::Scorers),
            )
            .add_systems(
                self.sequence,
                crate::sequence::sequence_system.in_set(BigBrainSet::Sequence),
            )
            .add_systems(
                self.scorers.intern(),
                (
                    crate::thinker::thinker_maintain_system,
                    crate::thinker::thinker_system,
                    crate::thinker::actor_gone_cleanup,
                )
                    .chain()
                    .in_set(BigBrainSet::Thinker),
            );
    }
}

/// [`BigBrainPlugin`] system sets. Use these to schedule your own
/// actions/scorers/etc.
#[derive(Clone, Debug, Hash, Eq, PartialEq, SystemSet)]
pub enum BigBrainSet {
    /// Scorers are evaluated in this set.
    Scorers,
    /// Actions are executed in this set.
    Actions,
    /// Sequence are executed in this set.
    Sequence,
    /// Thinkers run their logic in this set.
    Thinker,
}
