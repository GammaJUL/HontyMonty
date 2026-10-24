//! Other players: spawn their characters, interpolate movement, show held
//! weapons and teammate name tags.

use bevy::prelude::*;

use super::player::PlayerCamera;
use super::{GameSet, InGameEntity};
use crate::audio::{PlaySfx, Sound};
use crate::character::spawn_character;
use crate::game_mode::Team;
use crate::net::protocol::PlayerId;
use crate::net::NetClient;
use crate::profile::{Appearance, EquippedSkin};
use crate::skin_textures::SkinTextures;
use crate::ui::widgets::{font, rgb};
use crate::weapon_model::{spawn_grenade_model, spawn_weapon_model};
use crate::weapons::{GrenadeKind, WeaponId};

#[derive(Component)]
pub struct RemotePlayer {
    pub id: PlayerId,
    target_pos: Vec3,
    target_yaw: f32,
    alive: bool,
    crouch: bool,
    look: Option<(Appearance, Team)>,
    model: Option<Entity>,
    hand: Option<Entity>,
    held: Option<(Option<WeaponId>, Option<GrenadeKind>)>,
    held_entity: Option<Entity>,
    death_anim: f32,
    moving: bool,
    step_timer: f32,
}

impl RemotePlayer {
    /// Solid box for local movement; dead players can be walked through.
    pub fn collider(&self, pos: Vec3) -> Option<crate::map::Aabb> {
        let height = if self.crouch { crate::map::CROUCH_HEIGHT } else { crate::map::STAND_HEIGHT };
        self.alive.then(|| crate::map::player_aabb(pos, height))
    }
}

#[derive(Component)]
struct NameTag(PlayerId);

pub fn plugin(app: &mut App) {
    app.add_systems(Update, (sync_remote, sync_dropped).in_set(GameSet::Simulate))
        .add_systems(Update, (interpolate, name_tags, footsteps).in_set(GameSet::Present));
}

#[allow(clippy::too_many_arguments)]
fn sync_remote(
    mut commands: Commands,
    client: Option<Res<NetClient>>,
    mut q: Query<(Entity, &mut RemotePlayer, &mut Transform)>,
    tags: Query<(Entity, &NameTag)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut textures: ResMut<SkinTextures>,
) {
    let Some(c) = client else { return };
    let Some(snap) = &c.snapshot else { return };
    let me = c.my_id;
    let my_team = c.me().map(|p| p.team).unwrap_or(Team::None);
    let ffa = snap.lobby.mode.is_ffa();

    // Remove players that left.
    for (e, rp, _) in &q {
        if !snap.players.iter().any(|p| p.id == rp.id) {
            commands.entity(e).despawn();
        }
    }
    for (e, tag) in &tags {
        if !snap.players.iter().any(|p| p.id == tag.0 && Some(p.id) != me) {
            commands.entity(e).despawn();
        }
    }

    for np in &snap.players {
        if Some(np.id) == me {
            continue;
        }
        let existing = q.iter_mut().find(|(_, rp, _)| rp.id == np.id);
        let (entity, mut rp, mut t) = match existing {
            Some(v) => v,
            None => {
                commands
                    .spawn((
                        InGameEntity,
                        RemotePlayer {
                            id: np.id,
                            target_pos: Vec3::from(np.pos),
                            target_yaw: np.yaw,
                            alive: np.alive,
                            crouch: np.crouch,
                            look: None,
                            model: None,
                            hand: None,
                            held: None,
                            held_entity: None,
                            death_anim: 0.0,
                            moving: false,
                            step_timer: 0.0,
                        },
                        Transform::from_translation(Vec3::from(np.pos)),
                        Visibility::default(),
                    ));
                if !ffa && np.team == my_team {
                    commands.spawn((
                        InGameEntity,
                        NameTag(np.id),
                        Text::new(np.name.clone()),
                        font(14.0),
                        TextColor(rgb(np.team.color())),
                        Node {
                            position_type: PositionType::Absolute,
                            ..default()
                        },
                    ));
                }
                // Created next frame once the entity exists in the query.
                continue;
            }
        };
        let respawned = np.alive && !rp.alive;
        rp.target_pos = Vec3::from(np.pos);
        rp.target_yaw = np.yaw;
        rp.alive = np.alive;
        rp.crouch = np.crouch;
        rp.moving = np.moving && np.alive;
        if respawned {
            t.translation = rp.target_pos;
            rp.death_anim = 0.0;
        }

        let appearance = c
            .profiles
            .get(&np.id)
            .map(|p| p.appearance.clone())
            .unwrap_or_default();
        let look = (appearance.clone(), np.team);
        if rp.look.as_ref() != Some(&look) {
            if let Some(old) = rp.model.take() {
                commands.entity(old).despawn();
            }
            let team = if ffa { None } else { Some(np.team) };
            let (model, hand) = spawn_character(&mut commands, &mut meshes, &mut mats, &appearance, team);
            commands.entity(entity).add_child(model);
            rp.model = Some(model);
            rp.hand = Some(hand);
            rp.held = None;
            rp.held_entity = None;
            rp.look = Some(look);
        }

        let held = (np.weapon, np.grenade);
        if rp.held != Some(held)
            && let Some(hand) = rp.hand
        {
            if let Some(old) = rp.held_entity.take() {
                commands.entity(old).despawn();
            }
            let skins: Vec<EquippedSkin> = c.profiles.get(&np.id).map(|p| p.skins.clone()).unwrap_or_default();
            let model = match held {
                (Some(w), _) => {
                    let skin = skins.iter().find(|s| s.weapon == w).copied();
                    Some(spawn_weapon_model(&mut commands, &mut meshes, &mut mats, &mut images, &mut textures, w, skin))
                }
                (None, Some(g)) => Some(spawn_grenade_model(&mut commands, &mut meshes, &mut mats, g)),
                _ => None,
            };
            if let Some(m) = model {
                commands.entity(hand).add_child(m);
                rp.held_entity = Some(m);
            }
            rp.held = Some(held);
        }
    }
}

