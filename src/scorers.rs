//! Scorers look at the world and boil down arbitrary characteristics into a
//! range of 0.0..=1.0. This module includes the ScorerBuilder trait and some
//! built-in Composite Scorers.

use crate::{
    evaluators::Evaluator,
    measures::{Measure, WeightedMeasure},
    thinker::{Actor, Scorer},
};
use bevy::prelude::*;
use std::sync::Arc;

/// Score value between `0.0..=1.0` associated with a Scorer.
#[derive(Component, Clone, Debug, Default, Reflect)]
pub struct Score(pub f32);

impl Score {
    /// Returns the `Score`'s current value.
    pub fn get(&self) -> f32 {
        self.0
    }

    /// Set the `Score`'s value.
    ///
    /// ### Panics
    ///
    /// Panics if `value` isn't within `0.0..=1.0`.
    pub fn set(&mut self, value: f32) {
        if !(0.0..=1.0).contains(&value) {
            panic!("Score value must be between 0.0 and 1.0");
        }
        self.0 = value;
    }

    /// Set the `Score`'s value. Allows values outside the range `0.0..=1.0`
    /// WARNING: `Scorer`s are significantly harder to compose when there
    /// isn't a set scale. Avoid using unless it's not feasible to rescale
    /// and use `set` instead.
    pub fn set_unchecked(&mut self, value: f32) {
        self.0 = value;
    }
}

/// Trait that must be defined by types in order to be `ScorerBuilder`s.
/// `ScorerBuilder`s' job is to spawn new `Scorer` entities. In general, most
/// of this is already done for you, and the only method you really have to
/// implement is `.build()`.
///
/// The `build()` method MUST be implemented for any `ScorerBuilder`s you want
/// to define.
#[reflect_trait]
pub trait ScorerBuilder: Sync + Send {
    fn spawn(&self, cmd: &mut Commands, actor: Actor) -> Scorer;
}

/// Scorer that always returns the same, fixed score. Good for combining with
/// things creatively!
#[derive(Clone, Component, Debug, Reflect)]
pub struct FixedScore(pub f32);

impl ScorerBuilder for FixedScore {
    fn spawn(&self, cmd: &mut Commands, actor: Actor) -> Scorer {
        Scorer(cmd.spawn((actor, Score::default(), self.clone())).id())
    }
}

pub fn fixed_score_system(mut query: Query<(&mut Score, &FixedScore)>) {
    for (mut score, &FixedScore(fixed)) in query.iter_mut() {
        score.set(fixed);
    }
}

/// Composite Scorer that takes any number of other Scorers and returns the
/// sum of their [`Score`] values if each _individual_ [`Score`] is at or
/// above the configured `threshold`.
///
/// ### Example
///
/// ```
/// # use bevy::prelude::*;
/// # use big_brain::prelude::*;
/// # #[derive(Debug, Clone, Component, ScorerBuilder)]
/// # struct MyScorer;
/// # #[derive(Debug, Clone, Component, ScorerBuilder)]
/// # struct MyOtherScorer;
/// # #[derive(Debug, Clone, Component, ActionBuilder)]
/// # struct MyAction;
/// # fn main() {
/// Thinker::build()
///     .when(
///         AllOrNothing::build(0.8)
///           .push(MyScorer)
///           .push(MyOtherScorer),
///         MyAction);
/// # ;
/// # }
/// ```
#[derive(Component, Debug, Reflect)]
pub struct AllOrNothing {
    threshold: f32,
}

impl AllOrNothing {
    pub fn build(threshold: f32) -> AllOrNothingBuilder {
        AllOrNothingBuilder {
            threshold,
            scorers: Vec::new(),
        }
    }
}

pub fn all_or_nothing_system(
    query: Query<(Entity, &AllOrNothing, &Children)>,
    mut scores: Query<&mut Score>,
) {
    for (aon_ent, AllOrNothing { threshold }, scorers) in query.iter() {
        let mut sum = 0.0;
        for &child in scorers.iter() {
            let score = scores.get_mut(child).expect("where is it?");
            if score.0 < *threshold {
                sum = 0.0;
                break;
            } else {
                sum += score.0;
            }
        }
        let mut score = scores.get_mut(aon_ent).expect("where did it go?");
        score.set(crate::evaluators::clamp(sum, 0.0, 1.0));
    }
}

