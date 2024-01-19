use bevy::prelude::*;
use big_brain::*;

#[derive(Debug, Clone, Component, ScorerSpawn)]
pub struct MyScorer;

#[test]
fn check_macro() {
    let _scorer: &dyn ScorerSpawn = &MyScorer;
}
