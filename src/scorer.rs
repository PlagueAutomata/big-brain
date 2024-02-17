//! Scorers look at the world and boil down arbitrary characteristics into a
//! range of 0.0..=1.0. This module includes the ScorerBuilder trait and some
//! built-in Composite Scorers.

use crate::thinker::Actor;
use bevy_ecs::{
    bundle::Bundle,
    component::Component,
    entity::Entity,
    query::{QueryData, With},
    system::{Commands, Query},
};
use bevy_hierarchy::{Children, PushChild};
use bevy_reflect::Reflect;
use bevy_utils::all_tuples;
use std::sync::Arc;

pub struct ScorerCommands<'w, 's, 'a> {
    cmd: &'a mut Commands<'w, 's>,
    actor: Actor,
}

impl<'w, 's, 'a> ScorerCommands<'w, 's, 'a> {
    #[inline]
    pub(crate) fn new(cmd: &'a mut Commands<'w, 's>, actor: Actor) -> Self {
        Self { cmd, actor }
    }

    #[inline]
    pub fn spawn(&mut self, bundle: impl Bundle) -> Scorer {
        let bundle = (self.actor, Score::default(), bundle);
        Scorer(self.cmd.spawn(bundle).id())
    }

    #[inline]
    pub fn push_child(&mut self, Scorer(parent): Scorer, builder: &dyn ScorerSpawn) {
        let Scorer(child) = builder.spawn(ScorerCommands::new(self.cmd, self.actor));
        self.cmd.add(PushChild { parent, child })
    }
}

pub trait ScorersList {
    fn build(scorers: Self) -> Vec<Arc<dyn ScorerSpawn>>;
}

impl<T: ScorerSpawn + 'static> ScorersList for T {
    fn build(scorer: Self) -> Vec<Arc<dyn ScorerSpawn>> {
        vec![Arc::new(scorer)]
    }
}

macro_rules! impl_scorers_list {
    ($(($Type:ident, $index:ident)),*) => {
        impl<$($Type),*> ScorersList for ($($Type,)*) where $($Type: ScorerSpawn + 'static),* {
            fn build(($($index,)*): Self) -> Vec<Arc<dyn ScorerSpawn>> {
                vec![ $(Arc::new($index),)* ]
            }
        }
    }
}

all_tuples!(impl_scorers_list, 1, 15, Type, index);

pub struct ScorerSpawner<Bundle: Component + Clone> {
    bundle: Bundle,
    scorers: Vec<Arc<dyn ScorerSpawn>>,
}

impl<Bundle: Component + Clone> ScorerSpawner<Bundle> {
    pub fn new<B: ScorersList>(bundle: Bundle, scorers: B) -> Self {
        let scorers = B::build(scorers);
        Self { bundle, scorers }
    }
}

impl<Bundle: Component + Clone> ScorerSpawn for ScorerSpawner<Bundle> {
    fn spawn(&self, mut cmd: ScorerCommands) -> Scorer {
        let scorer = cmd.spawn(self.bundle.clone());
        for child in &self.scorers {
            cmd.push_child(scorer, child.as_ref());
        }
        scorer
    }
}

#[derive(Debug, Clone, Copy, Reflect)]
pub struct Scorer(pub Entity);

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
pub trait ScorerSpawn: Sync + Send {
    fn spawn(&self, cmd: ScorerCommands) -> Scorer;
}

#[derive(QueryData)]
#[query_data(mutable)]
pub struct ScorerQuery {
    score: &'static mut Score,
    actor: &'static Actor,
}

impl<'w> ScorerQueryItem<'w> {
    pub fn actor(&self) -> Entity {
        self.actor.entity()
    }

    pub fn get(&self) -> f32 {
        self.score.get()
    }

    pub fn set(&mut self, score: f32) {
        self.score.set(score);
    }
}

impl<'w> ScorerQueryReadOnlyItem<'w> {
    pub fn actor(&self) -> Entity {
        self.actor.entity()
    }

    pub fn get(&self) -> f32 {
        self.score.get()
    }
}

/// Scorer that always returns minimal valid score (f32::MIN_POSITIVE).
#[derive(Clone, Component)]
pub struct IdleScorer;

impl ScorerSpawn for IdleScorer {
    fn spawn(&self, mut cmd: ScorerCommands) -> Scorer {
        cmd.spawn(self.clone())
    }
}

pub fn idle_scorer_system(mut query: Query<&mut Score, With<IdleScorer>>) {
    for mut score in query.iter_mut() {
        score.set(f32::MIN_POSITIVE);
    }
}

/// Scorer that always returns the same, fixed score.
/// Good for combining with things creatively!
#[derive(Clone, Component)]
pub struct FixedScorer(pub f32);

impl FixedScorer {
    pub const IDLE: Self = FixedScorer(f32::MIN_POSITIVE);
}

impl ScorerSpawn for FixedScorer {
    fn spawn(&self, mut cmd: ScorerCommands) -> Scorer {
        cmd.spawn(self.clone())
    }
}

