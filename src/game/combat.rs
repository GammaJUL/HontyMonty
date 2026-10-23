//! Weapons on the client: switching, firing with spread & recoil, scopes,
//! reloads, grenade throws and the first-person viewmodel.

use std::collections::HashMap;

use bevy::camera::PerspectiveProjection;
use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;

use super::effects::{spawn_impact, spawn_tracer};
use super::player::{horizontal_to_vertical, LocalPlayer, PlayerCamera};
use super::{GameSet, GameUi, InGameEntity};
use crate::audio::{PlaySfx, Sound};
use crate::game_mode::Phase;
use crate::keybinds::{Action, Controls};
use crate::map::{raycast_map, CROUCH_EYE, STAND_EYE};
use crate::net::protocol::Cmd;
use crate::net::NetClient;
use crate::profile::Profile;
use crate::settings::Settings;
use crate::skin_textures::SkinTextures;
use crate::states::AppState;
use crate::ui::settings_menu::SettingsOpen;
use crate::weapon_model::{spawn_grenade_model, spawn_weapon_model};
use crate::weapons::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Held {
    Weapon(WeaponId),
    Grenade(GrenadeKind),
}

#[derive(Resource)]
pub struct Arsenal {
    pub held: Held,
    /// (magazine, reserve)
    pub ammo: HashMap<WeaponId, (u32, u32)>,
    pub next_fire: f32,
    pub reload_end: Option<f32>,
    pub draw_end: f32,
    /// Consecutive-shot counter for spray inaccuracy (decays when not firing).
    pub spray: f32,
    pub last_shot: f32,
    pub scope: usize,
    pub rescope: Option<(f32, usize)>,
    /// View punch (x = yaw, y = pitch) in radians.
    pub punch: Vec2,
    pub owned: (Option<WeaponId>, Option<WeaponId>),
    pub grenades: Vec<GrenadeKind>,
    pub throw_ready: f32,
    /// Current cone half-angle in radians (for the dynamic crosshair).
    pub spread: f32,
    pub kick: f32,
    pub slash: f32,
    pub move_speed_cache: f32,
}

impl Default for Arsenal {
    fn default() -> Self {
        Self {
            held: Held::Weapon(WeaponId::Knife),
            ammo: HashMap::new(),
            next_fire: 0.0,
            reload_end: None,
            draw_end: 0.0,
            spray: 0.0,
            last_shot: 0.0,
            scope: 0,
            rescope: None,
            punch: Vec2::ZERO,
            owned: (None, None),
            grenades: Vec::new(),
            throw_ready: 0.0,
            spread: 0.0,
            kick: 0.0,
            slash: 0.0,
            move_speed_cache: 6.4,
        }
    }
}

impl Arsenal {
    pub fn weapon(&self) -> Option<WeaponId> {
        match self.held {
            Held::Weapon(w) => Some(w),
            Held::Grenade(_) => None,
        }
    }

    pub fn held_ids(&self) -> (Option<WeaponId>, Option<GrenadeKind>) {
        match self.held {
            Held::Weapon(w) => (Some(w), None),
            Held::Grenade(g) => (None, Some(g)),
        }
    }

    pub fn move_speed(&self) -> f32 {
        let base = match self.held {
            Held::Weapon(w) => w.def().move_speed,
            Held::Grenade(_) => 6.1,
        };
        // Scoped snipers move slower, like in CS.
        if self.scope > 0 && self.weapon().is_some_and(|w| w.def().scope == ScopeStyle::Sniper) {
            base * 0.6
        } else {
            base
        }
    }

    pub fn zoom_fov(&self) -> Option<f32> {
        let w = self.weapon()?;
        if self.scope == 0 {
            return None;
        }
        w.def().zoom_fovs.get(self.scope - 1).copied()
    }

    pub fn zoom_sensitivity_factor(&self, settings: &Settings) -> f32 {
        match self.zoom_fov() {
            Some(z) => {
                let base = horizontal_to_vertical(settings.fov, 16.0 / 9.0);
                settings.zoom_sensitivity * ((z.to_radians() / 2.0).tan() / (base / 2.0).tan())
            }
            None => 1.0,
        }
    }

