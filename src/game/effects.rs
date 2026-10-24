//! Visual/audio effects driven by server events: tracers, impacts, grenades,
//! smoke, fire, explosions, flashbangs, and HUD feed updates.

use bevy::prelude::*;

use super::hud::HudFeed;
use super::player::{LocalPlayer, PlayerCamera};
use super::{GameSet, InGameEntity};
use crate::audio::{PlaySfx, Sound};
use super::remote::RemotePlayer;
use crate::map::{line_of_sight, raycast_map, step_grenade};
use crate::net::protocol::GameEvent;
use crate::net::{NetClient, NetEvent};
use crate::profile::Profile;
use crate::settings::Settings;
use crate::weapon_model::spawn_grenade_model;
use crate::weapons::*;

const GRENADE_STEP: f32 = 1.0 / 120.0;

#[derive(Resource, Default)]
pub struct ScreenEffects {
    pub shake: f32,
    /// Seconds of full white remaining + fade length.
    pub flash_hold: f32,
    pub flash_fade: f32,
    pub flash_total_fade: f32,
    pub damage: f32,
    pub hitmarker: f32,
    pub hit_head: bool,
}

#[derive(Component)]
pub struct Tracer {
    from: Vec3,
    to: Vec3,
    ttl: f32,
    mine: bool,
}

#[derive(Component)]
struct Lifetime(f32);

#[derive(Component)]
struct Impact;

#[derive(Component)]
struct GrenadeVisual {
    id: u32,
    pos: Vec3,
    vel: Vec3,
    acc: f32,
}

#[derive(Component)]
struct FireArea {
    id: u32,
}

#[derive(Component)]
struct Flicker {
    base: f32,
    seed: f32,
}

#[derive(Component)]
struct SmokePuff {
    age: f32,
    size: f32,
    mat: Handle<StandardMaterial>,
}

/// One smoke grenade volume (the puffs are separate entities).
#[derive(Component)]
struct SmokeCloud {
    pos: Vec3,
    age: f32,
}

/// Active smoke volumes as (center, strength 0..1), rebuilt every frame.
#[derive(Resource, Default)]
pub struct ActiveSmokes(pub Vec<(Vec3, f32)>);

const SMOKE_HALF_HEIGHT: f32 = 2.2;

impl ActiveSmokes {
    /// 1.0 deep inside a smoke, fading to 0.0 towards its edge.
    pub fn density_at(&self, p: Vec3) -> f32 {
        self.0
            .iter()
            .map(|(c, strength)| {
                let d = p - *c;
                let n = Vec3::new(d.x / SMOKE_RADIUS, d.y / SMOKE_HALF_HEIGHT, d.z / SMOKE_RADIUS).length();
                (1.0 - smoothstep(0.55, 1.05, n)) * strength
            })
            .fold(0.0, f32::max)
    }

