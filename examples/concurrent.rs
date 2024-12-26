//! This example describes how to create a composite action that executes multiple sub-actions
//! concurrently.
//!
//! `Race` succeeds when any of the sub-actions succeed.
//! `Join` succeeds if all the sub-actions succeed.

use bevy::log::LogPlugin;
use bevy::prelude::*;
use bevy::utils::tracing::debug;
use big_brain::*;
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

/// An action where the actor has to guess a given number
#[derive(Clone, Component, Debug, ActionSpawn)]
pub struct GuessNumber {
    // Number to guess (between 0 and 10 included)
    to_guess: u8,
    // Rng to perform guesses
    rng: SmallRng,
}

impl GuessNumber {
    fn from_entropy(to_guess: u8) -> Self {
        let rng = SmallRng::from_entropy();
        Self { to_guess, rng }
    }
}

fn guess_number_action(
    // A query on all current MoveToWaterSource actions.
    mut action_query: Query<(ActionQuery, &mut GuessNumber)>,
) {
    // Loop through all actions, just like you'd loop over all entities in any other query.
    for (mut action, mut guess_number) in &mut action_query {
        // Different behavior depending on action state.
        match action.state() {
            ActionState::Executing => {
                // debug!("Let's try to guess the secret number: {:?}", guess_number.to_guess);

                // Guess a number. If we guessed right, succeed; else keep trying.
                let guess: u8 = guess_number.rng.gen_range(0..=10);
                debug!("Guessed: {:?}", guess);
                if guess == guess_number.to_guess {
                    debug!("Guessed the secret number: {:?}!", guess_number.to_guess);
                    action.success();
                }
            }

            // Always treat cancellations, or we might keep doing this forever!
            // You don't need to terminate immediately, by the way, this is only a flag that
            // the cancellation has been requested. If the actor is balancing on a tightrope,
            // for instance, you may let them walk off before ending the action.
            ActionState::Cancelled => action.failure(),

            ActionState::Success | ActionState::Failure => (),
        }
    }
}

// We will use a dummy scorer that always returns 1.0
#[derive(Clone, Component, Debug, ScorerSpawn)]
pub struct DummyScorer;

pub fn dummy_scorer_system(mut query: Query<ScorerQuery, With<DummyScorer>>) {
    for mut score in &mut query {
        score.set(1.0);
    }
}

pub fn init_entities(mut cmd: Commands, mut thinkers: ResMut<Assets<ThinkerSpawner>>) {
    let number_to_guess: u8 = 5;

    // We use the Race struct to build a composite action that will try to guess
    // multiple numbers. If any of the guesses are right, the whole `Race` action succeeds.
    let race_guess_numbers = Sequence::race((
        // ...try to guess a first number
        GuessNumber::from_entropy(number_to_guess),
        // ...try to guess a second number
        GuessNumber::from_entropy(number_to_guess),
    ));

    // We use the Join struct to build a composite action that will try to guess
    // multiple numbers. If all of the guesses are right, the whole `Race` action succeeds.
    let join_guess_numbers = Sequence::join((
        // ...try to guess a first number
        GuessNumber::from_entropy(number_to_guess),
        // ...try to guess a second number
        GuessNumber::from_entropy(number_to_guess),
    ));

    // We'll use `Steps` to execute a sequence of actions.
    // First, we'll guess the numbers with 'Race', and then we'll guess the numbers with 'Join'
    // See the `sequence.rs` example for more details.
    let steps_guess_numbers = Sequence::step((race_guess_numbers, join_guess_numbers));

    // Build the thinker
    // always select the action with the highest score
    let thinker = thinkers.add(ThinkerSpawner::highest(0.0).when(DummyScorer, steps_guess_numbers));

    cmd.spawn(HandleThinkerSpawner(thinker));
}

fn main() {
    // Once all that's done, we just add our systems and off we go!
    App::new()
        .add_plugins(DefaultPlugins.set(LogPlugin {
            // Use `RUST_LOG=big_brain=trace,thirst=trace cargo run --example thirst --features=trace` to see extra tracing output.
            filter: "big_brain=warn,concurrent=debug".to_string(),
            ..default()
        }))
        .add_plugins(BigBrainPlugin::new(Update, Update, PostUpdate, Last))
        .add_systems(Startup, init_entities)
        .add_systems(
            PreUpdate,
            (
                guess_number_action.in_set(BigBrainSet::Actions),
                dummy_scorer_system.in_set(BigBrainSet::Scorers),
            ),
        )
        .run();
}
