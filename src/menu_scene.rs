//! 3D showroom behind the menus: your character on a turntable.

use bevy::prelude::*;

use crate::character::spawn_character;
use crate::profile::{Appearance, Profile};
use crate::skin_textures::SkinTextures;
use crate::states::AppState;
use crate::weapon_model::spawn_weapon_model;
use crate::weapons::WeaponId;

#[derive(Component)]
pub struct MenuSceneEntity;

#[derive(Component)]
pub struct MenuCamera;

#[derive(Component)]
pub struct Turntable;

#[derive(Component)]
struct ShowroomCharacter;

/// Last appearance/loadout the showroom was built with.
#[derive(Resource, Default)]
struct ShownLook(Option<(Appearance, Vec<crate::profile::EquippedSkin>)>);

/// Yaw offset the player can drag/rotate in the customize screen.
#[derive(Resource, Default)]
pub struct TurntableSpin(pub f32);

pub struct MenuScenePlugin;

impl Plugin for MenuScenePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ShownLook>()
            .init_resource::<TurntableSpin>()
            .add_systems(OnExit(AppState::InGame), spawn_scene)
            .add_systems(Startup, spawn_scene)
            .add_systems(OnEnter(AppState::InGame), despawn_scene)
            .add_systems(
                Update,
                (rebuild_character, spin_turntable, move_camera).run_if(|s: Res<State<AppState>>| s.get().is_menu()),
            );
    }
}

fn spawn_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut shown: ResMut<ShownLook>,
) {
    shown.0 = None;
    commands.spawn((
        MenuSceneEntity,
        MenuCamera,
        Camera3d::default(),
        Transform::from_xyz(1.6, 1.5, -3.8).looking_at(Vec3::new(1.0, 1.05, 0.0), Vec3::Y),
    ));
    commands.spawn((
        MenuSceneEntity,
        DirectionalLight {
            illuminance: 9000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(-3.0, 6.0, -4.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        MenuSceneEntity,
        PointLight {
            intensity: 400_000.0,
            color: Color::srgb(1.0, 0.7, 0.4),
            range: 12.0,
            ..default()
        },
        Transform::from_xyz(2.5, 2.5, 1.5),
    ));
    commands.spawn((
        MenuSceneEntity,
        PointLight {
            intensity: 300_000.0,
            color: Color::srgb(0.4, 0.6, 1.0),
            range: 12.0,
            ..default()
        },
        Transform::from_xyz(-3.0, 2.0, 1.0),
    ));
    // Pedestal and floor.
    commands.spawn((
        MenuSceneEntity,
        Mesh3d(meshes.add(Cylinder::new(0.8, 0.1))),
        MeshMaterial3d(mats.add(StandardMaterial {
            base_color: Color::srgb(0.12, 0.13, 0.16),
            metallic: 0.6,
            perceptual_roughness: 0.3,
            ..default()
        })),
        Transform::from_xyz(0.0, -0.05, 0.0),
    ));
    commands.spawn((
        MenuSceneEntity,
        Mesh3d(meshes.add(Plane3d::default().mesh().size(40.0, 40.0))),
        MeshMaterial3d(mats.add(StandardMaterial {
            base_color: Color::srgb(0.05, 0.055, 0.07),
            perceptual_roughness: 0.9,
            ..default()
        })),
        Transform::from_xyz(0.0, -0.1, 0.0),
    ));
    commands.spawn((
        MenuSceneEntity,
        Turntable,
        Transform::default(),
        Visibility::default(),
    ));
}

fn despawn_scene(mut commands: Commands, q: Query<Entity, With<MenuSceneEntity>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}

#[allow(clippy::too_many_arguments)]
fn rebuild_character(
    mut commands: Commands,
    profile: Res<Profile>,
    mut shown: ResMut<ShownLook>,
    turntable: Query<Entity, With<Turntable>>,
    old: Query<Entity, With<ShowroomCharacter>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut textures: ResMut<SkinTextures>,
) {
    let Ok(table) = turntable.single() else { return };
    let loadout = profile.loadout();
    let current = (profile.appearance.clone(), loadout.clone());
    if shown.0.as_ref() == Some(&current) {
        return;
    }
    shown.0 = Some(current);
    for e in &old {
        commands.entity(e).despawn();
    }
    let (ch, hand) = spawn_character(&mut commands, &mut meshes, &mut mats, &profile.appearance, None);
    commands.entity(ch).insert(ShowroomCharacter);
    commands.entity(table).add_child(ch);
    // Show off the best equipped gun (or an AK).
    let weapon = loadout
        .iter()
        .filter(|s| s.weapon != WeaponId::Knife)
        .max_by_key(|s| crate::cases::skin(s.skin_id).map(|d| d.rarity))
        .copied();
    let id = weapon.map(|w| w.weapon).unwrap_or(WeaponId::Ak47);
    let gun = spawn_weapon_model(&mut commands, &mut meshes, &mut mats, &mut images, &mut textures, id, weapon);
    commands.entity(hand).add_child(gun);
}

fn spin_turntable(
    time: Res<Time>,
    state: Res<State<AppState>>,
    mut spin: ResMut<TurntableSpin>,
    mut q: Query<&mut Transform, With<Turntable>>,
) {
    if *state.get() != AppState::Customize {
        spin.0 += time.delta_secs() * 0.4;
    }
    for mut t in &mut q {
        t.rotation = Quat::from_rotation_y(0.35 + spin.0.sin() * 0.6);
        if *state.get() == AppState::Customize {
            t.rotation = Quat::from_rotation_y(spin.0);
        }
    }
}

fn move_camera(
    time: Res<Time>,
    state: Res<State<AppState>>,
    mut q: Query<&mut Transform, With<MenuCamera>>,
) {
    // Character stands at the origin; the camera frames it on one side of the screen.
    let (eye, target) = match state.get() {
        AppState::Customize => (Vec3::new(1.2, 1.35, -3.1), Vec3::new(0.6, 1.0, 0.0)),
        AppState::Inventory => (Vec3::new(2.6, 1.6, -4.5), Vec3::new(2.6, 1.2, 0.0)),
        _ => (Vec3::new(1.6, 1.5, -3.8), Vec3::new(1.0, 1.05, 0.0)),
    };
    for mut t in &mut q {
        let k = (time.delta_secs() * 4.0).min(1.0);
        let goal = Transform::from_translation(eye).looking_at(target, Vec3::Y);
        t.translation = t.translation.lerp(goal.translation, k);
        t.rotation = t.rotation.slerp(goal.rotation, k);
    }
}