    pub fn reloading(&self) -> bool {
        self.reload_end.is_some()
    }

    fn switch(&mut self, held: Held, now: f32) {
        if self.held == held {
            return;
        }
        self.held = held;
        self.scope = 0;
        self.rescope = None;
        self.reload_end = None;
        self.spray = 0.0;
        self.draw_end = now + 0.4;
        self.next_fire = self.next_fire.max(now + 0.4);
    }

    fn best_weapon(&self) -> Held {
        if let Some(w) = self.owned.0 {
            Held::Weapon(w)
        } else if let Some(w) = self.owned.1 {
            Held::Weapon(w)
        } else {
            Held::Weapon(WeaponId::Knife)
        }
    }
}

const VIEWMODEL_SCALE: f32 = 0.7;

#[derive(Component)]
pub struct ViewModel;

#[derive(Component)]
struct ViewModelContent(Held);

pub fn plugin(app: &mut App) {
    app.init_resource::<Arsenal>()
        .add_systems(OnEnter(AppState::InGame), |mut c: Commands| c.insert_resource(Arsenal::default()))
        .add_systems(
            Update,
            (sync_inventory, switch_weapons, reload, fire, scope_and_fov, recover)
                .chain()
                .in_set(GameSet::Simulate),
        )
        .add_systems(Update, (rebuild_viewmodel, animate_viewmodel).chain().in_set(GameSet::Present));
}

fn full_ammo(w: WeaponId) -> (u32, u32) {
    let d = w.def();
    (d.mag, d.reserve)
}

/// Mirrors the server-side inventory into the local arsenal.
fn sync_inventory(
    time: Res<Time>,
    client: Option<Res<NetClient>>,
    mut arsenal: ResMut<Arsenal>,
    mut last_spawn: Local<u32>,
) {
    let Some(me) = client.as_ref().and_then(|c| c.me()) else { return };
    let now = time.elapsed_secs();
    let owned = (me.primary, me.secondary);
    let respawned = me.spawn_seq != *last_spawn;
    *last_spawn = me.spawn_seq;
    if respawned {
        arsenal.ammo.clear();
        arsenal.punch = Vec2::ZERO;
        arsenal.reload_end = None;
        arsenal.scope = 0;
    }
    for w in [owned.0, owned.1].into_iter().flatten() {
        arsenal.ammo.entry(w).or_insert_with(|| full_ammo(w));
    }
    arsenal.ammo.retain(|w, _| Some(*w) == owned.0 || Some(*w) == owned.1);
    if owned != arsenal.owned {
        let old = arsenal.owned;
        arsenal.owned = owned;
        // Freshly bought guns go straight to the hands.
        if let Some(p) = owned.0
            && old.0 != Some(p)
        {
            arsenal.switch(Held::Weapon(p), now);
        } else if let Some(s) = owned.1
            && old.1 != Some(s)
            && arsenal.weapon() != owned.0
        {
            arsenal.switch(Held::Weapon(s), now);
        }
    }
    arsenal.grenades = me.grenades.clone();
    let still_valid = match arsenal.held {
        Held::Weapon(WeaponId::Knife) => true,
        Held::Weapon(w) => Some(w) == owned.0 || Some(w) == owned.1,
        Held::Grenade(g) => arsenal.grenades.contains(&g) || now < arsenal.throw_ready,
    };
    if !still_valid || respawned && arsenal.held == Held::Weapon(WeaponId::Knife) {
        let next = match arsenal.held {
            Held::Grenade(_) => arsenal
                .grenades
                .first()
                .map(|g| Held::Grenade(*g))
                .unwrap_or_else(|| arsenal.best_weapon()),
            _ => arsenal.best_weapon(),
        };
        arsenal.switch(next, now);
    }
    arsenal.move_speed_cache = arsenal.move_speed();
}

