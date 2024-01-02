use bevy::{
    app::AppExit,
    log::{Level, LogPlugin},
    prelude::*,
};
use big_brain::prelude::{
    Action, ActionCommands, ActionSpawn, ActionState, Actor, BigBrainPlugin, BigBrainSet,
    FirstToScore, Score, Scorer, ScorerCommands, ScorerSpawn, Sequence, Thinker,
};

#[test]
fn steps() {
    App::new()
        .add_plugins((
            MinimalPlugins,
            LogPlugin {
                level: Level::TRACE,
                ..default()
            },
            BigBrainPlugin::new(PreUpdate),
        ))
        .init_resource::<FailState>()
        .add_systems(Startup, setup)
        .add_systems(Update, no_failure_score.before(BigBrainSet::Scorers))
        .add_systems(
            PreUpdate,
            (mock_action, failure_action, exit_action).in_set(BigBrainSet::Actions),
        )
        .add_systems(Last, last.before(BigBrainSet::Cleanup))
        .run();

    error!("end");

    fn last() {
        trace!("-------------------");
    }
}

fn setup(mut cmds: Commands) {
    cmds.spawn(
        Thinker::build(FirstToScore::new(0.5))
            .when(NoFailureScorer, Sequence::step(FailureAction))
            .otherwise(Sequence::step((
                MockAction::new("A_action"),
                MockAction::new("B_action"),
                ExitAction,
            ))),
    );
}

#[derive(Component, Clone)]
struct MockAction {
    label: String,
}

impl MockAction {
    fn new(label: impl ToString) -> Self {
        Self {
            label: label.to_string(),
        }
    }
}

impl ActionSpawn for MockAction {
    fn spawn(&self, mut cmd: ActionCommands) -> Action {
        let action = cmd.spawn(self.clone());
        info!("spawned {} as {:?}", self.label, action);
        action
    }
}

fn mock_action(mut query: Query<(&Actor, &mut ActionState, &MockAction)>) {
    for (_actor, mut state, this) in query.iter_mut() {
        let prev_state = state.clone();

        match prev_state {
            ActionState::Executing => state.success(),
            ActionState::Cancelled => state.failure(),
            ActionState::Success | ActionState::Failure => (),
        }

        info!("{}: {:?} -> {:?}", this.label, prev_state, *state);
    }
}

#[derive(Component)]
struct ExitAction;

impl ActionSpawn for ExitAction {
    fn spawn(&self, mut cmd: ActionCommands) -> Action {
        cmd.spawn(Self)
    }
}

fn exit_action(
    mut query: Query<(&Actor, &mut ActionState), With<ExitAction>>,
    mut app_exit_events: EventWriter<AppExit>,
) {
    for (_actor, mut state) in query.iter_mut() {
        info!("exit_action {state:?}");
        match *state {
            ActionState::Executing => app_exit_events.send(AppExit),
            ActionState::Cancelled => state.failure(),
            ActionState::Success | ActionState::Failure => (),
        }
    }
}

#[derive(Component)]
struct FailureAction;

impl ActionSpawn for FailureAction {
    fn spawn(&self, mut cmd: ActionCommands) -> Action {
        cmd.spawn(Self)
    }
}

fn failure_action(
    mut query: Query<(&Actor, &mut ActionState), With<FailureAction>>,
    mut global_state: ResMut<FailState>,
) {
    for (_actor, mut state) in query.iter_mut() {
        global_state.failure |= state.is_executing();

        let prev_state = state.clone();
        match prev_state {
            ActionState::Executing => state.failure(),
            ActionState::Cancelled => panic!("wtf?"),
            ActionState::Success | ActionState::Failure => (),
        }
        info!("FailureAction: {:?} -> {:?}", prev_state, *state);
    }
}

#[derive(Default, Resource)]
struct FailState {
    failure: bool,
}

#[derive(Component)]
struct NoFailureScorer;

impl ScorerSpawn for NoFailureScorer {
    fn spawn(&self, mut cmd: ScorerCommands) -> Scorer {
        cmd.spawn(Self)
    }
}

fn no_failure_score(state: Res<FailState>, mut query: Query<&mut Score, With<NoFailureScorer>>) {
    for mut score in query.iter_mut() {
        score.set(if state.failure { 0.0 } else { 1.0 });
    }
}
