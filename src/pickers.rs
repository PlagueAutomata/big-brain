//! Pickers are used by Thinkers to determine which of its Scorers will "win".

use crate::{
    action::{ActionInner, ActionSpawn},
    scorer::{Score, Scorer, ScorerSpawn},
};
use bevy::prelude::*;
use std::sync::Arc;

/// Contains different types of Considerations and Actions
#[derive(Clone)]
pub struct Choice {
    pub(crate) scorer: Scorer,
    pub(crate) action: ActionInner,
}

impl Choice {
    pub fn calculate(&self, scores: &Query<&Score>) -> Score {
        scores
            .get(self.scorer.0)
            .cloned()
            .expect("Where did the score go?")
    }
}

/// Builds a new [`Choice`].
#[derive(Clone)]
pub struct ChoiceBuilder {
    pub when: Arc<dyn ScorerSpawn>,
    pub then: Arc<dyn ActionSpawn>,
}

/// Required trait for Pickers. A Picker is given a slice of choices and a
/// query that can be passed into `Choice::calculate`.
///
/// Implementations of `pick` must return `Some(Choice)` for the `Choice` that
/// was picked, or `None`.
pub trait Picker: Sync + Send {
    fn pick(&self, choices: &[Choice], scores: &Query<&Score>) -> Option<ActionInner>;
}

/// Picker that chooses the first `Choice` with a [`Score`] higher than its
/// configured `threshold`.
///
/// ### Example
///
/// ```
/// # use big_brain::prelude::*;
/// # fn main() {
/// Thinker::build(FirstToScore::new(0.8))
///     // .when(...)
/// # ;
/// # }
/// ```
#[derive(Debug, Clone, Default)]
pub struct FirstToScore {
    pub threshold: f32,
}

impl FirstToScore {
    pub fn new(threshold: f32) -> Self {
        Self { threshold }
    }
}

impl Picker for FirstToScore {
    fn pick(&self, choices: &[Choice], scores: &Query<&Score>) -> Option<ActionInner> {
        choices
            .iter()
            .find(|choice| choice.calculate(scores).0 >= self.threshold)
            .map(|Choice { action, .. }| action.clone())
    }
}

/// Picker that chooses the `Choice` with the highest non-zero [`Score`], and the first highest in case of a tie.
///
/// ### Example
///
/// ```
/// # use big_brain::prelude::*;
/// # fn main() {
/// Thinker::build(Highest)
///     // .when(...)
/// # ;
/// # }
/// ```
#[derive(Debug, Clone, Default)]
pub struct Highest;

impl Picker for Highest {
    fn pick(&self, choices: &[Choice], scores: &Query<&Score>) -> Option<ActionInner> {
        let mut max_score = 0.0;
        choices.iter().fold(None, |acc, choice| {
            let Score(score) = choice.calculate(scores);
            if score <= max_score || score <= 0.0 {
                acc
            } else {
                max_score = score;
                Some(choice.action.clone())
            }
        })
    }
}