fn switch_weapons(
    time: Res<Time>,
    controls: Controls,
    ui: Res<GameUi>,
    open: Res<SettingsOpen>,
    mut wheel: MessageReader<MouseWheel>,
    mut arsenal: ResMut<Arsenal>,
) {
    let scroll: f32 = wheel.read().map(|w| w.y).sum();
    if ui.any_open(&open) {
        return;
    }
    let now = time.elapsed_secs();
    let mut slots: Vec<Held> = Vec::new();
    if let Some(p) = arsenal.owned.0 {
        slots.push(Held::Weapon(p));
    }
    if let Some(s) = arsenal.owned.1 {
        slots.push(Held::Weapon(s));
    }
    slots.push(Held::Weapon(WeaponId::Knife));
    let mut unique: Vec<GrenadeKind> = Vec::new();
    for g in &arsenal.grenades {
        if !unique.contains(g) {
            unique.push(*g);
        }
    }
    for g in &unique {
        slots.push(Held::Grenade(*g));
    }

    if controls.just_pressed(Action::Primary)
        && let Some(p) = arsenal.owned.0
    {
        arsenal.switch(Held::Weapon(p), now);
    }
    if controls.just_pressed(Action::Secondary)
        && let Some(s) = arsenal.owned.1
    {
        arsenal.switch(Held::Weapon(s), now);
    }
    if controls.just_pressed(Action::Knife) {
        arsenal.switch(Held::Weapon(WeaponId::Knife), now);
    }
    if controls.just_pressed(Action::Grenades) && !unique.is_empty() {
        let next = match arsenal.held {
            Held::Grenade(g) => {
                let i = unique.iter().position(|x| *x == g).unwrap_or(0);
                unique[(i + 1) % unique.len()]
            }
            _ => unique[0],
        };
        arsenal.switch(Held::Grenade(next), now);
    }
    if scroll != 0.0 {
        let i = slots.iter().position(|h| *h == arsenal.held).unwrap_or(0) as i32;
        let d = if scroll > 0.0 { -1 } else { 1 };
        let n = slots.len() as i32;
        let next = slots[((i + d).rem_euclid(n)) as usize];
        arsenal.switch(next, now);
    }
}

fn reload(
    time: Res<Time>,
    controls: Controls,
    ui: Res<GameUi>,
    open: Res<SettingsOpen>,
    mut arsenal: ResMut<Arsenal>,
    mut sfx: MessageWriter<PlaySfx>,
) {
    let now = time.elapsed_secs();
    let Some(w) = arsenal.weapon() else { return };
    if w == WeaponId::Knife {
        return;
    }
    if let Some(end) = arsenal.reload_end {
        if now >= end {
            arsenal.reload_end = None;
            let def = w.def();
            if let Some((mag, reserve)) = arsenal.ammo.get_mut(&w) {
                let need = def.mag - *mag;
                let take = need.min(*reserve);
                *mag += take;
                *reserve -= take;
            }
        }
        return;
    }
    let wants = !ui.any_open(&open) && controls.just_pressed(Action::Reload);
    let (mag, reserve) = arsenal.ammo.get(&w).copied().unwrap_or((0, 0));
    if wants && mag < w.def().mag && reserve > 0 {
        start_reload(&mut arsenal, w, now);
        sfx.write(PlaySfx::fx(Sound::Reload, 0.6));
    }
}

fn start_reload(arsenal: &mut Arsenal, w: WeaponId, now: f32) {
    arsenal.reload_end = Some(now + w.def().reload_time);
    arsenal.scope = 0;
    arsenal.rescope = None;
}

fn random_in_cone(dir: Vec3, half_angle: f32, rng: &mut fastrand::Rng) -> Vec3 {
    if half_angle <= 0.0 {
        return dir;
    }
    // CS-like distribution: more shots near the center.
    let r = half_angle * rng.f32().sqrt() * rng.f32().sqrt().max(0.3);
    let theta = rng.f32() * std::f32::consts::TAU;
    let up = if dir.y.abs() > 0.99 { Vec3::X } else { Vec3::Y };
    let right = dir.cross(up).normalize();
    let up = right.cross(dir).normalize();
    (dir + right * (r.tan() * theta.cos()) + up * (r.tan() * theta.sin())).normalize()
}

