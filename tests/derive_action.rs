use bevy::prelude::*;
use big_brain::*;

#[derive(Debug, Clone, Component, ActionSpawn)]
pub struct MyAction;

#[test]
fn check_macro() {
    let _action: &dyn ActionSpawn = &MyAction;
}