    /// True if the segment passes through enough smoke to hide what is behind it.
    pub fn blocks(&self, a: Vec3, b: Vec3) -> bool {
        if self.0.is_empty() {
            return false;
        }
        let len = a.distance(b);
        let steps = (len / 0.25).ceil().max(1.0) as usize;
        let step = len / steps as f32;
        let thickness: f32 = (0..steps)
            .map(|i| self.density_at(a.lerp(b, (i as f32 + 0.5) / steps as f32)) * step)
            .sum();
        thickness > 1.2
    }
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn smoke_strength(age: f32) -> f32 {
    let grow = (age / 1.2).clamp(0.0, 1.0);
    let fade = ((SMOKE_DURATION - age) / 2.5).clamp(0.0, 1.0);
    grow * fade
}

#[derive(Component)]
struct Expand {
    age: f32,
    len: f32,
    max: f32,
    mat: Handle<StandardMaterial>,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<ScreenEffects>()
        .init_resource::<ActiveSmokes>()
        .add_systems(
            OnEnter(crate::states::AppState::InGame),
            |mut c: Commands| {
                c.insert_resource(ScreenEffects::default());
                c.insert_resource(ActiveSmokes::default());
            },
        )
        .add_systems(Startup, make_particle_assets)
        .add_systems(Update, (handle_events, stattrak_counter, event_particles).in_set(GameSet::Simulate))
        .add_systems(
            Update,
            (
                draw_tracers,
                lifetimes,
                simulate_grenades,
                animate_smoke,
                track_smokes,
                animate_particles,
                animate_fire,
                animate_expand,
                decay_screen,
            )
                .in_set(GameSet::Present),
        );
}

pub fn spawn_tracer(commands: &mut Commands, from: Vec3, to: Vec3, mine: bool) {
    commands.spawn((InGameEntity, Tracer { from, to, ttl: 0.06, mine }));
}

/// A brief flash of light where a gun was fired.
pub fn spawn_muzzle_flash(commands: &mut Commands, pos: Vec3) {
    commands.spawn((
        InGameEntity,
        Lifetime(0.05),
        PointLight {
            intensity: 150_000.0,
            color: Color::srgb(1.0, 0.8, 0.45),
            range: 6.0,
            ..default()
        },
        Transform::from_translation(pos),
    ));
}

pub fn spawn_impact(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    mats: &mut Assets<StandardMaterial>,
    at: Vec3,
    dir: Vec3,
) {
    commands.spawn((
        InGameEntity,
        Impact,
        Lifetime(8.0),
        Mesh3d(meshes.add(Cuboid::new(0.05, 0.05, 0.05))),
        MeshMaterial3d(mats.add(StandardMaterial {
            base_color: Color::srgb(0.05, 0.05, 0.05),
            ..default()
        })),
        Transform::from_translation(at - dir * 0.01),
    ));
}

fn draw_tracers(time: Res<Time>, mut commands: Commands, mut gizmos: Gizmos, mut q: Query<(Entity, &mut Tracer)>) {
    for (e, mut t) in &mut q {
        let color = if t.mine {
            Color::srgba(1.0, 0.9, 0.5, 0.6)
        } else {
            Color::srgba(1.0, 0.8, 0.4, 0.9)
        };
        // Draw only the far part for my own tracers so they don't block the view.
        let from = if t.mine { t.from.lerp(t.to, 0.15) } else { t.from };
        gizmos.line(from, t.to, color);
        t.ttl -= time.delta_secs();
        if t.ttl <= 0.0 {
            commands.entity(e).despawn();
        }
    }
}

fn lifetimes(
    time: Res<Time>,
    mut commands: Commands,
    mut q: Query<(Entity, &mut Lifetime)>,
    impacts: Query<Entity, With<Impact>>,
) {
    for (e, mut l) in &mut q {
        l.0 -= time.delta_secs();
        if l.0 <= 0.0 {
            commands.entity(e).despawn();
        }
    }
    // Keep the number of bullet holes bounded.
    let n = impacts.iter().count();
    if n > 120 {
        for e in impacts.iter().take(n - 120) {
            commands.entity(e).despawn();
        }
    }
}

fn simulate_grenades(time: Res<Time>, mut q: Query<(&mut GrenadeVisual, &mut Transform)>) {
    for (mut g, mut t) in &mut q {
        g.acc += time.delta_secs();
        while g.acc >= GRENADE_STEP {
            g.acc -= GRENADE_STEP;
            let (mut pos, mut vel) = (g.pos, g.vel);
            step_grenade(&mut pos, &mut vel, GRENADE_STEP);
            g.pos = pos;
            g.vel = vel;
        }
        t.translation = g.pos;
        t.rotate_x(time.delta_secs() * g.vel.length() * 2.0);
    }
}

fn spawn_smoke(commands: &mut Commands, meshes: &mut Assets<Mesh>, mats: &mut Assets<StandardMaterial>, pos: Vec3) {
    commands.spawn((InGameEntity, SmokeCloud { pos, age: 0.0 }));
    let mesh = meshes.add(Sphere::new(1.0));
    for i in 0..22 {
        let a = i as f32 * 2.399;
        let r = (i as f32 / 22.0).sqrt() * (SMOKE_RADIUS - 1.0);
        let offset = Vec3::new(a.cos() * r, 0.6 + fastrand::f32() * 2.2, a.sin() * r);
        let grey = 0.62 + fastrand::f32() * 0.12;
        let mat = mats.add(StandardMaterial {
            base_color: Color::srgba(grey, grey, grey + 0.02, 0.0),
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 1.0,
            ..default()
        });
        commands.spawn((
            InGameEntity,
            SmokePuff {
                age: -(i as f32) * 0.02,
                size: 1.3 + fastrand::f32() * 0.7,
                mat: mat.clone(),
            },
            Mesh3d(mesh.clone()),
            MeshMaterial3d(mat),
            Transform::from_translation(pos + offset).with_scale(Vec3::splat(0.01)),
        ));
    }
}

/// Counts kills on the equipped StatTrak skin of the weapon used.
fn stattrak_counter(
    mut events: MessageReader<NetEvent>,
    client: Option<Res<NetClient>>,
    mut profile: ResMut<Profile>,
) {
    let me = client.as_ref().and_then(|c| c.my_id);
    let mut changed = false;
    for NetEvent(e) in events.read() {
        let GameEvent::Kill { killer, victim, source: DamageSource::Weapon(w), .. } = e else { continue };
        if Some(*killer) != me || killer == victim {
            continue;
        }
        let Some(uid) = profile.equipped.get(w).copied() else { continue };
        if let Some(item) = profile.inventory.iter_mut().find(|s| s.uid == uid && s.stattrak) {
            item.kills += 1;
            changed = true;
        }
    }
    if changed {
        profile.save();
    }
}

fn track_smokes(
    time: Res<Time>,
    mut commands: Commands,
    mut clouds: Query<(Entity, &mut SmokeCloud)>,
    mut active: ResMut<ActiveSmokes>,
) {
    active.0.clear();
    for (e, mut c) in &mut clouds {
        c.age += time.delta_secs();
        if c.age >= SMOKE_DURATION {
            commands.entity(e).despawn();
            continue;
        }
        active.0.push((c.pos + Vec3::Y * 1.6, smoke_strength(c.age)));
    }
}

fn animate_smoke(
    time: Res<Time>,
    mut commands: Commands,
    mut q: Query<(Entity, &mut SmokePuff, &mut Transform)>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    for (e, mut puff, mut t) in &mut q {
        puff.age += time.delta_secs();
        if puff.age < 0.0 {
            continue;
        }
        let grow = (puff.age / 1.2).min(1.0);
        let fade = ((SMOKE_DURATION - puff.age) / 2.5).clamp(0.0, 1.0);
        t.scale = Vec3::splat(puff.size * (0.3 + 0.7 * grow));
        t.translation.y += time.delta_secs() * 0.02;
        if let Some(mut m) = mats.get_mut(&puff.mat) {
            let c = m.base_color.to_srgba();
            m.base_color = Color::srgba(c.red, c.green, c.blue, 0.93 * fade * grow.max(0.2));
        }
        if puff.age >= SMOKE_DURATION {
            commands.entity(e).despawn();
        }
    }
}

fn spawn_fire(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    mats: &mut Assets<StandardMaterial>,
    id: u32,
    pos: Vec3,
) {
    let flame = mats.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.45, 0.05),
        emissive: LinearRgba::rgb(8.0, 2.5, 0.3),
        ..default()
    });
    commands
        .spawn((
            InGameEntity,
            FireArea { id },
            Lifetime(FIRE_DURATION + 0.5),
            Transform::from_translation(pos),
            Visibility::default(),
        ))
        .with_children(|p| {
            p.spawn((
                Mesh3d(meshes.add(Cylinder::new(FIRE_RADIUS, 0.03))),
                MeshMaterial3d(mats.add(StandardMaterial {
                    base_color: Color::srgb(0.15, 0.05, 0.0),
                    emissive: LinearRgba::rgb(1.5, 0.4, 0.05),
                    ..default()
                })),
                Transform::from_xyz(0.0, 0.02, 0.0),
            ));
            for i in 0..18 {
                let a = i as f32 * 2.399;
                let r = (i as f32 / 18.0).sqrt() * (FIRE_RADIUS - 0.3);
                p.spawn((
                    Flicker {
                        base: 0.25 + fastrand::f32() * 0.35,
                        seed: fastrand::f32() * 10.0,
                    },
                    Mesh3d(meshes.add(Cuboid::new(0.18, 1.0, 0.18))),
                    MeshMaterial3d(flame.clone()),
                    Transform::from_xyz(a.cos() * r, 0.2, a.sin() * r),
                ));
            }
            p.spawn((
                PointLight {
                    intensity: 300_000.0,
                    color: Color::srgb(1.0, 0.55, 0.2),
                    range: 10.0,
                    ..default()
                },
                Transform::from_xyz(0.0, 1.0, 0.0),
            ));
        });
}

