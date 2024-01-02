//! Simple example of a utility ai farming agent

use bevy::{log::LogPlugin, prelude::*};
use bevy_scene_hook::{HookPlugin, HookedSceneBundle, SceneHook};
use big_brain::prelude::*;

const DEFAULT_COLOR: Color = Color::BLACK;
const SLEEP_COLOR: Color = Color::RED;
const FARM_COLOR: Color = Color::BLUE;
const MAX_DISTANCE: f32 = 0.1;
const MAX_INVENTORY_ITEMS: f32 = 20.0;

#[derive(Component, Clone)]
pub struct Field;

#[derive(Component, Clone)]
pub struct Market;

#[derive(Component, Clone)]
pub struct House;

#[derive(Component, Reflect)]
pub struct Inventory {
    pub money: u32,
    pub items: f32,
}

#[derive(Component)]
pub struct MoneyText;

#[derive(Component)]
pub struct FatigueText;

#[derive(Component)]
pub struct InventoryText;

// ================================================================================
//  Sleepiness 😴
// ================================================================================
#[derive(Component, Reflect)]
pub struct Fatigue {
    pub is_sleeping: bool,
    pub per_second: f32,
    pub level: f32,
}

pub fn fatigue_system(time: Res<Time>, mut fatigues: Query<&mut Fatigue>) {
    for mut fatigue in &mut fatigues {
        fatigue.level += fatigue.per_second * time.delta_seconds();
        if fatigue.level >= 100.0 {
            fatigue.level = 100.0;
        }
        trace!("Tiredness: {}", fatigue.level);
    }
}

#[derive(Component, Clone, ActionSpawn)]
pub struct Sleep {
    until: f32,
    per_second: f32,
}

