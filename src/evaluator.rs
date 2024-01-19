//! Utilities for turning values within a certain range into different curves.

use crate::scorer::{Score, ScorerSpawn, ScorerSpawner};
use bevy_ecs::{component::Component, entity::Entity, system::Query};
use bevy_hierarchy::Children;
use bevy_reflect::{reflect_trait, Reflect};
use std::sync::Arc;

/// Trait that any evaluators must implement.
/// Must return an `f32` value between `0.0..=100.0`.
#[reflect_trait]
pub trait Evaluator: Sync + Send {
    fn evaluate(&self, value: f32) -> f32;
}

/// Composite scorer that takes a [`ScorerSpawn`] and applies an [`Evaluator`].
/// Note that unlike other composite scorers, [`EvaluatingScorer`] only takes
/// one scorer upon building.
///
/// ### Example
///
/// ```
/// # use bevy::prelude::*;
/// # use big_brain::*;
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
/// ThinkerSpawner::highest(0.0)
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
    xb: f32,
    yb: f32,
}

impl LinearEvaluator {
    pub fn new((xa, ya): (f32, f32), (xb, yb): (f32, f32)) -> Self {
        Self { xa, ya, xb, yb }
    }
}

impl Default for LinearEvaluator {
    fn default() -> Self {
        Self::new((0.0, 0.0), (1.0, 1.0))
    }
}

impl Evaluator for LinearEvaluator {
    fn evaluate(&self, value: f32) -> f32 {
        let dy_over_dx = (self.yb - self.ya) / (self.xb - self.xa);
        (self.ya + dy_over_dx * (value - self.xa)).clamp(self.ya, self.yb)
    }
}

/// [`Evaluator`] with an exponent curve.
/// The value will grow according to its `power` parameter.
#[derive(Debug, Clone, Reflect)]
pub struct PowerEvaluator {
    power: f32,
    a: (f32, f32),
    b: (f32, f32),
}

impl PowerEvaluator {
    pub fn new(power: f32, (xa, ya): (f32, f32), (xb, yb): (f32, f32)) -> Self {
        Self {
            power: power.clamp(0.0, 10000.0),
            a: (xa, ya),
            b: (xb, yb),
        }
    }
}

impl Default for PowerEvaluator {
    fn default() -> Self {
        Self::new(2.0, (0.0, 0.0), (1.0, 1.0))
    }
}

impl Evaluator for PowerEvaluator {
    fn evaluate(&self, value: f32) -> f32 {
        let cx = value.clamp(self.a.0, self.b.0);
        let dx = self.b.0 - self.a.0;
        let dy = self.b.1 - self.a.1;
        dy * ((cx - self.a.0) / dx).powf(self.power) + self.a.1
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

    x_mean: f32,
    y_mean: f32,
    dy_over_two: f32,
    two_over_dx: f32,
}

impl SigmoidEvaluator {
    pub fn new_simple(k: f32) -> Self {
        Self::new(k, (0.0, 0.0), (1.0, 1.0))
    }

    pub fn new_ranged(k: f32, min: f32, max: f32) -> Self {
        Self::new(k, (min, 0.0), (max, 1.0))
    }

    pub fn new(k: f32, (xa, ya): (f32, f32), (xb, yb): (f32, f32)) -> Self {
        Self {
            xa,
            xb,
            ya,
            yb,

            k: k.clamp(-0.99999, 0.99999),

            x_mean: (xa + xb) / 2.0,
            y_mean: (ya + yb) / 2.0,

            dy_over_two: (yb - ya) / 2.0,
            two_over_dx: (2.0 / (xb - ya)).abs(),
        }
    }
}

impl Evaluator for SigmoidEvaluator {
    fn evaluate(&self, value: f32) -> f32 {
        let cx_minus_x_mean = value.clamp(self.xa, self.xb) - self.x_mean;
        let numerator = self.two_over_dx * cx_minus_x_mean * (1.0 - self.k);
        let denominator = self.k * (1.0 - 2.0 * (self.two_over_dx * cx_minus_x_mean)).abs() + 1.0;
        (self.dy_over_two * (numerator / denominator) + self.y_mean).clamp(self.ya, self.yb)
    }
}

impl Default for SigmoidEvaluator {
    fn default() -> Self {
        Self::new_simple(-0.5)
    }
}

pub fn linear_evaluator(
    min_x: f32,
    max_x: f32,
    min_y: f32,
    max_y: f32,
) -> impl Fn(f32) -> f32 + Copy {
    let dx = max_x - min_x;
    let dy = max_y - min_y;
    move |v| (min_y + (dy / dx) * (v - min_x))
}

pub fn power_evaluator(
    power: f32,
    min_x: f32,
    max_x: f32,
    min_y: f32,
    max_y: f32,
) -> impl Fn(f32) -> f32 + Copy {
    let dx = max_x - min_x;
    let dy = max_y - min_y;
    move |v| dy * (v - min_x / dx).powf(power) + min_y
}

pub fn sigmoid_evaluator(
    k: f32,
    min_x: f32,
    max_x: f32,
    min_y: f32,
    max_y: f32,
) -> impl Fn(f32) -> f32 + Copy {
    let dx = max_x - min_x;
    let dy = max_y - min_y;

    let k = k.clamp(-0.99999, 0.99999);

    let x_mean = (min_x + max_x) / 2.0;
    let y_mean = (min_y + max_y) / 2.0;

    let dy_over_two = dy / 2.0;
    let two_over_dx = (2.0 / dx).abs();

    move |v| {
        let cx_minus_x_mean = v.clamp(min_x, max_x) - x_mean;
        let numerator = two_over_dx * cx_minus_x_mean * (1.0 - k);
        let denominator = k * (1.0 - 2.0 * (two_over_dx * cx_minus_x_mean)).abs() + 1.0;
        (dy_over_two * (numerator / denominator) + y_mean).clamp(min_y, max_y)
    }
}