pub fn current_spread(def: &WeaponDef, arsenal: &Arsenal, p: &LocalPlayer) -> f32 {
    let scoped = arsenal.scope > 0 && def.scope != ScopeStyle::None;
    let base = if scoped { def.scoped_spread } else { def.base_spread };
    let speed = Vec2::new(p.vel.x, p.vel.z).length();
    let frac = (speed / def.move_speed).clamp(0.0, 1.0);
    // Walking slowly is accurate, like counter-strafing.
    let move_frac = ((frac - 0.34) / 0.66).clamp(0.0, 1.0);
    let mut spread = base + def.move_spread * move_frac.powf(1.3);
    if !p.on_ground {
        spread += def.jump_spread;
    }
    spread += def.recoil_spread * arsenal.spray.min(12.0);
    if p.crouch && p.on_ground {
        spread *= 0.8;
    }
    spread
}

#[allow(clippy::too_many_arguments)]
fn fire(
    mut commands: Commands,
    time: Res<Time>,
    controls: Controls,
    ui: Res<GameUi>,
    open: Res<SettingsOpen>,
    client: Option<ResMut<NetClient>>,
    mut arsenal: ResMut<Arsenal>,
    players: Query<(&LocalPlayer, &Transform)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut sfx: MessageWriter<PlaySfx>,
    mut rng: Local<Option<fastrand::Rng>>,
) {
    let rng = rng.get_or_insert_with(fastrand::Rng::new);
    let Some(mut client) = client else { return };
    let Ok((p, t)) = players.single() else { return };
    let now = time.elapsed_secs();
    let phase = client.snapshot.as_ref().map(|s| s.phase).unwrap_or(Phase::Lobby);
    if let Held::Weapon(w) = arsenal.held
        && w != WeaponId::Knife
    {
        arsenal.spread = current_spread(w.def(), &arsenal, p);
    } else {
        arsenal.spread = 0.0;
    }
    if !p.alive || ui.any_open(&open) || !matches!(phase, Phase::Live | Phase::RoundEnd) {
        return;
    }
    let eye = t.translation + Vec3::Y * if p.crouch { CROUCH_EYE } else { STAND_EYE };
    let aim = Quat::from_euler(EulerRot::YXZ, p.yaw + arsenal.punch.x, p.pitch + arsenal.punch.y, 0.0);
    let forward = aim * Vec3::NEG_Z;
    let muzzle = eye + aim * Vec3::new(0.18, -0.14, -0.6);

    match arsenal.held {
        Held::Grenade(kind) => {
            let strong = controls.just_pressed(Action::Fire);
            let lob = controls.just_pressed(Action::Scope);
            if (strong || lob) && now >= arsenal.throw_ready && now >= arsenal.draw_end {
                let speed = if strong { 17.0 } else { 7.5 };
                let vel = forward * speed + Vec3::Y * 1.5 + p.vel * 0.5;
                client.send_cmd(Cmd::Throw {
                    kind,
                    origin: (eye + forward * 0.3).to_array(),
                    vel: vel.to_array(),
                });
                arsenal.throw_ready = now + 0.8;
                arsenal.kick = 1.0;
                sfx.write(PlaySfx::fx(Sound::Throw, 0.7));
            }
        }
        Held::Weapon(WeaponId::Knife) => {
            if controls.pressed(Action::Fire) && now >= arsenal.next_fire && now >= arsenal.draw_end {
                let def = WeaponId::Knife.def();
                arsenal.next_fire = now + def.fire_interval;
                arsenal.slash = 1.0;
                client.send_cmd(Cmd::Fire {
                    weapon: WeaponId::Knife,
                    origin: eye.to_array(),
                    dirs: vec![forward.to_array()],
                });
                sfx.write(PlaySfx::fx(Sound::Shot(WeaponClass::Knife), 0.6));
            }
        }
        Held::Weapon(w) => {
            let def = w.def();
            let trigger = if def.automatic {
                controls.pressed(Action::Fire)
            } else {
                controls.just_pressed(Action::Fire)
            };
            if !trigger || arsenal.reloading() || now < arsenal.next_fire || now < arsenal.draw_end {
                return;
            }
            let (mag, reserve) = arsenal.ammo.get(&w).copied().unwrap_or((0, 0));
            if mag == 0 {
                if controls.just_pressed(Action::Fire) {
                    sfx.write(PlaySfx::fx(Sound::Empty, 0.6));
                }
                if reserve > 0 {
                    start_reload(&mut arsenal, w, now);
                    sfx.write(PlaySfx::fx(Sound::Reload, 0.6));
                }
                return;
            }
            if let Some(a) = arsenal.ammo.get_mut(&w) {
                a.0 -= 1;
            }
            arsenal.next_fire = now + def.fire_interval;
            let spread = current_spread(def, &arsenal, p);
            let dirs: Vec<Vec3> = (0..def.pellets)
                .map(|_| random_in_cone(forward, spread, rng))
                .collect();
            client.send_cmd(Cmd::Fire {
                weapon: w,
                origin: eye.to_array(),
                dirs: dirs.iter().map(|d| d.to_array()).collect(),
            });
            for (i, d) in dirs.iter().enumerate() {
                let dist = raycast_map(eye, *d, def.range);
                let end = eye + *d * dist.unwrap_or(def.range);
                if i == 0 || i % 3 == 0 {
                    spawn_tracer(&mut commands, muzzle, end, true);
                }
                if dist.is_some() {
                    spawn_impact(&mut commands, &mut meshes, &mut mats, end, *d);
                }
            }
            // Recoil: vertical climb, then horizontal sway once the spray gets long.
            let kick = def.recoil_kick * (1.0 + rng.f32() * 0.25);
            arsenal.punch.y += kick;
            let sway = if arsenal.spray > 4.0 { (arsenal.spray * 0.7).sin() * kick * 0.9 } else { (rng.f32() - 0.5) * kick * 0.4 };
            arsenal.punch.x += sway;
            arsenal.spray += 1.0;
            arsenal.last_shot = now;
            arsenal.kick = 1.0;
            if def.scope == ScopeStyle::Sniper && arsenal.scope > 0 {
                arsenal.rescope = Some((now + def.fire_interval * 0.9, arsenal.scope));
                arsenal.scope = 0;
            }
            sfx.write(PlaySfx::fx(Sound::Shot(def.class), 0.8));
        }
    }
}