fn animate_fire(time: Res<Time>, mut q: Query<(&Flicker, &mut Transform)>) {
    let t = time.elapsed_secs();
    for (f, mut tr) in &mut q {
        let h = f.base * (0.7 + 0.3 * (t * 12.0 + f.seed).sin().abs() + 0.2 * (t * 23.0 + f.seed * 2.0).sin());
        tr.scale = Vec3::new(1.0, h.max(0.05), 1.0);
        tr.translation.y = h * 0.5;
    }
}

fn spawn_flash_sphere(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    mats: &mut Assets<StandardMaterial>,
    pos: Vec3,
    color: LinearRgba,
    max: f32,
    len: f32,
) {
    let mat = mats.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.9, 0.7, 0.9),
        emissive: color,
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    commands.spawn((
        InGameEntity,
        Expand {
            age: 0.0,
            len,
            max,
            mat: mat.clone(),
        },
        Mesh3d(meshes.add(Sphere::new(1.0))),
        MeshMaterial3d(mat),
        Transform::from_translation(pos).with_scale(Vec3::splat(0.1)),
    ));
    commands.spawn((
        InGameEntity,
        Lifetime(len),
        PointLight {
            intensity: 3_000_000.0,
            color: Color::srgb(1.0, 0.85, 0.6),
            range: 25.0,
            ..default()
        },
        Transform::from_translation(pos + Vec3::Y * 0.5),
    ));
}