#[derive(Clone, Reflect)]
pub struct AllOrNothingBuilder {
    threshold: f32,
    #[reflect(ignore)]
    scorers: Vec<Arc<dyn ScorerBuilder>>,
}

impl AllOrNothingBuilder {
    /// Add another Scorer to this [`ScorerBuilder`].
    pub fn push(mut self, scorer: impl ScorerBuilder + 'static) -> Self {
        self.scorers.push(Arc::new(scorer));
        self
    }
}

impl ScorerBuilder for AllOrNothingBuilder {
    fn spawn(&self, cmd: &mut Commands, actor: Actor) -> Scorer {
        let scorer = cmd.spawn_empty().id();

        let scorers: Vec<_> = self
            .scorers
            .iter()
            .map(|scorer| scorer.spawn(cmd, actor).0)
            .collect();

        cmd.entity(scorer).push_children(&scorers[..]).insert((
            actor,
            Score::default(),
            AllOrNothing {
                threshold: self.threshold,
            },
        ));

        Scorer(scorer)
    }
}

/// Composite Scorer that takes any number of other Scorers and returns the sum of their [`Score`] values if the _total_ summed [`Score`] is at or above the configured `threshold`.
///
/// ### Example
///
/// ```
/// # use bevy::prelude::*;
/// # use big_brain::prelude::*;
/// # #[derive(Debug, Clone, Component, ScorerBuilder)]
/// # struct MyScorer;
/// # #[derive(Debug, Clone, Component, ScorerBuilder)]
/// # struct MyOtherScorer;
/// # #[derive(Debug, Clone, Component, ActionBuilder)]
/// # struct MyAction;
/// # fn main() {
/// Thinker::build()
///     .when(
///         SumOfScorers::build(0.8)
///           .push(MyScorer)
///           .push(MyOtherScorer),
///         MyAction)
/// # ;
/// # }
/// ```
#[derive(Component, Debug, Reflect)]
pub struct SumOfScorers {
    threshold: f32,
    scorers: Vec<Scorer>,
}

impl SumOfScorers {
    pub fn build(threshold: f32) -> SumOfScorersBuilder {
        SumOfScorersBuilder {
            threshold,
            scorers: Vec::new(),
        }
    }
}

pub fn sum_of_scorers_system(query: Query<(Entity, &SumOfScorers)>, mut scores: Query<&mut Score>) {
    for (sos_ent, SumOfScorers { threshold, scorers }) in query.iter() {
        let mut sum = 0.0;
        for Scorer(child) in scorers.iter() {
            let score = scores.get_mut(*child).expect("where is it?");
            sum += score.0;
        }
        if sum < *threshold {
            sum = 0.0;
        }
        let mut score = scores.get_mut(sos_ent).expect("where did it go?");
        score.set(crate::evaluators::clamp(sum, 0.0, 1.0));
    }
}

#[derive(Clone, Reflect)]
pub struct SumOfScorersBuilder {
    threshold: f32,
    #[reflect(ignore)]
    scorers: Vec<Arc<dyn ScorerBuilder>>,
}

impl SumOfScorersBuilder {
    /// Add a new Scorer to this [`SumOfScorersBuilder`].
    pub fn push(mut self, scorer: impl ScorerBuilder + 'static) -> Self {
        self.scorers.push(Arc::new(scorer));
        self
    }
}

impl ScorerBuilder for SumOfScorersBuilder {
    #[allow(clippy::needless_collect)]
    fn spawn(&self, cmd: &mut Commands, actor: Actor) -> Scorer {
        let scorers: Vec<_> = self
            .scorers
            .iter()
            .map(|scorer| scorer.spawn(cmd, actor).0)
            .collect();

        Scorer(
            cmd.spawn((actor, Score::default()))
                .push_children(&scorers[..])
                .insert(SumOfScorers {
                    threshold: self.threshold,
                    scorers: scorers.into_iter().map(Scorer).collect(),
                })
                .id(),
        )
    }
}

