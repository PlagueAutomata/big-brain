//! * A series of
//!   [Measures](https://en.wikipedia.org/wiki/Measure_(mathematics)) used to
//!  * weight score.

use crate::prelude::Score;
use bevy::prelude::*;

/// A Measure trait describes a way to combine scores together.
#[reflect_trait]
pub trait Measure: std::fmt::Debug + Sync + Send {
    /// Calculates a score from the child scores
    fn calculate(&self, inputs: &[(Score, f32)]) -> f32;
}

/// A measure that adds all the elements together and multiplies them by the
/// weight.
#[derive(Debug, Clone, Reflect)]
pub struct WeightedSum;

impl Measure for WeightedSum {
    fn calculate(&self, scores: &[(Score, f32)]) -> f32 {
        scores
            .iter()
            .map(|(Score(score), weight)| score * weight)
            .sum()
    }
}

/// A measure that multiplies all the elements together.
#[derive(Debug, Clone, Reflect)]
pub struct WeightedProduct;

impl Measure for WeightedProduct {
    fn calculate(&self, scores: &[(Score, f32)]) -> f32 {
        scores
            .iter()
            .map(|(Score(score), weight)| score * weight)
            .product()
    }
}

/// A measure that returns the max of the weighted child scares based on the
/// one-dimensional (Chebychev
/// Distance)[https://en.wikipedia.org/wiki/Chebyshev_distance].
#[derive(Debug, Clone, Reflect)]
pub struct ChebyshevDistance;

impl Measure for ChebyshevDistance {
    fn calculate(&self, scores: &[(Score, f32)]) -> f32 {
        scores
            .iter()
            .map(|(Score(score), weight)| score * weight)
            .fold(0.0, |best, score| score.max(best))
    }
}

/// The default measure which uses a weight to provide an intuitive curve.
#[derive(Debug, Clone, Default, Reflect)]
pub struct WeightedMeasure;

impl Measure for WeightedMeasure {
    fn calculate(&self, scores: &[(Score, f32)]) -> f32 {
        let wsum: f32 = scores.iter().map(|(_, weight)| weight).sum();

        if wsum == 0.0 {
            return 0.0;
        }

        scores
            .iter()
            .map(|(Score(score), weight)| weight / wsum * score.powf(2.0))
            .sum::<f32>()
            .powf(0.5)
    }
}