fn animate_expand(
    time: Res<Time>,
    mut commands: Commands,
    mut q: Query<(Entity, &mut Expand, &mut Transform)>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    for (e, mut x, mut t) in &mut q {
        x.age += time.delta_secs();
        let k = (x.age / x.len).min(1.0);
        t.scale = Vec3::splat(0.1 + x.max * k.sqrt());
        if let Some(mut m) = mats.get_mut(&x.mat) {
            m.base_color = Color::srgba(1.0, 0.9, 0.7, 0.9 * (1.0 - k));
        }
        if k >= 1.0 {
            commands.entity(e).despawn();
        }
    }
}

fn decay_screen(time: Res<Time>, mut fx: ResMut<ScreenEffects>) {
    let dt = time.delta_secs();
    fx.shake = (fx.shake - dt * 2.0).max(0.0);
    if fx.flash_hold > 0.0 {
        fx.flash_hold -= dt;
    } else {
        fx.flash_fade = (fx.flash_fade - dt).max(0.0);
    }
    fx.damage = (fx.damage - dt * 2.0).max(0.0);
    fx.hitmarker = (fx.hitmarker - dt * 3.0).max(0.0);
}

fn name_of(client: &NetClient, id: u8) -> String {
    client.player(id).map(|p| p.name.clone()).unwrap_or_else(|| "?".into())
}

