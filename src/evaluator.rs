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
pub struct EvaluatingScorer(Arc<dyn Evaluator>);

impl EvaluatingScorer {
    pub fn build(
        scorer: impl ScorerSpawn + 'static,
        evaluator: impl Evaluator + 'static,
    ) -> impl ScorerSpawn {
        let evaluator = Arc::new(evaluator);
        ScorerSpawner::new(Self(evaluator), scorer)
    }
}

pub fn evaluating_scorer_system(
    query: Query<(Entity, &EvaluatingScorer, &Children)>,
    mut scores: Query<&mut Score>,
) {
    for (this_entity, this, children) in query.iter() {
        let &inner = children.first().unwrap();
        let &Score(inner) = scores.get(inner).unwrap();
        let value = this.0.evaluate(inner).clamp(0.0, 1.0);
        scores.get_mut(this_entity).unwrap().set(value);
    }
}

/// [`Evaluator`] based on function.
pub struct FnEvaluator(Arc<dyn (Fn(f32) -> f32) + Sync + Send>);

impl FnEvaluator {
    pub fn new(f: impl (Fn(f32) -> f32) + Sync + Send + 'static) -> Self {
        Self(Arc::new(f))
    }

    pub fn linear(a: (f32, f32), b: (f32, f32)) -> Self {
        let e = Linear::new(a, b);
        Self::new(move |t| e.eval(t))
    }

    pub fn power(power: f32, a: (f32, f32), b: (f32, f32)) -> Self {
        let e = Power::new(power, a, b);
        Self::new(move |t| e.eval(t))
    }

    pub fn sigmoid(k: f32, a: (f32, f32), b: (f32, f32)) -> Self {
        let e = Sigmoid::new(k, a, b);
        Self::new(move |t| e.eval(t))
    }
}

impl Evaluator for FnEvaluator {
    fn evaluate(&self, value: f32) -> f32 {
        (self.0)(value)
    }
}

/// [`Evaluator`] with a "Sigmoid", or "S-like" curve.
#[derive(Debug, Clone, Copy, Reflect)]
pub struct Sigmoid {
    k: f32,
    min: f32,
    max: f32,
    x_mean: f32,
    y_mean: f32,
    half_dy: f32,
    two_over_dx: f32,
}

impl Evaluator for Sigmoid {
    fn evaluate(&self, value: f32) -> f32 {
        self.eval(value)
    }
}

impl Default for Sigmoid {
    fn default() -> Self {
        Self::new(-0.5, (0.0, 0.0), (1.0, 1.0))
    }
}

impl Sigmoid {
    #[inline]
    pub fn new(k: f32, (ax, ay): (f32, f32), (bx, by): (f32, f32)) -> Self {
        Self {
            k: k.clamp(-0.99999, 0.99999),
            min: ax.min(bx),
            max: ax.max(bx),
            x_mean: (ax + bx) / 2.0,
            y_mean: (ay + by) / 2.0,
            half_dy: (by - ay) / 2.0,
            two_over_dx: 2.0 / (bx - ax),
        }
    }

    #[inline]
    pub fn eval(self, t: f32) -> f32 {
        let t = t.clamp(self.min, self.max);
        let value = self.two_over_dx * (t - self.x_mean);
        let num = value * (1.0 - self.k);
        let den = self.k * (1.0 - 2.0 * value.abs()) + 1.0;
        self.half_dy * (num / den) + self.y_mean
    }
}

/// [`Evaluator`] with an exponent curve.
/// The value will grow according to its `power` parameter.
#[derive(Debug, Clone, Copy, Reflect)]
pub struct Power {
    power: f32,
    ax: f32,
    ay: f32,
    dx: f32,
    dy: f32,
    min: f32,
    max: f32,
}

impl Default for Power {
    fn default() -> Self {
        Self::new(2.0, (0.0, 0.0), (1.0, 1.0))
    }
}

impl Evaluator for Power {
    fn evaluate(&self, value: f32) -> f32 {
        self.eval(value)
    }
}

impl Power {
    #[inline]
    pub fn new(power: f32, (ax, ay): (f32, f32), (bx, by): (f32, f32)) -> Self {
        Self {
            power,
            ax,
            ay,
            dx: (bx - ax),
            dy: (by - ay),
            min: ax.min(bx),
            max: ax.max(bx),
        }
    }

    #[inline]
    pub fn eval(self, t: f32) -> f32 {
        let t = t.clamp(self.min, self.max);
        self.dy * ((t - self.ax) / self.dx).powf(self.power) + self.ay
    }
}

/// [`Evaluator`] for linear values.
/// That is, there's no curve to the value mapping.
#[derive(Debug, Clone, Copy, Reflect)]
pub struct Linear {
    ax: f32,
    ay: f32,
    min: f32,
    max: f32,
    dy_over_dx: f32,
}

impl Default for Linear {
    fn default() -> Self {
        Self::new((0.0, 0.0), (1.0, 1.0))
    }
}

impl Evaluator for Linear {
    fn evaluate(&self, value: f32) -> f32 {
        self.eval(value)
    }
}

impl Linear {
    #[inline]
    pub fn new((ax, ay): (f32, f32), (bx, by): (f32, f32)) -> Self {
        Self {
            ax,
            ay,
            min: ax.min(bx),
            max: ax.max(bx),
            dy_over_dx: (by - ay) / (bx - ax),
        }
    }

    #[inline]
    pub fn eval(self, t: f32) -> f32 {
        let t = t.clamp(self.min, self.max);
        self.ay + self.dy_over_dx * (t - self.ax)
    }
}

fn linear_simple(t: f32) -> f32 {
    t.clamp(0.0, 1.0)
}

pub fn power_simple(t: f32, power: f32) -> f32 {
    t.clamp(0.0, 1.0).powf(power)
}

fn sigmoid_simple(t: f32, k: f32) -> f32 {
    let k = k.clamp(-0.99999, 0.99999);
    let t = 2.0 * t.clamp(0.0, 1.0) - 1.0;
    (t - t * k) / (4.0 * k * (0.5 - t.abs()) + 2.0) + 0.5
}
