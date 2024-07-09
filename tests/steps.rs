use bevy::{
    app::AppExit,
    log::{Level, LogPlugin},
    prelude::*,
};
use big_brain::{
    Action, ActionCommands, ActionSpawn, ActionState, Actor, BigBrainPlugin, BigBrainSet,
    FixedScorer, Score, Scorer, ScorerCommands, ScorerSpawn, Sequence, ThinkerSpawner,
};

#[test]
fn steps() {
    App::new()
        .add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            LogPlugin {
                level: Level::TRACE,
                ..default()
            },
            BigBrainPlugin::new(Update, Update, PostUpdate, Last),
        ))
        .init_resource::<FailState>()
        .add_systems(Startup, setup)
        .add_systems(First, || trace!("-------------------"))
        .add_systems(
            Update,
            (
                no_failure_score.in_set(BigBrainSet::Scorers),
                (mock_action, failure_action, exit_action).in_set(BigBrainSet::Actions),
            ),
        )
        .run();

    error!("end");
}

fn setup(mut cmds: Commands, mut builders: ResMut<Assets<ThinkerSpawner>>) {
    let handle = builders.add(
        ThinkerSpawner::first_to_score(0.0)
            .when(NoFailureScorer, Sequence::step(FailureAction))
            .when(
                FixedScorer::IDLE,
                Sequence::step((
                    MockAction::new("A_action"),
                    MockAction::new("B_action"),
                    MockAction::new("C_action"),
                    MockAction::new("D_action"),
                    ExitAction,
                )),
            ),
    );
    cmds.spawn(handle);
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
        info!("{} spawned as {:?}", self.label, action);
        action
    }
}

fn mock_action(mut query: Query<(Entity, &Actor, &mut ActionState, &MockAction)>) {
    for (entity, _actor, mut state, this) in query.iter_mut() {
        let prev_state = state.clone();

        match prev_state {
            ActionState::Executing => state.success(),
            ActionState::Cancelled => state.failure(),
            ActionState::Success | ActionState::Failure => (),
        }

        info!(
            "{} {:?}: {:?} -> {:?}",
            this.label, entity, prev_state, *state
        );
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
            ActionState::Executing => {
                app_exit_events.send(AppExit::Success);
            }
            ActionState::Cancelled => state.failure(),
            ActionState::Success | ActionState::Failure => (),
        }
    }
}

#[derive(Component, Clone)]
struct FailureAction;

impl ActionSpawn for FailureAction {
    fn spawn(&self, mut cmd: ActionCommands) -> Action {
        let action = cmd.spawn(self.clone());
        info!("FailureAction spawned as {:?}", action);
        action
    }
}

fn failure_action(
    mut query: Query<(Entity, &Actor, &mut ActionState), With<FailureAction>>,
    mut global_state: ResMut<FailState>,
) {
    for (entity, _actor, mut state) in query.iter_mut() {
        global_state.failure |= state.is_executing();

        let prev_state = state.clone();
        match prev_state {
            ActionState::Executing => state.failure(),
            ActionState::Cancelled => panic!("wtf?"),
            ActionState::Success | ActionState::Failure => (),
        }
        info!(
            "FailureAction {:?}: {:?} -> {:?}",
            entity, prev_state, *state
        );
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
        let result = if state.failure { 0.0 } else { 1.0 };
        info!("NoFailureScorer: {:?}", result);
        score.set(result);
    }
}