#[allow(clippy::too_many_arguments)]
fn handle_events(
    mut commands: Commands,
    mut events: MessageReader<NetEvent>,
    client: Option<Res<NetClient>>,
    settings: Res<Settings>,
    me_q: Query<(&LocalPlayer, &Transform)>,
    cam_q: Query<&GlobalTransform, With<PlayerCamera>>,
    grenades: Query<(Entity, &GrenadeVisual)>,
    fires: Query<(Entity, &FireArea)>,
    cleanup: Query<Entity, Or<(With<SmokePuff>, With<SmokeCloud>, With<FireArea>, With<Impact>, With<GrenadeVisual>)>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    smokes: Res<ActiveSmokes>,
    mut fx: ResMut<ScreenEffects>,
    mut feed: ResMut<HudFeed>,
    mut sfx: MessageWriter<PlaySfx>,
) {
    let Some(c) = client else {
        events.clear();
        return;
    };
    let me = c.my_id;
    let my_pos = me_q.single().map(|(_, t)| t.translation).unwrap_or_default();
    let cam = cam_q.single().ok();
    let dist_vol = |p: Vec3| (1.0 - p.distance(my_pos) / 60.0).clamp(0.08, 1.0);

    for NetEvent(e) in events.read() {
        match e.clone() {
            GameEvent::Shot { shooter, weapon, from, to } => {
                if Some(shooter) == me {
                    continue;
                }
                let from = Vec3::from(from);
                spawn_tracer(&mut commands, from, Vec3::from(to), false);
                sfx.write(PlaySfx::fx_at(Sound::Shot(weapon.def().class), 0.8 * dist_vol(from), from));
                spawn_muzzle_flash(&mut commands, from);
            }
            GameEvent::Hit { attacker, victim, damage, headshot } => {
                feed.record_hit(me, attacker, victim, damage);
                if Some(attacker) == me && Some(victim) != me {
                    fx.hitmarker = 1.0;
                    fx.hit_head = headshot;
                    if settings.hitsound {
                        sfx.write(PlaySfx::fx(if headshot { Sound::Headshot } else { Sound::Hit }, 0.7));
                    }
                }
                if Some(victim) == me {
                    fx.damage = (fx.damage + damage as f32 / 40.0).min(1.0);
                    fx.shake = (fx.shake + 0.3).min(1.0);
                    if headshot {
                        sfx.write(PlaySfx::fx(Sound::Headshot, 0.5));
                    }
                }
            }
            GameEvent::Kill { killer, victim, source, headshot } => {
                let killer_name = name_of(&c, killer);
                let victim_name = name_of(&c, victim);
                let involved = Some(killer) == me || Some(victim) == me;
                let text = if source == DamageSource::Fall {
                    format!("{victim_name} fell to their death")
                } else if killer == victim {
                    format!("{victim_name} [{}] (suicide)", source.name())
                } else {
                    format!(
                        "{killer_name}  [{}{}]  {victim_name}",
                        source.name(),
                        if headshot { " HS" } else { "" }
                    )
                };
                feed.kill(text, involved);
                if Some(killer) == me && killer != victim {
                    sfx.write(PlaySfx::fx(Sound::Kill, 0.6));
                }
                if Some(victim) == me {
                    feed.killed_by = Some(if source == DamageSource::Fall {
                        "You fell to your death".to_string()
                    } else if killer == victim {
                        format!("You killed yourself with a {}", source.name())
                    } else {
                        format!(
                            "Killed by {killer_name} ({}{})",
                            source.name(),
                            if headshot { ", headshot" } else { "" }
                        )
                    });
                }
            }
            GameEvent::GrenadeThrown { id, kind, pos, vel, .. } => {
                let model = spawn_grenade_model(&mut commands, &mut meshes, &mut mats, kind);
                commands.entity(model).insert((
                    InGameEntity,
                    GrenadeVisual {
                        id,
                        pos: Vec3::from(pos),
                        vel: Vec3::from(vel),
                        acc: 0.0,
                    },
                    Transform::from_translation(Vec3::from(pos)),
                ));
            }
            GameEvent::GrenadeDetonate { id, kind, pos } => {
                for (e, g) in &grenades {
                    if g.id == id {
                        commands.entity(e).despawn();
                    }
                }
                let pos = Vec3::from(pos);
                let vol = dist_vol(pos);
                match kind {
                    GrenadeKind::He => {
                        spawn_flash_sphere(&mut commands, &mut meshes, &mut mats, pos, LinearRgba::rgb(20.0, 8.0, 2.0), 4.0, 0.45);
                        sfx.write(PlaySfx::fx_at(Sound::Explosion, vol, pos));
                        let d = pos.distance(my_pos);
                        if d < 15.0 {
                            fx.shake = (fx.shake + (1.0 - d / 15.0)).min(1.5);
                        }
                    }
                    GrenadeKind::Flash => {
                        spawn_flash_sphere(&mut commands, &mut meshes, &mut mats, pos, LinearRgba::rgb(40.0, 40.0, 40.0), 1.5, 0.15);
                        sfx.write(PlaySfx::fx_at(Sound::Flash, vol, pos));
                        if let (Some(cam), Ok((p, _))) = (cam, me_q.single())
                            && p.alive
                        {
                            let eye = cam.translation();
                            let to = pos - eye;
                            let d = to.length();
                            if d < 30.0
                                && line_of_sight(eye, pos + Vec3::Y * 0.05)
                                && !smokes.blocks(eye, pos + Vec3::Y * 0.05)
                            {
                                let facing = cam.forward().dot(to / d.max(0.01));
                                let look = if facing > 0.5 { 1.0 } else if facing > -0.3 { 0.5 } else { 0.2 };
                                let strength = look * (1.0 - d / 30.0).clamp(0.2, 1.0);
                                fx.flash_hold = fx.flash_hold.max(strength * 2.0);
                                fx.flash_fade = fx.flash_fade.max(strength * 3.0);
                                fx.flash_total_fade = fx.flash_fade.max(0.01);
                            }
                        }
                    }
                    GrenadeKind::Smoke => {
                        spawn_smoke(&mut commands, &mut meshes, &mut mats, pos);
                        sfx.write(PlaySfx::fx_at(Sound::Smoke, vol, pos));
                    }
                    GrenadeKind::Molotov => {
                        spawn_fire(&mut commands, &mut meshes, &mut mats, id, pos);
                        sfx.write(PlaySfx::fx_at(Sound::Fire, vol, pos));
                    }
                }
            }
            GameEvent::FireOut { id } => {
                for (e, f) in &fires {
                    if f.id == id {
                        commands.entity(e).despawn();
                    }
                }
            }
            GameEvent::Bought { player, item } => {
                if Some(player) == me {
                    sfx.write(PlaySfx::ui(Sound::Buy));
                    feed.info(format!("Bought {}", item.name()));
                }
            }
            GameEvent::BuyDenied { player, reason } => {
                if Some(player) == me {
                    sfx.write(PlaySfx::ui(Sound::Deny));
                    feed.info(reason);
                }
            }
            GameEvent::RoundStart { round } => {
                for e in &cleanup {
                    commands.entity(e).despawn();
                }
                feed.killed_by = None;
                let mode = c.snapshot.as_ref().map(|s| s.lobby.mode);
                let text = if mode.is_some_and(|m| m.is_ffa()) {
                    "All vs All - first to 25 kills".to_string()
                } else {
                    format!("Round {round}")
                };
                feed.center(text, 2.5);
                sfx.write(PlaySfx::ui(Sound::RoundStart));
            }
            GameEvent::RoundEnd { winner, reason } => {
                let head = match winner {
                    Some(t) => format!("{} wins the round", t.name()),
                    None => "Round draw".to_string(),
                };
                feed.center(format!("{head}\n{reason}"), 4.5);
                sfx.write(PlaySfx::ui(Sound::RoundEnd));
            }
            GameEvent::MatchEnd { winner } => {
                feed.center(format!("MATCH OVER\n{winner}"), 9.0);
                sfx.write(PlaySfx::ui(Sound::RoundEnd));
            }
            GameEvent::Chat { from, text, team } => {
                let my_team = c.me().map(|p| p.team);
                match team {
                    None => feed.chat(format!("{from}: {text}")),
                    Some(t) if Some(t) == my_team => feed.chat(format!("(Team) {from}: {text}")),
                    Some(_) => {}
                }
            }
            GameEvent::Info(text) => feed.info(text),
        }
    }
}