pub fn fixed_scorer_system(mut query: Query<(&mut Score, &FixedScorer)>) {
    for (mut score, &FixedScorer(fixed)) in query.iter_mut() {
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
/// # use big_brain::*;
/// # #[derive(Debug, Clone, Component, ScorerSpawn)]
/// # struct MyScorer;
/// # #[derive(Debug, Clone, Component, ScorerSpawn)]
/// # struct MyOtherScorer;
/// # #[derive(Debug, Clone, Component, ActionSpawn)]
/// # struct MyAction;
/// # fn main() {
/// ThinkerSpawner::highest(0.0)
///     .when(AllOrNothing::build(0.8, (MyScorer, MyOtherScorer)), MyAction);
/// # ;
/// # }
/// ```
#[derive(Component, Clone)]
pub struct AllOrNothing {
    threshold: f32,
}

impl AllOrNothing {
    pub fn build<B: ScorersList>(threshold: f32, scorers: B) -> impl ScorerSpawn {
        ScorerSpawner::new(Self { threshold }, scorers)
    }
}

pub fn all_or_nothing_system(
    query: Query<(Entity, &AllOrNothing, &Children)>,
    mut scores: Query<&mut Score>,
) {
    for (aon_ent, AllOrNothing { threshold }, scorers) in query.iter() {
        let mut sum = 0.0;
        for &child in scorers.iter() {
            let score = scores.get(child).unwrap();
            if score.0 < *threshold {
                sum = 0.0;
                break;
            } else {
                sum += score.0;
            }
        }
        scores.get_mut(aon_ent).unwrap().set(sum.clamp(0.0, 1.0));
    }
}

/// Composite Scorer that takes any number of other Scorers and returns the sum of their [`Score`] values if the _total_ summed [`Score`] is at or above the configured `threshold`.
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
///     .when(SumOfScorers::build(0.8, (MyScorer, MyOtherScorer)), MyAction)
/// # ;
/// # }
/// ```
#[derive(Component, Clone)]
pub struct SumOfScorers {
    threshold: f32,
}

impl SumOfScorers {
    pub fn build<B: ScorersList>(threshold: f32, scorers: B) -> impl ScorerSpawn {
        ScorerSpawner::new(Self { threshold }, scorers)
    }
}

pub fn sum_of_scorers_system(
    query: Query<(Entity, &SumOfScorers, &Children)>,
    mut scores: Query<&mut Score>,
) {
    for (sos_ent, SumOfScorers { threshold }, scorers) in query.iter() {
        let mut sum = 0.0;
        for &child in scorers.iter() {
            let score = scores.get(child).unwrap();
            sum += score.0;
        }
        if sum < *threshold {
            sum = 0.0;
        }
        scores.get_mut(sos_ent).unwrap().set(sum.clamp(0.0, 1.0));
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
/// # use big_brain::*;
/// # #[derive(Debug, Clone, Component, ScorerSpawn)]
/// # struct MyScorer;
/// # #[derive(Debug, Clone, Component, ScorerSpawn)]
/// # struct MyOtherScorer;
/// # #[derive(Debug, Clone, Component, ActionSpawn)]
/// # struct MyAction;
/// # fn main() {
/// ThinkerSpawner::highest(0.0)
///     .when(ProductOfScorers::build(0.5, (MyScorer, MyOtherScorer)), MyAction)
/// # ;
/// # }
/// ```
#[derive(Component, Clone)]
pub struct ProductOfScorers {
    threshold: f32,
}

impl ProductOfScorers {
    pub fn build<B: ScorersList>(threshold: f32, scorers: B) -> impl ScorerSpawn {
        ScorerSpawner::new(Self { threshold }, scorers)
    }
}

pub fn product_of_scorers_system(
    query: Query<(Entity, &ProductOfScorers, &Children)>,
    mut scores: Query<&mut Score>,
) {
    for (this_entity, this, scorers) in query.iter() {
        let mut product = 1.0;

        for &child in scorers.iter() {
            product *= scores.get_mut(child).unwrap().0;
        }

        if product < this.threshold {
            product = 0.0;
        }

        let score = product.clamp(0.0, 1.0);
        scores.get_mut(this_entity).unwrap().set(score);
    }
}

#[derive(Component, Clone)]
pub struct CompensatedProductOfScorers {
    threshold: f32,
}

impl CompensatedProductOfScorers {
    pub fn build<B: ScorersList>(threshold: f32, scorers: B) -> impl ScorerSpawn {
        ScorerSpawner::new(Self { threshold }, scorers)
    }
}

pub fn compensated_product_of_scorers_system(
    query: Query<(Entity, &CompensatedProductOfScorers, &Children)>,
    mut scores: Query<&mut Score>,
) {
    for (this_entity, this, scorers) in query.iter() {
        let mut product = 1.0;

        for &child in scorers.iter() {
            product *= scores.get_mut(child).unwrap().0;
        }

        // See for example
        // http://www.gdcvault.com/play/1021848/Building-a-Better-Centaur-AI
        if product < 1.0 {
            let mod_factor = 1.0 - 1.0 / (scorers.len() as f32);
            let makeup = (1.0 - product) * mod_factor;
            product += makeup * product;
        }

        if product < this.threshold {
            product = 0.0;
        }

        let score = product.clamp(0.0, 1.0);
        scores.get_mut(this_entity).unwrap().set(score);
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
/// # use big_brain::*;
/// # #[derive(Debug, Clone, Component, ScorerSpawn)]
/// # struct MyScorer;
/// # #[derive(Debug, Clone, Component, ScorerSpawn)]
/// # struct MyOtherScorer;
/// # #[derive(Debug, Clone, Component, ActionSpawn)]
/// # struct MyAction;
/// # fn main() {
/// ThinkerSpawner::highest(0.0)
///     .when(WinningScorer::build(0.8, (MyScorer, MyOtherScorer)), MyAction)
/// # ;
/// # }
/// ```
#[derive(Component, Clone)]
pub struct WinningScorer {
    threshold: f32,
}

impl WinningScorer {
    pub fn build<B: ScorersList>(threshold: f32, scorers: B) -> impl ScorerSpawn {
        ScorerSpawner::new(Self { threshold }, scorers)
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

        scores
            .get_mut(this_entity)
            .unwrap()
            .set(winning_score_or_zero.clamp(0.0, 1.0));
    }
}