/// Composite Scorer that takes any number of other Scorers and returns the
/// product of their [`Score`]. If the resulting score is less than the
/// threshold, it returns 0.
///
/// The Scorer can also apply a compensation factor based on the number of
/// Scores passed to it. This can be enabled by passing `true` to the
/// `use_compensation` method on the builder.
///
/// ### Example
///
/// ```
/// # use bevy::prelude::*;
/// # use big_brain::prelude::*;
/// # #[derive(Debug, Clone, Component, ScorerBuilder)]
/// # struct MyScorer;
/// # #[derive(Debug, Clone, Component, ScorerBuilder)]
/// # struct MyOtherScorer;
/// # #[derive(Debug, Clone, Component, ActionBuilder)]
/// # struct MyAction;
/// # fn main() {
/// Thinker::build()
///     .when(
///         ProductOfScorers::build(0.5)
///           .use_compensation(true)
///           .push(MyScorer)
///           .push(MyOtherScorer),
///         MyAction)
/// # ;
/// # }
/// ```

#[derive(Component, Debug, Reflect)]
pub struct ProductOfScorers {
    threshold: f32,
    use_compensation: bool,
}

impl ProductOfScorers {
    pub fn build(threshold: f32) -> ProductOfScorersBuilder {
        ProductOfScorersBuilder {
            threshold,
            use_compensation: false,
            scorers: Vec::new(),
        }
    }
}

pub fn product_of_scorers_system(
    query: Query<(Entity, &ProductOfScorers, &Children)>,
    mut scores: Query<&mut Score>,
) {
    for (
        sos_ent,
        ProductOfScorers {
            threshold,
            use_compensation,
        },
        scorers,
    ) in query.iter()
    {
        let mut product = 1.0;
        let mut num_scorers = 0;

        for &child in scorers.iter() {
            let score = scores.get_mut(child).expect("where is it?");
            product *= score.0;
            num_scorers += 1;
        }

        // See for example
        // http://www.gdcvault.com/play/1021848/Building-a-Better-Centaur-AI
        if *use_compensation && product < 1.0 {
            let mod_factor = 1.0 - 1.0 / (num_scorers as f32);
            let makeup = (1.0 - product) * mod_factor;
            product += makeup * product;
        }

        if product < *threshold {
            product = 0.0;
        }

        let mut score = scores.get_mut(sos_ent).expect("where did it go?");
        score.set(product.clamp(0.0, 1.0));
    }
}

#[derive(Clone)]
pub struct ProductOfScorersBuilder {
    threshold: f32,
    use_compensation: bool,
    scorers: Vec<Arc<dyn ScorerBuilder>>,
}

impl ProductOfScorersBuilder {
    /// To account for the fact that the total score will be reduced for
    /// scores with more inputs, we can optionally apply a compensation factor
    /// by calling this and passing `true`
    pub fn use_compensation(mut self, use_compensation: bool) -> Self {
        self.use_compensation = use_compensation;
        self
    }

    /// Add a new scorer to this [`ProductOfScorersBuilder`].
    pub fn push(mut self, scorer: impl ScorerBuilder + 'static) -> Self {
        self.scorers.push(Arc::new(scorer));
        self
    }
}

impl ScorerBuilder for ProductOfScorersBuilder {
    #[allow(clippy::needless_collect)]
    fn spawn(&self, cmd: &mut Commands, actor: Actor) -> Scorer {
        let scorers: Vec<_> = self
            .scorers
            .iter()
            .map(|scorer| scorer.spawn(cmd, actor).0)
            .collect();

        Scorer(
            cmd.spawn((actor, Score::default()))
                .push_children(&scorers[..])
                .insert(ProductOfScorers {
                    threshold: self.threshold,
                    use_compensation: self.use_compensation,
                })
                .id(),
        )
    }
}

/// Composite Scorer that takes any number of other Scorers and returns the
/// single highest value [`Score`] if  _any_ [`Score`]s are at or above the
/// configured `threshold`.
///
/// ### Example
///
/// ```
/// # use bevy::prelude::*;
/// # use big_brain::prelude::*;
/// # #[derive(Debug, Clone, Component, ScorerBuilder)]
/// # struct MyScorer;
/// # #[derive(Debug, Clone, Component, ScorerBuilder)]
/// # struct MyOtherScorer;
/// # #[derive(Debug, Clone, Component, ActionBuilder)]
/// # struct MyAction;
/// # fn main() {
/// Thinker::build()
///     .when(
///         WinningScorer::build(0.8)
///           .push(MyScorer)
///           .push(MyOtherScorer),
///         MyAction)
/// # ;
/// # }
/// ```

#[derive(Component, Debug, Reflect)]
pub struct WinningScorer {
    threshold: f32,
}

impl WinningScorer {
    pub fn build(threshold: f32) -> WinningScorerBuilder {
        WinningScorerBuilder {
            threshold,
            scorers: Vec::new(),
        }
    }
}