// ---------------------------------------------------------------- particles

/// Shared meshes/materials for small, frequent particles.
#[derive(Resource)]
pub struct ParticleAssets {
    casing_mesh: Handle<Mesh>,
    casing_mat: Handle<StandardMaterial>,
    blood_mesh: Handle<Mesh>,
    blood_mat: Handle<StandardMaterial>,
}

#[derive(Component)]
struct Particle {
    vel: Vec3,
    spin: f32,
}

fn make_particle_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(ParticleAssets {
        casing_mesh: meshes.add(Cuboid::new(0.012, 0.012, 0.035)),
        casing_mat: mats.add(StandardMaterial {
            base_color: Color::srgb(0.85, 0.65, 0.25),
            metallic: 0.9,
            perceptual_roughness: 0.3,
            ..default()
        }),
        blood_mesh: meshes.add(Cuboid::new(0.04, 0.04, 0.04)),
        blood_mat: mats.add(StandardMaterial {
            base_color: Color::srgb(0.55, 0.0, 0.02),
            perceptual_roughness: 0.6,
            ..default()
        }),
    });
}

/// A spent shell flying out of the gun's ejection port.
pub fn spawn_casing(commands: &mut Commands, assets: &ParticleAssets, pos: Vec3, vel: Vec3) {
    commands.spawn((
        InGameEntity,
        Particle { vel, spin: 25.0 },
        Lifetime(1.5),
        Mesh3d(assets.casing_mesh.clone()),
        MeshMaterial3d(assets.casing_mat.clone()),
        Transform::from_translation(pos),
    ));
}