fn sleep_action_system(
    time: Res<Time>,
    mut actors: Query<(&mut Fatigue, &Handle<StandardMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut query: Query<(ActionQuery, &Sleep)>,
) {
    for (mut action, sleep) in &mut query {
        let actor = actors.get_mut(action.actor()).ok();
        let Some((mut fatigue, material)) =
            actor.and_then(|(fatigue, id)| Some((fatigue, materials.get_mut(id)?)))
        else {
            action.failure();
            continue;
        };

        if action.is_executing() {
            if !fatigue.is_sleeping {
                debug!("Time to sleep!");
                fatigue.is_sleeping = true;
            }

            trace!("Sleeping...");

            fatigue.level -= sleep.per_second * time.delta_seconds();
            material.base_color = SLEEP_COLOR;

            if fatigue.level <= sleep.until {
                debug!("Woke up well-rested!");
                material.base_color = DEFAULT_COLOR;
                fatigue.is_sleeping = false;
                action.success();
            }
        }

        if action.is_cancelled() {
            debug!("Sleep was interrupted. Still tired.");
            material.base_color = DEFAULT_COLOR;
            fatigue.is_sleeping = false;
            action.failure();
        }
    }
}

#[derive(Component, Clone, ScorerSpawn)]
pub struct FatigueScorer;

pub fn fatigue_scorer_system(
    mut last_score: Local<Option<f32>>,
    fatigues: Query<&Fatigue>,
    mut query: Query<ScorerQuery, With<FatigueScorer>>,
) {
    for mut score in &mut query {
        if let Ok(fatigue) = fatigues.get(score.actor()) {
            let new_score = fatigue.level / 100.0;

            if fatigue.is_sleeping {
                score.set(*last_score.get_or_insert(new_score));
            } else {
                last_score.take();
                score.set(new_score);
                if fatigue.level >= 80.0 {
                    debug!("Fatigue above threshold! Score: {}", fatigue.level / 100.0)
                }
            }
        }
    }
}

// ================================================================================
//  Farming 🚜
// ================================================================================

#[derive(Component, Clone, ActionSpawn)]
pub struct Farm {
    pub until: f32,
    pub per_second: f32,
}

fn farm_action_system(
    time: Res<Time>,
    mut actors: Query<(&mut Inventory, &Handle<StandardMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut query: Query<(ActionQuery, &Farm)>,
) {
    for (mut action, farm) in &mut query {
        let actor = actors.get_mut(action.actor()).ok();
        let Some((mut inventory, material)) =
            actor.and_then(|(inv, id)| Some((inv, materials.get_mut(id)?)))
        else {
            action.failure();
            continue;
        };

        if action.is_executing() {
            //debug!("Time to farm!");

            trace!("Farming...");
            inventory.items += farm.per_second * time.delta_seconds();
            material.base_color = FARM_COLOR;

            if inventory.items >= MAX_INVENTORY_ITEMS {
                debug!("Inventory full!");
                material.base_color = DEFAULT_COLOR;
                action.success();
            }
        }

        if action.is_cancelled() {
            debug!("Farming was interrupted. Still need to work.");
            material.base_color = DEFAULT_COLOR;
            action.failure();
        }
    }
}

#[derive(Component, Clone, ScorerSpawn)]
pub struct WorkNeedScorer;

pub fn work_need_scorer_system(
    actors: Query<&Inventory>,
    mut query: Query<ScorerQuery, With<WorkNeedScorer>>,
) {
    for mut score in &mut query {
        let inventory = actors.get(score.actor());
        let need_more = inventory.map_or(false, |inv| inv.items < MAX_INVENTORY_ITEMS);
        score.set(if need_more { 0.6 } else { 0.0 });
    }
}

// ================================================================================
//  Selling 💰
// ================================================================================

#[derive(Component, Clone, ActionSpawn)]
pub struct Sell;

fn sell_action_system(
    mut actors: Query<&mut Inventory>,
    mut query: Query<ActionQuery, With<Sell>>,
) {
    for mut action in &mut query {
        let Ok(mut inventory) = actors.get_mut(action.actor()) else {
            action.failure();
            continue;
        };

        if action.is_executing() {
            debug!("Time to sell!");
            trace!("Selling...");

            inventory.money += inventory.items as u32;
            inventory.items = 0.0;

            debug!("Sold! Money: {}", inventory.money);

            action.success();
        }

        if action.is_cancelled() {
            debug!("Selling was interrupted. Still need to work.");
            action.failure();
        }
    }
}

#[derive(Component, Clone, ScorerSpawn)]
pub struct SellNeedScorer;

pub fn sell_need_scorer_system(
    actors: Query<&Inventory>,
    mut query: Query<ScorerQuery, With<SellNeedScorer>>,
) {
    for mut score in &mut query {
        let inventory = actors.get(score.actor());
        let has_enough = inventory.map_or(false, |inv| inv.items >= MAX_INVENTORY_ITEMS);
        score.set(if has_enough { 0.6 } else { 0.0 });
    }
}

// ================================================================================
//  Movement 🚶
// ================================================================================

#[derive(Component, Clone, ActionSpawn)]
pub struct MoveToNearest<T: Component + Clone> {
    speed: f32,
    target: Option<Entity>,
    marker: std::marker::PhantomData<T>,
}

impl<T: Component + Clone> MoveToNearest<T> {
    pub fn new(speed: f32) -> Self {
        Self {
            speed,
            target: None,
            marker: std::marker::PhantomData,
        }
    }

    pub fn system(
        time: Res<Time>,
        query: Query<(Entity, &Transform), With<T>>,
        mut thinkers: Query<&mut Transform, (With<HasThinker>, Without<T>)>,
        mut action_query: Query<(ActionQuery, &mut MoveToNearest<T>)>,
    ) {
        for (mut action, mut move_to) in &mut action_query {
            if action.is_cancelled() {
                action.failure();
            }
            if !action.is_executing() {
                continue;
            }

            let mut actor_transform = thinkers.get_mut(action.actor()).unwrap();

            let Some((goal_entity, goal_tranform)) = (if let Some(entity) = move_to.target {
                query.get(entity).ok()
            } else {
                debug!("Let's go find a {:?}", std::any::type_name::<T>());
                query.iter().min_by(|(_, a), (_, b)| {
                    let a = a.translation - actor_transform.translation;
                    let b = b.translation - actor_transform.translation;
                    f32::total_cmp(&a.length_squared(), &b.length_squared())
                })
            }) else {
                action.failure();
                continue;
            };

            move_to.target = Some(goal_entity);

            let delta = goal_tranform.translation - actor_transform.translation;
            let distance = delta.xz().length();

            trace!("Distance: {}", distance);

            if distance > MAX_DISTANCE {
                trace!("Stepping closer.");

                let step = move_to.speed * time.delta_seconds();
                let step = delta.normalize() * step.min(distance);

                actor_transform.translation.x += step.x;
                actor_transform.translation.z += step.z;
            } else {
                debug!("We got there!");
                action.success()
            }
        }
    }
}

// ================================================================================
//  UI
// ================================================================================

type TextQuery<'a> = (
    &'a mut Text,
    Has<MoneyText>,
    Has<FatigueText>,
    Has<InventoryText>,
);

fn update_ui(actor_query: Query<(&Inventory, &Fatigue)>, mut text_query: Query<TextQuery>) {
    for (inventory, fatigue) in &mut actor_query.iter() {
        for (mut text, has_money, has_fatigue, has_inventory) in &mut text_query {
            text.sections[0].value = match () {
                _ if has_money => format!("Money: {}", inventory.money),
                _ if has_fatigue => format!("Fatigue: {}", fatigue.level as u32),
                _ if has_inventory => format!("Inventory: {}", inventory.items as u32),
                _ => continue,
            }
        }
    }
}

fn init_entities(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((Camera3dBundle {
        transform: Transform::from_xyz(6.0, 6.0, 4.0)
            .looking_at(Vec3::new(0.0, -1.0, 0.0), Vec3::Y),
        ..default()
    },));

    commands.insert_resource(AmbientLight {
        color: Color::WHITE,
        brightness: 1.0,
    });

    commands.spawn((
        Name::new("Light"),
        SpotLightBundle {
            spot_light: SpotLight {
                shadows_enabled: true,
                intensity: 5_000.0,
                range: 100.0,
                ..default()
            },
            transform: Transform::from_xyz(2.0, 10.0, 0.0).looking_at(Vec3::ZERO, Vec3::Y),
            ..default()
        },
    ));

    commands.spawn((
        Name::new("Town"),
        HookedSceneBundle {
            scene: SceneBundle {
                scene: asset_server.load("town.glb#Scene0"),
                ..default()
            },
            hook: SceneHook::new(|entity, cmd| {
                match entity.get::<Name>().map(|t| t.as_str()) {
                    Some("Farm_Marker") => cmd.insert(Field),
                    Some("Market_Marker") => cmd.insert(Market),
                    Some("House_Marker") => cmd.insert(House),
                    _ => cmd,
                };
            }),
        },
    ));

    let sleep = Sleep {
        until: 10.0,
        per_second: 10.0,
    };

    let farm = Farm {
        until: 10.0,
        per_second: 10.0,
    };

    let sell = Sell;

    let move_and_sleep = Sequence::step((MoveToNearest::<House>::new(1.0), sleep));
    let move_and_farm = Sequence::step((MoveToNearest::<Field>::new(1.0), farm));
    let move_and_sell = Sequence::step((MoveToNearest::<Market>::new(1.0), sell));

    commands.spawn((
        Name::new("Farmer"),
        PbrBundle {
            mesh: meshes.add(Mesh::from(shape::Capsule {
                depth: 0.3,
                radius: 0.1,
                ..default()
            })),
            material: materials.add(DEFAULT_COLOR.into()),
            transform: Transform::from_xyz(0.0, 0.5, 0.0),
            ..default()
        },
        Fatigue {
            is_sleeping: false,
            per_second: 2.0,
            level: 0.0,
        },
        Inventory {
            money: 0,
            items: 0.0,
        },
        Thinker::first_to_score(0.6)
            .when(FatigueScorer, move_and_sleep)
            .when(WorkNeedScorer, move_and_farm)
            .when(SellNeedScorer, move_and_sell),
    ));

    // scoreboard
    commands
        .spawn(NodeBundle {
            style: Style {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::End,
                align_items: AlignItems::FlexStart,
                padding: UiRect::all(Val::Px(20.0)),
                ..default()
            },
            ..default()
        })
        .with_children(|builder| {
            let text_bundle = || {
                let style = TextStyle {
                    font: default(),
                    font_size: 40.0,
                    color: Color::WHITE,
                };
                TextBundle::from_section("", style)
            };

            builder.spawn((text_bundle(), MoneyText));
            builder.spawn((text_bundle(), FatigueText));
            builder.spawn((text_bundle(), InventoryText));
        });
}

fn main() {
    App::new()
        // .add_plugins(DefaultPlugins)
        .add_plugins(DefaultPlugins.set(LogPlugin {
            level: bevy::log::Level::WARN,
            // Use `RUST_LOG=big_brain=trace,farming_sim=trace cargo run --example
            // farming_sim --features=trace` to see extra tracing output.
            // filter: "big_brain=debug,farming_sim=trace".to_string(),
            ..default()
        }))
        .register_type::<Fatigue>()
        .register_type::<Inventory>()
        .add_plugins(HookPlugin)
        .add_plugins(BigBrainPlugin::new(PreUpdate))
        .add_systems(Startup, init_entities)
        .add_systems(Update, (fatigue_system, update_ui))
        .add_systems(
            PreUpdate,
            (
                (
                    sleep_action_system,
                    farm_action_system,
                    sell_action_system,
                    MoveToNearest::<House>::system,
                    MoveToNearest::<Field>::system,
                    MoveToNearest::<Market>::system,
                )
                    .in_set(BigBrainSet::Actions),
                (
                    fatigue_scorer_system,
                    work_need_scorer_system,
                    sell_need_scorer_system,
                )
                    .in_set(BigBrainSet::Scorers),
            ),
        )
        .run();
}
