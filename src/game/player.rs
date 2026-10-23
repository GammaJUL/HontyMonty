//! Local player: mouse look, movement, crouching, spawning and the camera.

use std::f32::consts::FRAC_PI_2;

use bevy::camera::PerspectiveProjection;
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;

use super::combat::Arsenal;
use super::remote::RemotePlayer;
use super::{GameSet, GameUi, InGameEntity};
use crate::audio::{PlaySfx, Sound};
use crate::game_mode::Phase;
use crate::keybinds::{Action, Controls};
use crate::map::{self, collides, CROUCH_EYE, CROUCH_HEIGHT, STAND_EYE, STAND_HEIGHT};
use crate::net::protocol::{ClientState, PlayerId};
use crate::net::NetClient;
use crate::settings::Settings;
use crate::states::AppState;
use crate::ui::settings_menu::SettingsOpen;

pub const GRAVITY: f32 = 20.0;
pub const JUMP_SPEED: f32 = 6.6;
pub const WALK_FACTOR: f32 = 0.52;
pub const CROUCH_FACTOR: f32 = 0.34;

#[derive(Component)]
pub struct LocalPlayer {
    pub vel: Vec3,
    pub on_ground: bool,
    pub crouch: bool,
    /// Smoothed 0..1 for the camera height.
    pub crouch_amount: f32,
    pub yaw: f32,
    pub pitch: f32,
    pub spawn_seq: u32,
    pub alive: bool,
    pub step_timer: f32,
    pub death_time: f32,
}

impl LocalPlayer {
    pub fn height(&self) -> f32 {
        if self.crouch { CROUCH_HEIGHT } else { STAND_HEIGHT }
    }
}

#[derive(Component)]
pub struct PlayerCamera;

/// Who the camera follows while we are dead.
#[derive(Resource, Default)]
pub struct Spectate {
    pub target: Option<PlayerId>,
}

const DEATH_CAM_TIME: f32 = 2.0;
const SPECTATE_DISTANCE: f32 = 3.0;

pub fn plugin(app: &mut App) {
    app.init_resource::<Spectate>()
        .add_systems(OnEnter(AppState::InGame), (spawn_local_player, |mut s: ResMut<Spectate>| s.target = None))
        .add_systems(
            Update,
            (handle_spawns, choose_spectate_target, look, movement, update_camera, write_net_state)
                .chain()
                .in_set(GameSet::Input),
        );
}

fn spawn_local_player(mut commands: Commands, settings: Res<Settings>) {
    commands
        .spawn((
            InGameEntity,
            LocalPlayer {
                vel: Vec3::ZERO,
                on_ground: false,
                crouch: false,
                crouch_amount: 0.0,
                yaw: 0.0,
                pitch: 0.0,
                spawn_seq: 0,
                alive: false,
                step_timer: 0.0,
                death_time: 0.0,
            },
            Transform::from_xyz(0.0, 5.0, 0.0),
            Visibility::default(),
        ))
        .with_children(|p| {
            p.spawn((
                PlayerCamera,
                Camera3d::default(),
                Camera {
                    clear_color: ClearColorConfig::Custom(Color::srgb(0.53, 0.68, 0.86)),
                    ..default()
                },
                Projection::from(PerspectiveProjection {
                    fov: horizontal_to_vertical(settings.fov, 16.0 / 9.0),
                    near: 0.03,
                    ..default()
                }),
                Transform::from_xyz(0.0, STAND_EYE, 0.0),
            ));
        });
}

pub fn horizontal_to_vertical(h_deg: f32, aspect: f32) -> f32 {
    2.0 * ((h_deg.to_radians() / 2.0).tan() / aspect).atan()
}

fn handle_spawns(
    client: Option<Res<NetClient>>,
    time: Res<Time>,
    mut q: Query<(&mut LocalPlayer, &mut Transform)>,
) {
    let Some(me) = client.as_ref().and_then(|c| c.me()) else { return };
    let Ok((mut p, mut t)) = q.single_mut() else { return };
    if me.spawn_seq != p.spawn_seq {
        p.spawn_seq = me.spawn_seq;
        t.translation = Vec3::from(me.spawn_pos);
        p.yaw = me.spawn_yaw;
        p.pitch = 0.0;
        p.vel = Vec3::ZERO;
        p.crouch = false;
    }
    if p.alive && !me.alive {
        p.death_time = time.elapsed_secs();
    }
    p.alive = me.alive;
}

fn look(
    motion: Res<AccumulatedMouseMotion>,
    settings: Res<Settings>,
    ui: Res<GameUi>,
    open: Res<SettingsOpen>,
    arsenal: Res<Arsenal>,
    mut q: Query<&mut LocalPlayer>,
) {
    if ui.any_open(&open) {
        return;
    }
    let Ok(mut p) = q.single_mut() else { return };
    let zoom = arsenal.zoom_sensitivity_factor(&settings);
    let scale = (settings.sensitivity * 0.022 * zoom).to_radians();
    let dy = if settings.invert_y { -motion.delta.y } else { motion.delta.y };
    p.yaw -= motion.delta.x * scale;
    p.pitch = (p.pitch - dy * scale).clamp(-FRAC_PI_2 + 0.01, FRAC_PI_2 - 0.01);
}