fn spawn_blood(commands: &mut Commands, assets: &ParticleAssets, pos: Vec3, amount: usize) {
    for _ in 0..amount {
        let dir = Vec3::new(fastrand::f32() - 0.5, fastrand::f32() * 0.8, fastrand::f32() - 0.5);
        commands.spawn((
            InGameEntity,
            Particle { vel: dir * 3.0, spin: 0.0 },
            Lifetime(0.5 + fastrand::f32() * 0.3),
            Mesh3d(assets.blood_mesh.clone()),
            MeshMaterial3d(assets.blood_mat.clone()),
            Transform::from_translation(pos).with_scale(Vec3::splat(0.5 + fastrand::f32())),
        ));
    }
}

fn animate_particles(time: Res<Time>, mut q: Query<(&mut Particle, &mut Transform)>) {
    let dt = time.delta_secs();
    for (mut p, mut t) in &mut q {
        if p.vel == Vec3::ZERO {
            continue;
        }
        p.vel.y -= 9.8 * dt;
        let step = p.vel * dt;
        let len = step.length();
        // Stop on whatever we hit.
        if len > 0.0 && raycast_map(t.translation, step / len, len).is_some() {
            p.vel = Vec3::ZERO;
            continue;
        }
        t.translation += step;
        let spin = p.spin * dt;
        t.rotate_x(spin);
    }
}

/// Casings for other players' shots and blood where players get hit.
fn event_particles(
    mut commands: Commands,
    mut events: MessageReader<NetEvent>,
    client: Option<Res<NetClient>>,
    remotes: Query<(&RemotePlayer, &Transform)>,
    assets: Res<ParticleAssets>,
) {
    let me = client.as_ref().and_then(|c| c.my_id);
    for NetEvent(e) in events.read() {
        match e {
            GameEvent::Shot { shooter, from, .. } if Some(*shooter) != me => {
                let side = Vec3::new(fastrand::f32() - 0.5, 0.0, fastrand::f32() - 0.5).normalize_or_zero();
                spawn_casing(&mut commands, &assets, Vec3::from(*from), side * 2.0 + Vec3::Y * 1.5);
            }
            GameEvent::Hit { victim, headshot, damage, .. } if Some(*victim) != me => {
                if let Some((_, t)) = remotes.iter().find(|(rp, _)| rp.id == *victim) {
                    let height = if *headshot { 1.65 } else { 1.15 };
                    let amount = (*damage as usize / 10).clamp(3, 12);
                    spawn_blood(&mut commands, &assets, t.translation + Vec3::Y * height, amount);
                }
            }
            _ => {}
        }
    }
}
