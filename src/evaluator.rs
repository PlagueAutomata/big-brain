//! Utilities for turning values within a certain range into different curves.

use crate::scorer::{Score, ScorerSpawn, ScorerSpawner};
use bevy::prelude::*;
use std::sync::Arc;

/// Trait that any evaluators must implement.
/// Must return an `f32` value between `0.0..=100.0`.
#[reflect_trait]
pub trait Evaluator: Sync + Send {
    fn evaluate(&self, value: f32) -> f32;
}

/// Composite scorer that takes a `ScorerBuilder` and applies an `Evaluator`.
/// Note that unlike other composite scorers, `EvaluatingScorer` only takes
/// one scorer upon building.
///
/// ### Example
///
/// ```
/// # use bevy::prelude::*;
/// # use big_brain::prelude::*;
/// # #[derive(Debug, Clone, Component, ScorerSpawn)]
/// # struct MyScorer;
/// # #[derive(Debug, Clone, Component, ActionSpawn)]
/// # struct MyAction;
/// # #[derive(Debug, Clone)]
/// # struct MyEvaluator;
/// # impl Evaluator for MyEvaluator {
/// #    fn evaluate(&self, score: f32) -> f32 {
/// #        score
/// #    }
/// # }
/// # fn main() {
/// Thinker::build(Highest)
///     .when(EvaluatingScorer::build(MyScorer, MyEvaluator), MyAction)
/// # ;
/// # }
/// ```
#[derive(Component, Clone)]
pub struct EvaluatingScorer {
    evaluator: Arc<dyn Evaluator>,
}

impl EvaluatingScorer {
    pub fn build(
        scorer: impl ScorerSpawn + 'static,
        evaluator: impl Evaluator + 'static,
    ) -> impl ScorerSpawn {
        let evaluator = Arc::new(evaluator);
        ScorerSpawner::new(Self { evaluator }, scorer)
    }
}

pub fn evaluating_scorer_system(
    query: Query<(Entity, &EvaluatingScorer, &Children)>,
    mut scores: Query<&mut Score>,
) {
    for (this_entity, this, children) in query.iter() {
        let &inner = children.first().unwrap();
        let &Score(inner) = scores.get(inner).unwrap();
        let value = this.evaluator.evaluate(inner).clamp(0.0, 1.0);
        scores.get_mut(this_entity).unwrap().set(value);
    }
}

/// [`Evaluator`] for linear values.
/// That is, there's no curve to the value mapping.
#[derive(Debug, Clone, Reflect)]
pub struct LinearEvaluator {
    xa: f32,
    ya: f32,
    yb: f32,
    dy_over_dx: f32,
}

impl LinearEvaluator {
    pub fn inversed() -> Self {
        Self::new(1.0, 0.0, 1.0, 1.0)
    }

    pub fn ranged(min: f32, max: f32) -> Self {
        Self::new(min, 0.0, max, 1.0)
    }

    fn new(xa: f32, ya: f32, xb: f32, yb: f32) -> Self {
        Self {
            xa,
            ya,
            yb,
            dy_over_dx: (yb - ya) / (xb - xa),
        }
    }
}

impl Default for LinearEvaluator {
    fn default() -> Self {
        Self::new(0.0, 0.0, 1.0, 1.0)
    }
}

impl Evaluator for LinearEvaluator {
    fn evaluate(&self, value: f32) -> f32 {
        let value = self.ya + self.dy_over_dx * (value - self.xa);
        value.clamp(self.ya, self.yb)
    }
}

/// [`Evaluator`] with an exponent curve.
/// The value will grow according to its `power` parameter.
#[derive(Debug, Clone, Reflect)]
pub struct PowerEvaluator {
    xa: f32,
    ya: f32,
    xb: f32,
    power: f32,
    dy: f32,
}

impl PowerEvaluator {
    pub fn new(power: f32) -> Self {
        Self::new_full(power, 0.0, 0.0, 1.0, 1.0)
    }

    pub fn new_ranged(power: f32, min: f32, max: f32) -> Self {
        Self::new_full(power, min, 0.0, max, 1.0)
    }

    pub fn new_full(power: f32, xa: f32, ya: f32, xb: f32, yb: f32) -> Self {
        Self {
            power: power.clamp(0.0, 10000.0),
            dy: yb - ya,
            xa,
            ya,
            xb,
        }
    }
}

impl Default for PowerEvaluator {
    fn default() -> Self {
        Self::new(2.0)
    }
}

impl Evaluator for PowerEvaluator {
    fn evaluate(&self, value: f32) -> f32 {
        let cx = value.clamp(self.xa, self.xb);
        self.dy * ((cx - self.xa) / (self.xb - self.xa)).powf(self.power) + self.ya
    }
}

/// [`Evaluator`] with a "Sigmoid", or "S-like" curve.
#[derive(Debug, Clone, Reflect)]
pub struct SigmoidEvaluator {
    xa: f32,
    xb: f32,
    ya: f32,
    yb: f32,

    k: f32,
}

impl SigmoidEvaluator {
    pub fn new(k: f32) -> Self {
        Self::new_full(k, 0.0, 0.0, 1.0, 1.0)
    }

    pub fn new_ranged(k: f32, min: f32, max: f32) -> Self {
        Self::new_full(k, min, 0.0, max, 1.0)
    }

    pub fn new_full(k: f32, xa: f32, ya: f32, xb: f32, yb: f32) -> Self {
        Self {
            xa,
            xb,
            ya,
            yb,
            k: k.clamp(-0.99999, 0.99999),
        }
    }
}

impl Evaluator for SigmoidEvaluator {
    fn evaluate(&self, x: f32) -> f32 {
        let x_mean = (self.xa + self.xb) / 2.0;
        let y_mean = (self.ya + self.yb) / 2.0;
        let dy_over_two = (self.yb - self.ya) / 2.0;
        let one_minus_k = 1.0 - self.k;
        let two_over_dx = (2.0 / (self.xb - self.ya)).abs();

        let cx_minus_x_mean = x.clamp(self.xa, self.xb) - x_mean;
        let numerator = two_over_dx * cx_minus_x_mean * one_minus_k;
        let denominator = self.k * (1.0 - 2.0 * (two_over_dx * cx_minus_x_mean)).abs() + 1.0;
        (dy_over_two * (numerator / denominator) + y_mean).clamp(self.ya, self.yb)
    }
}

impl Default for SigmoidEvaluator {
    fn default() -> Self {
        Self::new(-0.5)
    }
}
