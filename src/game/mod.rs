//! In-match gameplay: world, local player, weapons, remote players, effects and HUD.

pub mod buy_menu;
pub mod combat;
pub mod effects;
pub mod hud;
pub mod pause;
pub mod player;
pub mod remote;

use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use crate::map::{map, BlockKind};
use crate::states::AppState;
use crate::ui::settings_menu::SettingsOpen;

/// Which in-game overlays are open (they free the mouse cursor).
#[derive(Resource, Default)]
pub struct GameUi {
    pub buy_open: bool,
    pub pause_open: bool,
}

impl GameUi {
    pub fn any_open(&self, settings: &SettingsOpen) -> bool {
        self.buy_open || self.pause_open || settings.0
    }
}

/// Everything spawned for a match is tagged so it can be cleaned up.
#[derive(Component)]
pub struct InGameEntity;

/// System ordering inside a frame.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum GameSet {
    Input,
    Simulate,
    Present,
}

pub fn in_game(state: Res<State<AppState>>) -> bool {
    *state.get() == AppState::InGame
}

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameUi>()
            .configure_sets(
                Update,
                (GameSet::Input, GameSet::Simulate, GameSet::Present)
                    .chain()
                    .run_if(in_game),
            )
            .add_systems(OnEnter(AppState::InGame), spawn_world)
            .add_systems(OnExit(AppState::InGame), (cleanup, release_cursor))
            .add_systems(Update, grab_cursor.in_set(GameSet::Present))
            .add_plugins((
                player::plugin,
                combat::plugin,
                remote::plugin,
                effects::plugin,
                hud::plugin,
                buy_menu::plugin,
                pause::plugin,
            ));
    }
}

fn block_material(kind: BlockKind) -> StandardMaterial {
    let (color, rough, metal) = match kind {
        BlockKind::Floor => (Color::srgb(0.55, 0.5, 0.42), 0.95, 0.0),
        BlockKind::Wall => (Color::srgb(0.72, 0.64, 0.5), 0.9, 0.0),
        BlockKind::Crate => (Color::srgb(0.55, 0.38, 0.2), 0.85, 0.0),
        BlockKind::Concrete => (Color::srgb(0.6, 0.6, 0.6), 0.9, 0.0),
        BlockKind::Metal => (Color::srgb(0.35, 0.4, 0.45), 0.4, 0.7),
    };
    StandardMaterial {
        base_color: color,
        perceptual_roughness: rough,
        metallic: metal,
        ..default()
    }
}

fn spawn_world(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut ui: ResMut<GameUi>,
    mut ambient: ResMut<GlobalAmbientLight>,
) {
    *ui = GameUi::default();
    ambient.brightness = 350.0;
    let kinds = [
        BlockKind::Floor,
        BlockKind::Wall,
        BlockKind::Crate,
        BlockKind::Concrete,
        BlockKind::Metal,
    ];
    let handles: Vec<_> = kinds.iter().map(|k| (*k, mats.add(block_material(*k)))).collect();
    for block in &map().blocks {
        let mat = handles.iter().find(|(k, _)| *k == block.kind).unwrap().1.clone();
        commands.spawn((
            InGameEntity,
            Mesh3d(meshes.add(Cuboid::from_size(block.aabb.size()))),
            MeshMaterial3d(mat),
            Transform::from_translation(block.aabb.center()),
        ));
    }
    commands.spawn((
        InGameEntity,
        DirectionalLight {
            illuminance: 12_000.0,
            shadow_maps_enabled: true,
            color: Color::srgb(1.0, 0.95, 0.85),
            ..default()
        },
        Transform::from_xyz(10.0, 30.0, 12.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn cleanup(mut commands: Commands, q: Query<Entity, With<InGameEntity>>, mut ui: ResMut<GameUi>) {
    for e in &q {
        commands.entity(e).despawn();
    }
    *ui = GameUi::default();
}

fn grab_cursor(
    ui: Res<GameUi>,
    settings: Res<SettingsOpen>,
    mut cursor: Single<&mut CursorOptions, With<PrimaryWindow>>,
    window: Single<&Window, With<PrimaryWindow>>,
) {
    let free = ui.any_open(&settings) || !window.focused;
    let (grab, visible) = if free {
        (CursorGrabMode::None, true)
    } else {
        (CursorGrabMode::Locked, false)
    };
    if cursor.grab_mode != grab {
        cursor.grab_mode = grab;
    }
    if cursor.visible != visible {
        cursor.visible = visible;
    }
}

fn release_cursor(mut cursor: Single<&mut CursorOptions, With<PrimaryWindow>>) {
    cursor.grab_mode = CursorGrabMode::None;
    cursor.visible = true;
}