pub fn winning_scorer_system(
    query: Query<(Entity, &WinningScorer, &Children)>,
    mut scores: Query<&mut Score>,
) {
    for (this_entity, this, children) in query.iter() {
        let mut children: Vec<Score> = children
            .iter()
            .map(|&entity| scores.get(entity).cloned().unwrap())
            .collect::<_>();

        children.sort_by(|Score(a), Score(b)| f32::total_cmp(a, b));

        let Score(winning_score_or_zero) = children
            .last()
            .cloned()
            .filter(|&Score(s)| s >= this.threshold)
            .unwrap_or(Score(0.0));

        let mut score = scores.get_mut(this_entity).expect("where did it go?");
        score.set(crate::evaluators::clamp(winning_score_or_zero, 0.0, 1.0));
    }
}

#[derive(Clone, Reflect)]
pub struct WinningScorerBuilder {
    threshold: f32,
    #[reflect(ignore)]
    scorers: Vec<Arc<dyn ScorerBuilder>>,
}

impl WinningScorerBuilder {
    /// Add another Scorer to this [`WinningScorerBuilder`].
    pub fn push(mut self, scorer: impl ScorerBuilder + 'static) -> Self {
        self.scorers.push(Arc::new(scorer));
        self
    }
}

impl ScorerBuilder for WinningScorerBuilder {
    #[allow(clippy::needless_collect)]
    fn spawn(&self, cmd: &mut Commands, actor: Actor) -> Scorer {
        let scorers: Vec<_> = self
            .scorers
            .iter()
            .map(|scorer| scorer.spawn(cmd, actor).0)
            .collect();

        Scorer(
            cmd.spawn((
                actor,
                Score::default(),
                WinningScorer {
                    threshold: self.threshold,
                },
            ))
            .push_children(&scorers[..])
            .id(),
        )
    }
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
/// # #[derive(Debug, Clone, Component, ScorerBuilder)]
/// # struct MyScorer;
/// # #[derive(Debug, Clone, Component, ActionBuilder)]
/// # struct MyAction;
/// # #[derive(Debug, Clone)]
/// # struct MyEvaluator;
/// # impl Evaluator for MyEvaluator {
/// #    fn evaluate(&self, score: f32) -> f32 {
/// #        score
/// #    }
/// # }
/// # fn main() {
/// Thinker::build()
///     .when(
///         EvaluatingScorer::build(MyScorer, MyEvaluator),
///         MyAction)
/// # ;
/// # }
/// ```
#[derive(Component, Reflect)]
#[reflect(from_reflect = false)]
pub struct EvaluatingScorer {
    scorer: Scorer,
    #[reflect(ignore)]
    evaluator: Arc<dyn Evaluator>,
}

impl EvaluatingScorer {
    pub fn build(
        scorer: impl ScorerBuilder + 'static,
        evaluator: impl Evaluator + 'static,
    ) -> EvaluatingScorerBuilder {
        EvaluatingScorerBuilder {
            evaluator: Arc::new(evaluator),
            scorer: Arc::new(scorer),
        }
    }
}

pub fn evaluating_scorer_system(
    query: Query<(Entity, &EvaluatingScorer)>,
    mut scores: Query<&mut Score>,
) {
    for (this_entity, this) in query.iter() {
        let &Score(inner) = scores.get(this.scorer.0).unwrap();
        let value = this.evaluator.evaluate(inner);
        let mut score = scores.get_mut(this_entity).unwrap();
        score.set(crate::evaluators::clamp(value, 0.0, 1.0));
    }
}

#[derive(Reflect)]
#[reflect(from_reflect = false)]
pub struct EvaluatingScorerBuilder {
    #[reflect(ignore)]
    scorer: Arc<dyn ScorerBuilder>,
    #[reflect(ignore)]
    evaluator: Arc<dyn Evaluator>,
}

impl ScorerBuilder for EvaluatingScorerBuilder {
    fn spawn(&self, cmd: &mut Commands, actor: Actor) -> Scorer {
        let inner_scorer = self.scorer.spawn(cmd, actor);
        Scorer(
            cmd.spawn((
                actor,
                Score::default(),
                EvaluatingScorer {
                    evaluator: self.evaluator.clone(),
                    scorer: inner_scorer,
                },
            ))
            .add_child(inner_scorer.0)
            .id(),
        )
    }
}

/// Composite Scorer that allows more fine-grained control of how the scores
/// are combined. The default is to apply a weighting
///
/// ### Example
///
/// Using the default measure ([`WeightedMeasure`]):
///
/// ```
/// # use bevy::prelude::*;
/// # use big_brain::prelude::*;
/// # #[derive(Debug, Clone, Component, ScorerBuilder)]
/// # struct MyScorer;
/// # #[derive(Debug, Clone, Component, ScorerBuilder)]
/// # struct MyOtherScorer;
/// # #[derive(Debug, Clone, Component, ActionBuilder)]
/// # struct MyAction;
/// # fn main() {
/// Thinker::build()
///     .when(
///         MeasuredScorer::build(0.5)
///           .push(MyScorer, 0.9)
///           .push(MyOtherScorer, 0.4),
///         MyAction)
/// # ;
/// # }
/// ```
///
/// Customising the measure:
///
/// ```
/// # use bevy::prelude::*;
/// # use big_brain::prelude::*;
/// # #[derive(Debug, Clone, Component, ScorerBuilder)]
/// # struct MyScorer;
/// # #[derive(Debug, Clone, Component, ScorerBuilder)]
/// # struct MyOtherScorer;
/// # #[derive(Debug, Clone, Component, ActionBuilder)]
/// # struct MyAction;
/// # fn main() {
/// Thinker::build()
///     .when(
///         MeasuredScorer::build(0.5)
///           .measure(ChebyshevDistance)
///           .push(MyScorer, 0.8)
///           .push(MyOtherScorer, 0.2),
///         MyAction)
/// # ;
/// # }
/// ```

#[derive(Component, Debug, Reflect)]
#[reflect(from_reflect = false)]
pub struct MeasuredScorer {
    threshold: f32,
    #[reflect(ignore)]
    measure: Arc<dyn Measure>,
    scorers: Vec<(Scorer, f32)>,
}

impl MeasuredScorer {
    pub fn build(threshold: f32) -> MeasuredScorerBuilder {
        MeasuredScorerBuilder {
            threshold,
            measure: Arc::new(WeightedMeasure),
            scorers: Vec::new(),
        }
    }
}

pub fn measured_scorers_system(
    query: Query<(Entity, &MeasuredScorer)>,
    mut scores: Query<&mut Score>,
) {
    for (this_entity, this) in query.iter() {
        let inputs = this
            .scorers
            .iter()
            .map(|&(Scorer(scorer), weight)| (scores.get(scorer).cloned().unwrap(), weight))
            .collect::<Vec<_>>();

        let measured_score = this.measure.calculate(&inputs);

        let mut score = scores.get_mut(this_entity).unwrap();

        if measured_score < this.threshold {
            score.set(0.0);
        } else {
            score.set(measured_score.clamp(0.0, 1.0));
        }
    }
}

#[derive(Reflect)]
#[reflect(from_reflect = false)]
pub struct MeasuredScorerBuilder {
    threshold: f32,
    #[reflect(ignore)]
    measure: Arc<dyn Measure>,
    #[reflect(ignore)]
    scorers: Vec<(Arc<dyn ScorerBuilder>, f32)>,
}

impl MeasuredScorerBuilder {
    /// Sets the measure to be used to combine the child scorers
    pub fn measure(mut self, measure: impl Measure + 'static) -> Self {
        self.measure = Arc::new(measure);
        self
    }

    pub fn push(mut self, scorer: impl ScorerBuilder + 'static, weight: f32) -> Self {
        self.scorers.push((Arc::new(scorer), weight));
        self
    }
}

impl ScorerBuilder for MeasuredScorerBuilder {
    #[allow(clippy::needless_collect)]
    fn spawn(&self, cmd: &mut Commands, actor: Actor) -> Scorer {
        let scorers: Vec<_> = self
            .scorers
            .iter()
            .map(|(scorer, _)| scorer.spawn(cmd, actor).0)
            .collect();

        Scorer(
            cmd.spawn((actor, Score::default()))
                .push_children(&scorers[..])
                .insert(MeasuredScorer {
                    threshold: self.threshold,
                    measure: self.measure.clone(),
                    scorers: scorers
                        .iter()
                        .cloned()
                        .map(Scorer)
                        .zip(self.scorers.iter().map(|&(_, weight)| weight))
                        .collect(),
                })
                .id(),
        )
    }
}
