//! Simple server-side bots: they buy, wander between open spots on the map,
//! react to enemies they can see (not through walls or smoke), aim with a
//! little human-like error and shoot.

use std::f32::consts::FRAC_PI_2;
use std::sync::OnceLock;

use super::*;
use crate::map::{collides, move_player, STAND_EYE, STAND_HEIGHT};
use crate::profile::{Accessory, FacialHair, Gender, HairStyle, CLOTHING_COLORS, EYE_COLORS, HAIR_COLORS, SKIN_TONES};

const NAMES: [&str; 12] = [
    "Ace", "Blaze", "Cobra", "Dash", "Echo", "Frost", "Ghost", "Hawk", "Ivy", "Jinx", "Kilo", "Nova",
];
/// Turn speed while just walking around (not aiming).
const WALK_TURN_SPEED: f32 = 7.0;

pub(super) struct Brain {
    vel: Vec3,
    on_ground: bool,
    goal: Option<Vec3>,
    /// Position and time of the last progress check (to notice being stuck).
    progress: (Vec3, f32),
    target: Option<PlayerId>,
    seen_since: f32,
    aim_height: f32,
    next_shot: f32,
    /// Per-bot variation (around 1.0) applied to reaction time and aim error.
    variance: f32,
}

impl Brain {
    fn new() -> Self {
        Self {
            vel: Vec3::ZERO,
            on_ground: false,
            goal: None,
            progress: (Vec3::ZERO, 0.0),
            target: None,
            seen_since: 0.0,
            aim_height: 1.2,
            next_shot: 0.0,
            variance: 0.85 + fastrand::f32() * 0.3,
        }
    }
}

/// Open floor spots bots walk between.
fn waypoints() -> &'static [Vec3] {
    static POINTS: OnceLock<Vec<Vec3>> = OnceLock::new();
    POINTS.get_or_init(|| {
        let mut v = Vec::new();
        for x in (-29..=29).step_by(3) {
            for z in (-19..=19).step_by(3) {
                let p = Vec3::new(x as f32, 0.0, z as f32);
                // Leave a little room around each spot.
                let roomy = [Vec3::ZERO, Vec3::X * 0.5, -Vec3::X * 0.5, Vec3::Z * 0.5, -Vec3::Z * 0.5]
                    .iter()
                    .all(|o| !collides(p + *o, STAND_HEIGHT));
                if roomy {
                    v.push(p);
                }
            }
        }
        v
    })
}

fn random_look() -> Appearance {
    let pick = |n: usize| fastrand::usize(..n);
    Appearance {
        gender: if fastrand::bool() { Gender::Male } else { Gender::Female },
        skin_tone: pick(SKIN_TONES.len()),
        hair_style: HairStyle::ALL[pick(HairStyle::ALL.len())],
        hair_color: pick(HAIR_COLORS.len()),
        eye_color: pick(EYE_COLORS.len()),
        shirt_color: pick(CLOTHING_COLORS.len()),
        pants_color: pick(CLOTHING_COLORS.len()),
        height: 0.95 + fastrand::f32() * 0.1,
        facial_hair: FacialHair::ALL[pick(FacialHair::ALL.len())],
        accessory: Accessory::ALL[pick(Accessory::ALL.len())],
    }
}

/// Yaw/pitch that look along `dir` (matches the client: yaw 0 faces -Z).
fn look_angles(dir: Vec3) -> (f32, f32) {
    (f32::atan2(-dir.x, -dir.z), dir.y.clamp(-1.0, 1.0).asin())
}

fn turn_towards(current: f32, target: f32, max_step: f32) -> f32 {
    let mut diff = (target - current) % std::f32::consts::TAU;
    if diff > std::f32::consts::PI {
        diff -= std::f32::consts::TAU;
    } else if diff < -std::f32::consts::PI {
        diff += std::f32::consts::TAU;
    }
    current + diff.clamp(-max_step, max_step)
}

