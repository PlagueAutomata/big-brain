//! * A series of
//!   [Measures](https://en.wikipedia.org/wiki/Measure_(mathematics)) used to
//!  * weight score.

use crate::scorer::{Score, Scorer, ScorerCommands, ScorerSpawn};
use bevy_ecs::{
    component::Component,
    entity::Entity,
    system::{Local, Query},
};
use bevy_hierarchy::Children;
use bevy_utils::all_tuples;
use std::sync::Arc;

pub trait WeightnedScorersList {
    fn build(scorers: Self) -> Vec<(Arc<dyn ScorerSpawn>, f32)>;
}

impl<T: ScorerSpawn + 'static> WeightnedScorersList for (T, f32) {
    fn build((scorer, weight): Self) -> Vec<(Arc<dyn ScorerSpawn>, f32)> {
        vec![(Arc::new(scorer), weight)]
    }
}

macro_rules! impl_weghtned_scorers_list {
    ($(($Type:ident, $index:ident)),*) => {
        impl< $($Type),* > WeightnedScorersList for ( $(($Type, f32),)* ) where $($Type: ScorerSpawn + 'static),* {
            fn build(($($index,)*): Self) -> Vec<(Arc<dyn ScorerSpawn>, f32)> {
                vec![ $((Arc::new($index.0), $index.1)),* ]
            }
        }
    }
}

all_tuples!(impl_weghtned_scorers_list, 1, 15, Type, index);

#[derive(Clone, Copy)]
pub struct WeightedScore {
    pub score: f32,
    pub weight: f32,
}

impl WeightedScore {
    fn product(&self) -> f32 {
        self.score * self.weight
    }
}

/// A [`Measure`] describes a way to combine scores together.
pub type Measure = fn(inputs: &[WeightedScore]) -> f32;

/// A measure that adds all the elements together and multiplies them by the weight.
pub fn weighted_sum(scores: &[WeightedScore]) -> f32 {
    scores.iter().map(WeightedScore::product).sum()
}

/// A measure that multiplies all the elements together.
pub fn weighted_product(scores: &[WeightedScore]) -> f32 {
    scores.iter().map(WeightedScore::product).product()
}

/// A measure that returns the max of the weighted child scares based on the
/// one-dimensional (Chebychev Distance)[https://en.wikipedia.org/wiki/Chebyshev_distance].
pub fn chebyshev_distance(scores: &[WeightedScore]) -> f32 {
    let product = scores.iter().map(WeightedScore::product);
    product.fold(0.0, |best, score| score.max(best))
}

/// The default measure which uses a weight to provide an intuitive curve.
pub fn weighted_measure(scores: &[WeightedScore]) -> f32 {
    let wsum = scores.iter().map(|s| s.weight).sum::<f32>();
    if wsum == 0.0 {
        0.0
    } else {
        let measure = |wscore: &WeightedScore| wscore.weight / wsum * wscore.score.powf(2.0);
        scores.iter().map(measure).sum::<f32>().powf(0.5)
    }
}

/// Composite Scorer that allows more fine-grained control of how the scores
/// are combined. The default is to apply a weighting
///
/// ### Example
///
/// ```
/// # use bevy::prelude::*;
/// # use big_brain::*;
/// # #[derive(Debug, Clone, Component, ScorerSpawn)]
/// # struct MyScorer;
/// # #[derive(Debug, Clone, Component, ScorerSpawn)]
/// # struct MyOtherScorer;
/// # #[derive(Debug, Clone, Component, ActionSpawn)]
/// # struct MyAction;
/// # fn main() {
/// ThinkerSpawner::highest(0.0)
///     .when(
///         MeasuredScorer::chebyshev(0.5, (
///             (MyScorer, 0.8),
///             (MyOtherScorer, 0.2)
///         )),
///         MyAction)
/// # ;
/// # }
/// ```
#[derive(Component)]
pub struct MeasuredScorer {
    threshold: f32,
    measure: Measure,
    weights: Vec<f32>,
}

impl MeasuredScorer {
    pub fn sum<B: WeightnedScorersList>(threshold: f32, scorers: B) -> impl ScorerSpawn {
        Self::custom(threshold, weighted_sum, scorers)
    }

    pub fn product<B: WeightnedScorersList>(threshold: f32, scorers: B) -> impl ScorerSpawn {
        Self::custom(threshold, weighted_product, scorers)
    }

    pub fn chebyshev<B: WeightnedScorersList>(threshold: f32, scorers: B) -> impl ScorerSpawn {
        Self::custom(threshold, chebyshev_distance, scorers)
    }

    pub fn measure<B: WeightnedScorersList>(threshold: f32, scorers: B) -> impl ScorerSpawn {
        Self::custom(threshold, weighted_measure, scorers)
    }

    pub fn custom<B: WeightnedScorersList>(
        threshold: f32,
        measure: Measure,
        scorers: B,
    ) -> impl ScorerSpawn {
        MeasuredScorerSpawner {
            threshold,
            measure,
            scorers: B::build(scorers),
        }
    }
}

pub struct MeasuredScorerSpawner {
    threshold: f32,
    measure: Measure,
    scorers: Vec<(Arc<dyn ScorerSpawn>, f32)>,
}

impl ScorerSpawn for MeasuredScorerSpawner {
    fn spawn(&self, mut cmd: ScorerCommands) -> Scorer {
        let scorer = cmd.spawn(MeasuredScorer {
            threshold: self.threshold,
            measure: self.measure,
            weights: self.scorers.iter().map(|&(_, weight)| weight).collect(),
        });

        for (child, _weight) in &self.scorers {
            cmd.push_child(scorer, child.as_ref());
        }

        scorer
    }
}

pub fn measured_scorers_system(
    mut cache: Local<Vec<WeightedScore>>,
    query: Query<(Entity, &MeasuredScorer, &Children)>,
    mut scores: Query<&mut Score>,
) {
    for (this_entity, this, children) in query.iter() {
        let weights = this.weights.iter().copied();
        let scorers = children.iter().map(|&e| scores.get(e).cloned().unwrap().0);
        let weighted = scorers.zip(weights);

        cache.extend(weighted.map(|(score, weight)| WeightedScore { score, weight }));

        let score = {
            let score = (this.measure)(&cache);
            let filter = score >= this.threshold;
            (if filter { score } else { 0.0 }).clamp(0.0, 1.0)
        };

        scores.get_mut(this_entity).unwrap().set(score);
        cache.clear();
    }
}