fn scope_and_fov(
    time: Res<Time>,
    controls: Controls,
    ui: Res<GameUi>,
    open: Res<SettingsOpen>,
    settings: Res<Settings>,
    mut arsenal: ResMut<Arsenal>,
    mut cams: Query<&mut Projection, With<PlayerCamera>>,
    window: Single<&Window>,
) {
    let now = time.elapsed_secs();
    if let Some((at, level)) = arsenal.rescope
        && now >= at
    {
        arsenal.rescope = None;
        if controls.pressed(Action::Scope) || level > 0 {
            arsenal.scope = level;
        }
    }
    if !ui.any_open(&open)
        && controls.just_pressed(Action::Scope)
        && !arsenal.reloading()
        && let Some(w) = arsenal.weapon()
    {
        let levels = w.def().zoom_fovs.len();
        if levels > 0 {
            arsenal.scope = (arsenal.scope + 1) % (levels + 1);
            arsenal.rescope = None;
        }
    }
    let aspect = window.width() / window.height().max(1.0);
    let base = horizontal_to_vertical(settings.fov, aspect);
    let target = match arsenal.zoom_fov() {
        Some(z) => horizontal_to_vertical(z, aspect),
        None => base,
    };
    for mut proj in &mut cams {
        if let Projection::Perspective(PerspectiveProjection { fov, .. }) = proj.as_mut() {
            let k = (time.delta_secs() * 25.0).min(1.0);
            *fov += (target - *fov) * k;
        }
    }
}

fn recover(time: Res<Time>, mut arsenal: ResMut<Arsenal>) {
    let dt = time.delta_secs();
    let now = time.elapsed_secs();
    let k = 1.0 - (-6.0 * dt).exp();
    arsenal.punch *= 1.0 - k;
    if now - arsenal.last_shot > 0.25 {
        arsenal.spray = (arsenal.spray - dt * 18.0).max(0.0);
    }
    arsenal.kick = (arsenal.kick - dt * 8.0).max(0.0);
    arsenal.slash = (arsenal.slash - dt * 3.5).max(0.0);
}

// ---------------------------------------------------------------- viewmodel