impl GameServer {
    pub(super) fn add_bot(&mut self) {
        if self.players.len() >= self.max_players() {
            self.event(GameEvent::Info("Lobby is full".into()));
            return;
        }
        let id = self.next_player_id;
        self.next_player_id = self.next_player_id.wrapping_add(1).max(1);
        let base = NAMES[fastrand::usize(..NAMES.len())];
        let mut name = format!("BOT {base}");
        let mut n = 2;
        while self.players.iter().any(|p| p.name == name) {
            name = format!("BOT {base} {n}");
            n += 1;
        }
        let team = self.auto_team();
        let money = if self.mode.is_ffa() { economy::FFA_START_MONEY } else { economy::START_MONEY };
        let mut p = ServerPlayer {
            id,
            addr: SocketAddr::from(([0, 0, 0, 0], 0)),
            name: name.clone(),
            appearance: random_look(),
            skins: Vec::new(),
            team,
            last_seen: self.time,
            last_cmd: 0,
            state: ClientState::default(),
            alive: false,
            health: 100,
            armor: 0,
            helmet: false,
            money,
            kills: 0,
            deaths: 0,
            headshots: 0,
            damage: 0,
            primary: None,
            secondary: None,
            grenades: Vec::new(),
            spawn_seq: 0,
            spawn_pos: Vec3::ZERO,
            spawn_yaw: 0.0,
            respawn_at: None,
            received_at: self.time,
            bot: Some(Brain::new()),
        };
        p.reset_loadout();
        if self.mode.is_ffa() && self.phase == Phase::Live {
            p.respawn_at = Some(self.time + 1.0);
        }
        self.players.push(p);
        self.event(GameEvent::Info(format!("{name} joined the lobby")));
    }

    pub(super) fn remove_bot(&mut self) {
        if let Some(i) = self.players.iter().rposition(|p| p.bot.is_some()) {
            let p = self.players.remove(i);
            self.event(GameEvent::Info(format!("{} left the lobby", p.name)));
            self.check_round_end();
        }
    }

    /// Bots spend their money at the start of a round (or after respawning).
    pub(super) fn bots_buy(&mut self) {
        for i in 0..self.players.len() {
            if self.players[i].bot.is_none() || !self.can_buy(i) || self.players[i].primary.is_some() {
                continue;
            }
            let money = self.players[i].money;
            let rifle = if self.players[i].team == Team::Bravo { WeaponId::M4a4 } else { WeaponId::Ak47 };
            if money >= rifle.def().price + 650 {
                self.buy(i, ShopItem::Weapon(rifle));
            } else if money >= WeaponId::Mp9.def().price + 650 {
                self.buy(i, ShopItem::Weapon(WeaponId::Mp9));
            }
            let money = self.players[i].money;
            if money >= 1000 {
                self.buy(i, ShopItem::KevlarHelmet);
            } else if money >= 650 {
                self.buy(i, ShopItem::Kevlar);
            }
        }
    }

    pub(super) fn update_bots(&mut self, dt: f32) {
        let now = self.time;
        let mut any = false;
        for i in 0..self.players.len() {
            if self.players[i].bot.is_none() {
                continue;
            }
            any = true;
            self.players[i].last_seen = now;
            if self.players[i].alive {
                self.think(i, dt);
            }
        }
        if any && self.mode.is_ffa() {
            self.bots_buy();
        }
    }

    fn smoke_between(&self, a: Vec3, b: Vec3) -> bool {
        let now = self.time;
        self.smokes.iter().any(|(pos, end)| {
            if *end <= now {
                return false;
            }
            let center = *pos + Vec3::Y * 1.6;
            let ab = b - a;
            let t = ((center - a).dot(ab) / ab.length_squared().max(1e-4)).clamp(0.0, 1.0);
            (a + ab * t).distance(center) < SMOKE_RADIUS * 0.85
        })
    }

    /// Closest enemy this bot can actually see.
    fn visible_enemy(&self, i: usize, eye: Vec3, view_range: f32) -> Option<(usize, f32)> {
        let mut best: Option<(usize, f32)> = None;
        for (j, o) in self.players.iter().enumerate() {
            if !o.alive || !self.enemies(i, j) {
                continue;
            }
            let chest = o.feet() + Vec3::Y * 1.2;
            let d = eye.distance(chest);
            if d > view_range || !line_of_sight(eye, chest) || self.smoke_between(eye, chest) {
                continue;
            }
            if best.is_none_or(|b| d < b.1) {
                best = Some((j, d));
            }
        }
        best
    }