#[allow(clippy::too_many_arguments)]
fn movement(
    time: Res<Time>,
    controls: Controls,
    client: Option<Res<NetClient>>,
    ui: Res<GameUi>,
    open: Res<SettingsOpen>,
    arsenal: Res<Arsenal>,
    mut q: Query<(&mut LocalPlayer, &mut Transform)>,
    mut sfx: MessageWriter<PlaySfx>,
) {
    let Ok((mut p, mut t)) = q.single_mut() else { return };
    let dt = time.delta_secs().min(0.05);
    let phase = client
        .as_ref()
        .and_then(|c| c.snapshot.as_ref())
        .map(|s| s.phase)
        .unwrap_or(Phase::Lobby);
    if !p.alive {
        p.vel = Vec3::ZERO;
        return;
    }
    let frozen = matches!(phase, Phase::Freeze | Phase::MatchEnd);
    let input_ok = !ui.any_open(&open) && !frozen;

    let mut wish = Vec3::ZERO;
    if input_ok {
        let fwd = Vec3::new(-p.yaw.sin(), 0.0, -p.yaw.cos());
        let right = Vec3::new(fwd.z * -1.0, 0.0, fwd.x);
        if controls.pressed(Action::MoveForward) {
            wish += fwd;
        }
        if controls.pressed(Action::MoveBack) {
            wish -= fwd;
        }
        if controls.pressed(Action::MoveRight) {
            wish += right;
        }
        if controls.pressed(Action::MoveLeft) {
            wish -= right;
        }
    }
    let wish = wish.normalize_or_zero();

    // Crouch: in the air we tuck the legs (feet go up) which allows crouch-jumps.
    let want_crouch = input_ok && controls.pressed(Action::Crouch);
    let delta_h = STAND_HEIGHT - CROUCH_HEIGHT;
    if want_crouch && !p.crouch {
        p.crouch = true;
        if !p.on_ground {
            let lifted = t.translation + Vec3::Y * delta_h;
            if !collides(lifted, CROUCH_HEIGHT) {
                t.translation = lifted;
            }
        }
    } else if !want_crouch && p.crouch {
        if p.on_ground {
            if !collides(t.translation, STAND_HEIGHT) {
                p.crouch = false;
            }
        } else {
            let lowered = t.translation - Vec3::Y * delta_h;
            if !collides(lowered, STAND_HEIGHT) {
                t.translation = lowered;
                p.crouch = false;
            } else if !collides(t.translation, STAND_HEIGHT) {
                p.crouch = false;
            }
        }
    }

    let walking = input_ok && controls.pressed(Action::Walk);
    let mut speed = arsenal.move_speed();
    if p.crouch {
        speed *= CROUCH_FACTOR;
    } else if walking {
        speed *= WALK_FACTOR;
    }
    let target = wish * speed;
    let horiz = Vec3::new(p.vel.x, 0.0, p.vel.z);
    let accel = if p.on_ground { 12.0 } else { 1.5 };
    let new_h = horiz.lerp(target, 1.0 - (-accel * dt).exp());
    p.vel.x = new_h.x;
    p.vel.z = new_h.z;

    if input_ok && p.on_ground && controls.just_pressed(Action::Jump) {
        p.vel.y = JUMP_SPEED;
        p.on_ground = false;
    }
    p.vel.y -= GRAVITY * dt;
    p.vel.y = p.vel.y.max(-40.0);

    let height = p.height();
    let result = map::move_player(t.translation, p.vel, height, dt, p.on_ground);
    let landed = !p.on_ground && result.on_ground;
    t.translation = result.pos;
    p.vel = result.vel;
    p.on_ground = result.on_ground;
    if p.on_ground && p.vel.y < 0.0 {
        p.vel.y = 0.0;
    }

    // Safety net if something goes wrong with collision.
    if t.translation.y < -20.0
        && let Some(me) = client.as_ref().and_then(|c| c.me())
    {
        t.translation = Vec3::from(me.spawn_pos);
        p.vel = Vec3::ZERO;
    }

    let hspeed = Vec2::new(p.vel.x, p.vel.z).length();
    if p.on_ground && hspeed > speed.max(0.1) * 0.6 && !walking && !p.crouch {
        p.step_timer -= dt;
        if p.step_timer <= 0.0 {
            p.step_timer = 0.36;
            sfx.write(PlaySfx::fx(Sound::Footstep, 0.25));
        }
    }
    if landed {
        sfx.write(PlaySfx::fx(Sound::Footstep, 0.4));
    }
    p.crouch_amount += ((p.crouch as u8 as f32) - p.crouch_amount) * (1.0 - (-14.0 * dt).exp());
}