fn interpolate(
    time: Res<Time>,
    mut q: Query<(&mut RemotePlayer, &mut Transform)>,
    mut models: Query<&mut Transform, Without<RemotePlayer>>,
) {
    let dt = time.delta_secs();
    let k = 1.0 - (-18.0 * dt).exp();
    for (mut rp, mut t) in &mut q {
        if t.translation.distance(rp.target_pos) > 5.0 {
            t.translation = rp.target_pos;
        } else {
            t.translation = t.translation.lerp(rp.target_pos, k);
        }
        if !rp.alive {
            rp.death_anim = (rp.death_anim + dt * 3.0).min(1.0);
        }
        let target_rot = Quat::from_rotation_y(rp.target_yaw);
        t.rotation = t.rotation.slerp(target_rot, k);
        if let Some(model) = rp.model
            && let Ok(mut mt) = models.get_mut(model)
        {
            let crouch = if rp.crouch && rp.alive { 0.72 } else { 1.0 };
            mt.scale = Vec3::new(1.0, crouch, 1.0);
            let fall = if rp.alive { 0.0 } else { rp.death_anim };
            mt.rotation = Quat::from_rotation_x(fall * std::f32::consts::FRAC_PI_2);
            mt.translation = Vec3::new(0.0, fall * 0.15, 0.0);
        }
    }
}

fn name_tags(
    client: Option<Res<NetClient>>,
    cams: Query<(&Camera, &GlobalTransform), With<PlayerCamera>>,
    players: Query<(&RemotePlayer, &Transform)>,
    mut tags: Query<(&NameTag, &mut Node, &mut Visibility)>,
    smokes: Res<super::effects::ActiveSmokes>,
) {
    let Ok((cam, cam_t)) = cams.single() else { return };
    let alive = |id: PlayerId| {
        client
            .as_ref()
            .and_then(|c| c.player(id))
            .is_some_and(|p| p.alive)
    };
    for (tag, mut node, mut vis) in &mut tags {
        let pos = players.iter().find(|(rp, _)| rp.id == tag.0).map(|(_, t)| t.translation);
        let eye = cam_t.translation();
        // Hidden behind the camera or behind a smoke.
        let visible = pos.filter(|p| (*p - eye).dot(*cam_t.forward()) > 0.0 && !smokes.blocks(eye, *p + Vec3::Y * 1.6));
        let screen = visible.and_then(|p| cam.world_to_viewport(cam_t, p + Vec3::Y * 2.05).ok());
        match screen {
            Some(s) if alive(tag.0) => {
                node.left = Val::Px(s.x - 30.0);
                node.top = Val::Px(s.y - 10.0);
                *vis = Visibility::Inherited;
            }
            _ => *vis = Visibility::Hidden,
        }
    }
}

/// Running players make footstep noise; louder the closer they are.
fn footsteps(
    time: Res<Time>,
    cams: Query<&GlobalTransform, With<PlayerCamera>>,
    mut q: Query<(&mut RemotePlayer, &Transform)>,
    mut sfx: MessageWriter<PlaySfx>,
) {
    let Ok(cam) = cams.single() else { return };
    let ear = cam.translation();
    for (mut rp, t) in &mut q {
        if !rp.moving {
            rp.step_timer = 0.1;
            continue;
        }
        rp.step_timer -= time.delta_secs();
        if rp.step_timer > 0.0 {
            continue;
        }
        rp.step_timer = 0.36;
        let volume = (1.0 - t.translation.distance(ear) / 25.0).clamp(0.0, 1.0) * 0.6;
        if volume > 0.02 {
            sfx.write(PlaySfx::fx_at(Sound::Footstep, volume, t.translation));
        }
    }
}

/// A gun lying on the ground (mirrors `Snapshot::dropped`).
#[derive(Component)]
struct DroppedModel(u32);

fn sync_dropped(
    mut commands: Commands,
    client: Option<Res<NetClient>>,
    existing: Query<(Entity, &DroppedModel)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut textures: ResMut<SkinTextures>,
) {
    let Some(snap) = client.as_ref().and_then(|c| c.snapshot.as_ref()) else { return };
    for (e, d) in &existing {
        if !snap.dropped.iter().any(|g| g.id == d.0) {
            commands.entity(e).despawn();
        }
    }
    for gun in &snap.dropped {
        if existing.iter().any(|(_, d)| d.0 == gun.id) {
            continue;
        }
        let model = spawn_weapon_model(&mut commands, &mut meshes, &mut mats, &mut images, &mut textures, gun.weapon, None);
        // Lay it on its side.
        let rot = Quat::from_rotation_y(gun.yaw) * Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
        commands.entity(model).insert((
            InGameEntity,
            DroppedModel(gun.id),
            Transform::from_translation(Vec3::from(gun.pos)).with_rotation(rot),
        ));
    }
}