    fn think(&mut self, i: usize, dt: f32) {
        let now = self.time;
        let can_act = matches!(self.phase, Phase::Live | Phase::RoundEnd);
        let feet = self.players[i].feet();
        let eye = feet + Vec3::Y * STAND_EYE;
        let weapon = {
            let p = &self.players[i];
            p.primary.or(p.secondary).unwrap_or(WeaponId::Knife)
        };
        let def = weapon.def();
        let skill = self.bot_difficulty.skill();
        let seen = self.visible_enemy(i, eye, skill.view_range);

        let Some(mut brain) = self.players[i].bot.take() else { return };
        let (mut yaw, mut pitch) = (self.players[i].state.yaw, self.players[i].state.pitch);
        let mut wish = Vec3::ZERO;
        let mut fire_dir = None;

        match seen {
            Some((j, dist)) => {
                let target = &self.players[j];
                if brain.target != Some(target.id) {
                    brain.target = Some(target.id);
                    brain.seen_since = now;
                    // Most bots go for the body, some for the head.
                    brain.aim_height = if fastrand::f32() < skill.headshot_chance { 1.62 } else { 1.2 };
                }
                let aim_point = target.feet() + Vec3::Y * brain.aim_height;
                let (want_yaw, want_pitch) = look_angles((aim_point - eye).normalize_or_zero());
                let step = skill.turn_speed * dt;
                yaw = turn_towards(yaw, want_yaw, step);
                pitch = turn_towards(pitch, want_pitch, step);
                let aim_error = (want_yaw - yaw).abs().min(1.0) + (want_pitch - pitch).abs();
                let reaction = skill.reaction * brain.variance;
                let in_range = weapon != WeaponId::Knife || dist < 1.7;
                if can_act && in_range && now - brain.seen_since > reaction && aim_error < 0.06 && now >= brain.next_shot {
                    let facing = Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0) * Vec3::NEG_Z;
                    fire_dir = Some(facing);
                    let burst_pause = skill.shot_pause * (0.5 + fastrand::f32());
                    brain.next_shot = now + def.fire_interval + burst_pause;
                }
                // Stop to shoot like CS bots; rush in with a knife.
                if weapon == WeaponId::Knife {
                    wish = Vec3::new(aim_point.x - feet.x, 0.0, aim_point.z - feet.z).normalize_or_zero();
                }
            }
            None => {
                brain.target = None;
                let goal = match brain.goal {
                    Some(g) if g.distance(Vec3::new(feet.x, g.y, feet.z)) > 1.5 => g,
                    _ => {
                        // Pick a new spot we can walk to in a straight line.
                        let points = waypoints();
                        let candidates: Vec<Vec3> = points
                            .iter()
                            .copied()
                            .filter(|p| {
                                let d = p.distance(feet);
                                (5.0..30.0).contains(&d) && line_of_sight(feet + Vec3::Y * 0.5, *p + Vec3::Y * 0.5)
                            })
                            .collect();
                        let pool = if candidates.is_empty() { points.to_vec() } else { candidates };
                        let g = pool[fastrand::usize(..pool.len())];
                        brain.goal = Some(g);
                        g
                    }
                };
                wish = Vec3::new(goal.x - feet.x, 0.0, goal.z - feet.z).normalize_or_zero();
                if wish != Vec3::ZERO {
                    let (want_yaw, _) = look_angles(wish);
                    yaw = turn_towards(yaw, want_yaw, WALK_TURN_SPEED * dt);
                }
                pitch = turn_towards(pitch, 0.0, WALK_TURN_SPEED * dt);
            }
        }
        if !can_act {
            wish = Vec3::ZERO;
        }

        // Physics, same as players.
        let speed = def.move_speed * 0.9;
        let horiz = Vec3::new(brain.vel.x, 0.0, brain.vel.z).lerp(wish * speed, 1.0 - (-10.0 * dt).exp());
        brain.vel.x = horiz.x;
        brain.vel.z = horiz.z;
        brain.vel.y -= 20.0 * dt;
        let moved = move_player(feet, brain.vel, STAND_HEIGHT, dt, brain.on_ground, &[]);
        brain.vel = moved.vel;
        brain.on_ground = moved.on_ground;
        if brain.on_ground && brain.vel.y < 0.0 {
            brain.vel.y = 0.0;
        }
        // Stuck against something? Pick another goal.
        if now - brain.progress.1 > 1.5 {
            if wish != Vec3::ZERO && moved.pos.distance(brain.progress.0) < 0.5 {
                brain.goal = None;
            }
            brain.progress = (moved.pos, now);
        }

        let p = &mut self.players[i];
        p.state.pos = moved.pos.to_array();
        p.state.yaw = yaw;
        p.state.pitch = pitch.clamp(-FRAC_PI_2, FRAC_PI_2);
        p.state.crouch = false;
        p.state.weapon = Some(weapon);
        p.state.grenade = None;
        p.state.moving = Vec2::new(brain.vel.x, brain.vel.z).length() > 3.4;
        p.bot = Some(brain);

        if let Some(dir) = fire_dir {
            // Bots spray a little worse than they aim.
            let variance = self.players[i].bot.as_ref().map(|b| b.variance).unwrap_or(1.0);
            let error = def.base_spread + skill.aim_error * variance;
            let dirs: Vec<V3> = (0..def.pellets.max(1))
                .map(|_| {
                    let jitter = Vec3::new(fastrand::f32() - 0.5, fastrand::f32() - 0.5, fastrand::f32() - 0.5) * 2.0 * error;
                    (dir + jitter).normalize().to_array()
                })
                .collect();
            self.fire(i, weapon, eye, dirs, false);
        }
    }
}