fn update_camera(
    time: Res<Time>,
    players: Query<(&LocalPlayer, &mut Transform), (Without<PlayerCamera>, Without<RemotePlayer>)>,
    mut cams: Query<&mut Transform, With<PlayerCamera>>,
    remotes: Query<(&RemotePlayer, &Transform), Without<PlayerCamera>>,
    spectate: Res<Spectate>,
    arsenal: Res<Arsenal>,
    effects: Res<super::effects::ScreenEffects>,
) {
    for (p, mut t) in players {
        t.rotation = Quat::from_rotation_y(p.yaw);
        let Ok(mut cam) = cams.single_mut() else { continue };
        let followed = spectate
            .target
            .and_then(|id| remotes.iter().find(|(rp, _)| rp.id == id))
            .map(|(_, rt)| rt.translation);
        if let (false, Some(target)) = (p.alive, followed) {
            // Third-person orbit around the spectated player, pulled in front of walls.
            t.translation = target;
            let pivot = target + Vec3::Y * 1.7;
            let back = Quat::from_euler(EulerRot::YXZ, p.yaw, p.pitch, 0.0) * Vec3::Z;
            let dist = map::raycast_map(pivot, back, SPECTATE_DISTANCE)
                .map(|d| (d - 0.2).max(0.3))
                .unwrap_or(SPECTATE_DISTANCE);
            let pitch_rot = Quat::from_rotation_x(p.pitch);
            cam.translation = Vec3::Y * 1.7 + pitch_rot * Vec3::Z * dist;
            cam.rotation = pitch_rot;
            continue;
        }
        let mut eye = STAND_EYE + (CROUCH_EYE - STAND_EYE) * p.crouch_amount;
        // Crouching while airborne moves the feet up, keep the eye where the head is.
        if p.crouch && !p.on_ground {
            eye = CROUCH_EYE;
        }
        let mut pitch = p.pitch + arsenal.punch.y;
        let mut yaw_off = arsenal.punch.x;
        if !p.alive {
            // Simple death cam: float up and look down at where we died.
            let k = ((time.elapsed_secs() - p.death_time) * 1.5).clamp(0.0, 1.0);
            eye += k * 2.0;
            pitch = pitch * (1.0 - k) + k * -0.7;
        }
        let shake = effects.shake;
        if shake > 0.0 {
            let t = time.elapsed_secs() * 50.0;
            pitch += t.sin() * shake * 0.02;
            yaw_off += (t * 1.3).cos() * shake * 0.02;
        }
        cam.translation = Vec3::new(0.0, eye, 0.0);
        cam.rotation = Quat::from_euler(EulerRot::YXZ, yaw_off, pitch, 0.0);
    }
}

fn write_net_state(
    client: Option<ResMut<NetClient>>,
    q: Query<(&LocalPlayer, &Transform)>,
    arsenal: Res<Arsenal>,
) {
    let Some(mut c) = client else { return };
    let Ok((p, t)) = q.single() else { return };
    let (weapon, grenade) = arsenal.held_ids();
    c.local = ClientState {
        pos: t.translation.to_array(),
        yaw: p.yaw,
        pitch: p.pitch,
        crouch: p.crouch,
        weapon,
        grenade,
        // Only running makes noise; walking (shift) and crouching are silent like in CS.
        moving: p.on_ground && !p.crouch && Vec2::new(p.vel.x, p.vel.z).length() > 3.4,
        ..default()
    };
}

/// Picks a living player to watch after the death cam; Fire / Scope cycle through them.
fn choose_spectate_target(
    time: Res<Time>,
    client: Option<Res<NetClient>>,
    controls: Controls,
    ui: Res<GameUi>,
    open: Res<SettingsOpen>,
    players: Query<&LocalPlayer>,
    mut spectate: ResMut<Spectate>,
) {
    let Ok(p) = players.single() else { return };
    let Some(c) = client else { return };
    let (Some(snap), Some(me)) = (c.snapshot.as_ref(), c.me()) else { return };
    if p.alive || time.elapsed_secs() - p.death_time < DEATH_CAM_TIME {
        spectate.target = None;
        return;
    }
    let alive_others = || snap.players.iter().filter(|o| o.alive && o.id != me.id);
    // Teammates first (like CS); anyone in free-for-all or once the team is wiped.
    let mut candidates: Vec<PlayerId> = alive_others()
        .filter(|o| snap.lobby.mode.is_ffa() || o.team == me.team)
        .map(|o| o.id)
        .collect();
    if candidates.is_empty() {
        candidates = alive_others().map(|o| o.id).collect();
    }
    if candidates.is_empty() {
        spectate.target = None;
        return;
    }
    let current = spectate.target.and_then(|t| candidates.iter().position(|id| *id == t));
    let step = if ui.any_open(&open) {
        0
    } else if controls.just_pressed(Action::Fire) {
        1
    } else if controls.just_pressed(Action::Scope) {
        -1
    } else {
        0
    };
    let n = candidates.len() as i32;
    let index = match current {
        Some(i) => (i as i32 + step).rem_euclid(n),
        None => 0,
    };
    spectate.target = Some(candidates[index as usize]);
}