#[allow(clippy::too_many_arguments)]
fn rebuild_viewmodel(
    mut commands: Commands,
    arsenal: Res<Arsenal>,
    profile: Res<Profile>,
    cams: Query<Entity, With<PlayerCamera>>,
    current: Query<(Entity, &ViewModelContent)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut textures: ResMut<SkinTextures>,
) {
    let Ok(cam) = cams.single() else { return };
    if current.iter().any(|(_, c)| c.0 == arsenal.held) {
        return;
    }
    for (e, _) in &current {
        commands.entity(e).despawn();
    }
    let model = match arsenal.held {
        Held::Weapon(w) => {
            let skin = profile.loadout().into_iter().find(|s| s.weapon == w);
            spawn_weapon_model(&mut commands, &mut meshes, &mut mats, &mut images, &mut textures, w, skin)
        }
        Held::Grenade(g) => spawn_grenade_model(&mut commands, &mut meshes, &mut mats, g),
    };
    let root = commands
        .spawn((
            InGameEntity,
            ViewModel,
            ViewModelContent(arsenal.held),
            Transform::from_xyz(0.2, -0.2, -0.42).with_scale(Vec3::splat(VIEWMODEL_SCALE)),
            Visibility::default(),
        ))
        .id();
    commands.entity(root).add_child(model);
    // Hand holding the gun.
    let hand = commands
        .spawn((
            Mesh3d(meshes.add(Cuboid::new(0.07, 0.09, 0.08))),
            MeshMaterial3d(mats.add(StandardMaterial {
                base_color: crate::profile::pal(crate::profile::SKIN_TONES, profile.appearance.skin_tone),
                perceptual_roughness: 0.8,
                ..default()
            })),
            Transform::from_xyz(0.0, -0.05, 0.03),
        ))
        .id();
    commands.entity(root).add_child(hand);
    commands.entity(cam).add_child(root);
}

fn animate_viewmodel(
    time: Res<Time>,
    arsenal: Res<Arsenal>,
    players: Query<&LocalPlayer>,
    mut q: Query<(&mut Transform, &mut Visibility), With<ViewModel>>,
) {
    let Ok(p) = players.single() else { return };
    let now = time.elapsed_secs();
    let speed = Vec2::new(p.vel.x, p.vel.z).length();
    let bob = if p.on_ground { (speed / 6.0).min(1.0) } else { 0.0 };
    for (mut t, mut vis) in &mut q {
        let sniper_scoped = arsenal.scope > 0
            && arsenal.weapon().is_some_and(|w| w.def().scope == ScopeStyle::Sniper);
        let want = if sniper_scoped || !p.alive { Visibility::Hidden } else { Visibility::Inherited };
        if *vis != want {
            *vis = want;
        }
        let mut pos = Vec3::new(0.2, -0.2, -0.42);
        pos.x += (now * 7.0).sin() * 0.012 * bob;
        pos.y += (now * 14.0).sin().abs() * 0.01 * bob;
        pos.z += arsenal.kick * 0.05;
        let mut rot = Quat::from_rotation_x(arsenal.kick * 0.12);
        if let Some(end) = arsenal.reload_end
            && let Some(w) = arsenal.weapon()
        {
            let total = w.def().reload_time;
            let k = ((end - now) / total).clamp(0.0, 1.0);
            let dip = (k * std::f32::consts::PI).sin();
            pos.y -= dip * 0.12;
            rot *= Quat::from_rotation_x(-dip * 0.6) * Quat::from_rotation_z(dip * 0.4);
        }
        let draw = ((arsenal.draw_end - now) / 0.4).clamp(0.0, 1.0);
        pos.y -= draw * 0.2;
        if arsenal.slash > 0.0 {
            let s = (arsenal.slash * std::f32::consts::PI).sin();
            rot *= Quat::from_rotation_y(s * 0.9) * Quat::from_rotation_x(-s * 0.4);
            pos.x -= s * 0.1;
        }
        if arsenal.scope > 0 && !sniper_scoped {
            pos = Vec3::new(0.0, -0.12, -0.32);
        }
        t.translation = pos;
        t.rotation = rot;
        t.scale = Vec3::splat(VIEWMODEL_SCALE);
    }
}
