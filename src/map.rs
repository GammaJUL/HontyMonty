//! The arena: static box geometry shared by rendering, client movement and
//! server-side hit detection, plus the small collision/raycast helpers.

use std::f32::consts::FRAC_PI_2;
use std::sync::OnceLock;

use bevy::math::Vec3;

use crate::weapons::HitGroup;

pub const PLAYER_RADIUS: f32 = 0.3;
pub const STAND_HEIGHT: f32 = 1.8;
pub const CROUCH_HEIGHT: f32 = 1.25;
pub const STAND_EYE: f32 = 1.65;
pub const CROUCH_EYE: f32 = 1.1;
pub const STEP_HEIGHT: f32 = 0.45;

#[derive(Clone, Copy, Debug)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    pub fn center(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }

    pub fn size(&self) -> Vec3 {
        self.max - self.min
    }

    pub fn overlaps(&self, o: &Aabb) -> bool {
        self.min.x < o.max.x
            && self.max.x > o.min.x
            && self.min.y < o.max.y
            && self.max.y > o.min.y
            && self.min.z < o.max.z
            && self.max.z > o.min.z
    }

    /// Entry and exit distances along a normalized ray, if it hits.
    pub fn ray_span(&self, origin: Vec3, dir: Vec3) -> Option<(f32, f32)> {
        let inv = dir.recip();
        let t1 = (self.min - origin) * inv;
        let t2 = (self.max - origin) * inv;
        let tmin = t1.min(t2).max_element();
        let tmax = t1.max(t2).min_element();
        (tmax >= tmin.max(0.0)).then_some((tmin.max(0.0), tmax))
    }

    /// Slab test. Returns the entry distance along `dir` (normalized).
    pub fn ray(&self, origin: Vec3, dir: Vec3) -> Option<f32> {
        let inv = dir.recip();
        let t1 = (self.min - origin) * inv;
        let t2 = (self.max - origin) * inv;
        let tmin = t1.min(t2).max_element();
        let tmax = t1.max(t2).min_element();
        if tmax >= tmin.max(0.0) { Some(tmin.max(0.0)) } else { None }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BlockKind {
    Floor,
    Wall,
    Crate,
    Concrete,
    Metal,
}

#[derive(Clone, Copy, Debug)]
pub struct Block {
    pub aabb: Aabb,
    pub kind: BlockKind,
}

pub struct MapData {
    pub blocks: Vec<Block>,
    pub spawns_alpha: Vec<(Vec3, f32)>,
    pub spawns_bravo: Vec<(Vec3, f32)>,
    pub spawns_ffa: Vec<(Vec3, f32)>,
}

pub fn map() -> &'static MapData {
    static MAP: OnceLock<MapData> = OnceLock::new();
    MAP.get_or_init(build_arena)
}

fn build_arena() -> MapData {
    let mut blocks = Vec::new();
    let mut add = |min: [f32; 3], max: [f32; 3], kind: BlockKind| {
        blocks.push(Block {
            aabb: Aabb::new(Vec3::from(min), Vec3::from(max)),
            kind,
        });
    };

    // Floor and outer walls.
    add([-32.0, -1.0, -22.0], [32.0, 0.0, 22.0], BlockKind::Floor);
    add([-32.0, 0.0, -22.0], [32.0, 7.0, -21.0], BlockKind::Wall);
    add([-32.0, 0.0, 21.0], [32.0, 7.0, 22.0], BlockKind::Wall);
    add([-32.0, 0.0, -22.0], [-31.0, 7.0, 22.0], BlockKind::Wall);
    add([31.0, 0.0, -22.0], [32.0, 7.0, 22.0], BlockKind::Wall);

    // Mid building and its low cover.
    add([-3.0, 0.0, -3.0], [3.0, 3.5, 3.0], BlockKind::Concrete);
    add([-1.0, 0.0, -12.0], [1.0, 1.1, -10.5], BlockKind::Crate);
    add([-1.0, 0.0, 10.5], [1.0, 1.1, 12.0], BlockKind::Crate);

    // One half, mirrored onto the other side of x = 0.
    let half: Vec<([f32; 3], [f32; 3], BlockKind)> = vec![
        // Spawn separation wall with two doorways.
        ([-13.0, 0.0, -21.0], [-12.0, 4.0, -10.0], BlockKind::Wall),
        ([-13.0, 0.0, -5.0], [-12.0, 4.0, 5.0], BlockKind::Wall),
        ([-13.0, 0.0, 10.0], [-12.0, 4.0, 21.0], BlockKind::Wall),
        // Spawn crates.
        ([-23.0, 0.0, -7.0], [-21.8, 1.2, -5.8], BlockKind::Crate),
        ([-23.0, 0.0, 5.0], [-21.8, 1.2, 6.2], BlockKind::Crate),
        ([-23.0, 1.2, 5.0], [-21.8, 2.4, 6.2], BlockKind::Crate),
        ([-18.0, 0.0, -1.0], [-16.8, 1.2, 0.2], BlockKind::Crate),
        // Sniper nest with stairs.
        ([-20.0, 0.0, 12.0], [-16.0, 2.0, 18.0], BlockKind::Concrete),
        ([-20.0, 0.0, 11.5], [-17.5, 1.6, 12.0], BlockKind::Concrete),
        ([-20.0, 0.0, 11.0], [-17.5, 1.2, 11.5], BlockKind::Concrete),
        ([-20.0, 0.0, 10.5], [-17.5, 0.8, 11.0], BlockKind::Concrete),
        ([-20.0, 0.0, 10.0], [-17.5, 0.4, 10.5], BlockKind::Concrete),
        ([-20.0, 2.0, 17.5], [-16.0, 3.0, 18.0], BlockKind::Metal),
        // Lane pillars and cover.
        ([-6.5, 0.0, -15.5], [-5.5, 4.0, -14.5], BlockKind::Concrete),
        ([-6.5, 0.0, 14.5], [-5.5, 4.0, 15.5], BlockKind::Concrete),
        ([-8.5, 0.0, -7.5], [-7.5, 1.0, -5.5], BlockKind::Metal),
        ([-8.5, 0.0, 5.5], [-7.5, 1.0, 7.5], BlockKind::Metal),
        ([-10.0, 0.0, -19.0], [-8.8, 1.2, -17.8], BlockKind::Crate),
        ([-26.0, 0.0, -17.0], [-24.0, 2.5, -15.0], BlockKind::Concrete),
        ([-4.5, 0.0, 17.0], [-3.3, 1.2, 18.2], BlockKind::Crate),
    ];
    for (min, max, kind) in half {
        add(min, max, kind);
        add([-max[0], min[1], min[2]], [-min[0], max[1], max[2]], kind);
    }

    let face_pos_x = -FRAC_PI_2;
    let face_neg_x = FRAC_PI_2;
    let spawns_alpha = [-8.0, -4.0, 0.0, 4.0, 8.0]
        .iter()
        .map(|z| (Vec3::new(-27.0, 0.0, *z), face_pos_x))
        .collect();
    let spawns_bravo = [-8.0, -4.0, 0.0, 4.0, 8.0]
        .iter()
        .map(|z| (Vec3::new(27.0, 0.0, *z), face_neg_x))
        .collect();
    let spawns_ffa = vec![
        (Vec3::new(-27.0, 0.0, 0.0), face_pos_x),
        (Vec3::new(27.0, 0.0, 0.0), face_neg_x),
        (Vec3::new(0.0, 0.0, -17.0), 0.0),
        (Vec3::new(0.0, 0.0, 17.0), std::f32::consts::PI),
        (Vec3::new(-18.0, 2.0, 15.0), face_pos_x),
        (Vec3::new(18.0, 2.0, 15.0), face_neg_x),
        (Vec3::new(-9.0, 0.0, 0.0), face_pos_x),
        (Vec3::new(9.0, 0.0, 0.0), face_neg_x),
        (Vec3::new(-25.0, 0.0, -19.0), face_pos_x),
        (Vec3::new(25.0, 0.0, 19.0), face_neg_x),
        (Vec3::new(-20.0, 0.0, -10.0), 0.0),
        (Vec3::new(20.0, 0.0, 10.0), std::f32::consts::PI),
    ];

    MapData {
        blocks,
        spawns_alpha,
        spawns_bravo,
        spawns_ffa,
    }
}

pub fn player_aabb(feet: Vec3, height: f32) -> Aabb {
    Aabb::new(
        feet - Vec3::new(PLAYER_RADIUS, 0.0, PLAYER_RADIUS),
        feet + Vec3::new(PLAYER_RADIUS, height, PLAYER_RADIUS),
    )
}

pub fn collides(feet: Vec3, height: f32) -> bool {
    let a = player_aabb(feet, height);
    map().blocks.iter().any(|b| b.aabb.overlaps(&a))
}

/// Distance to the first map hit along a normalized ray.
pub fn raycast_map(origin: Vec3, dir: Vec3, max: f32) -> Option<f32> {
    map()
        .blocks
        .iter()
        .filter_map(|b| b.aabb.ray(origin, dir))
        .filter(|t| *t <= max)
        .min_by(|a, b| a.total_cmp(b))
}

impl BlockKind {
    /// How hard this material is to shoot through, per meter.
    pub fn penetration_cost(self) -> f32 {
        match self {
            BlockKind::Crate => 1.0,
            BlockKind::Metal => 2.5,
            BlockKind::Concrete | BlockKind::Wall | BlockKind::Floor => 4.0,
        }
    }
}

/// Total penetration cost of everything between `origin` and `origin + dir * max_t`.
pub fn penetration_cost(origin: Vec3, dir: Vec3, max_t: f32) -> f32 {
    map()
        .blocks
        .iter()
        .filter_map(|b| {
            let (enter, exit) = b.aabb.ray_span(origin, dir)?;
            let thickness = exit.min(max_t) - enter;
            (enter < max_t && thickness > 0.0).then(|| thickness * b.kind.penetration_cost())
        })
        .sum()
}

pub fn line_of_sight(a: Vec3, b: Vec3) -> bool {
    let d = b - a;
    let len = d.length();
    if len < 1e-4 {
        return true;
    }
    raycast_map(a, d / len, len - 0.05).is_none()
}

pub fn hitboxes(feet: Vec3, crouch: bool) -> [(HitGroup, Aabb); 4] {
    let h = if crouch { CROUCH_HEIGHT } else { STAND_HEIGHT };
    let b = |w: f32, y0: f32, y1: f32| {
        Aabb::new(
            feet + Vec3::new(-w, y0, -w),
            feet + Vec3::new(w, y1, w),
        )
    };
    [
        (HitGroup::Head, b(0.15, h - 0.32, h)),
        (HitGroup::Chest, b(0.27, h * 0.56, h - 0.32)),
        (HitGroup::Stomach, b(0.25, h * 0.42, h * 0.56)),
        (HitGroup::Legs, b(0.22, 0.0, h * 0.42)),
    ]
}

/// Closest hitbox hit along a ray (distance, group).
pub fn ray_player(origin: Vec3, dir: Vec3, feet: Vec3, crouch: bool) -> Option<(f32, HitGroup)> {
    hitboxes(feet, crouch)
        .iter()
        .filter_map(|(g, a)| a.ray(origin, dir).map(|t| (t, *g)))
        .min_by(|a, b| a.0.total_cmp(&b.0))
}

pub struct MoveResult {
    pub pos: Vec3,
    pub vel: Vec3,
    pub on_ground: bool,
}

/// Kinematic move-and-slide against the map with stair stepping.
/// `others` are extra solid boxes (other players). Ones we already overlap are ignored so
/// nobody gets stuck inside each other.
pub fn move_player(
    mut pos: Vec3,
    mut vel: Vec3,
    height: f32,
    dt: f32,
    was_on_ground: bool,
    others: &[Aabb],
) -> MoveResult {
    let start = player_aabb(pos, height);
    let others: Vec<Aabb> = others.iter().filter(|o| !o.overlaps(&start)).copied().collect();
    let blocked = |feet: Vec3| {
        collides(feet, height) || {
            let a = player_aabb(feet, height);
            others.iter().any(|o| o.overlaps(&a))
        }
    };
    let steps = ((dt / (1.0 / 120.0)).ceil() as usize).clamp(1, 16);
    let sdt = dt / steps as f32;
    let mut on_ground = false;
    for _ in 0..steps {
        for axis in [0usize, 2] {
            let delta = vel[axis] * sdt;
            if delta == 0.0 {
                continue;
            }
            let mut next = pos;
            next[axis] += delta;
            if !blocked(next) {
                pos = next;
                continue;
            }
            let mut stepped = next;
            stepped.y += STEP_HEIGHT;
            if (was_on_ground || on_ground) && !blocked(stepped) {
                // Settle down onto the step.
                let mut settle = stepped;
                for _ in 0..10 {
                    let mut lower = settle;
                    lower.y -= STEP_HEIGHT / 10.0;
                    if blocked(lower) {
                        break;
                    }
                    settle = lower;
                }
                pos = settle;
            } else {
                vel[axis] = 0.0;
            }
        }
        let mut next = pos;
        next.y += vel.y * sdt;
        if blocked(next) {
            if vel.y < 0.0 {
                on_ground = true;
                // Snap to the top of whatever we landed on.
                let a = player_aabb(next, height);
                let top = map()
                    .blocks
                    .iter()
                    .filter(|b| b.aabb.overlaps(&a))
                    .map(|b| b.aabb.max.y)
                    .chain(others.iter().filter(|o| o.overlaps(&a)).map(|o| o.max.y))
                    .fold(f32::MIN, f32::max);
                if top <= pos.y + 0.001 {
                    pos.y = top;
                }
            }
            vel.y = 0.0;
        } else {
            pos = next;
        }
    }
    if !on_ground {
        on_ground = blocked(pos - Vec3::Y * 0.02);
    }
    MoveResult { pos, vel, on_ground }
}

pub const GRENADE_GRAVITY: f32 = 15.0;
pub const GRENADE_RADIUS: f32 = 0.08;

/// Steps a grenade. Returns true if it bounced off the ground this step.
pub fn step_grenade(pos: &mut Vec3, vel: &mut Vec3, dt: f32) -> bool {
    let mut hit_ground = false;
    vel.y -= GRENADE_GRAVITY * dt;
    let r = Vec3::splat(GRENADE_RADIUS);
    for axis in 0..3 {
        let mut next = *pos;
        next[axis] += vel[axis] * dt;
        let a = Aabb::new(next - r, next + r);
        if map().blocks.iter().any(|b| b.aabb.overlaps(&a)) {
            if axis == 1 && vel.y < 0.0 {
                hit_ground = true;
                // Ground friction.
                vel.x *= 0.6;
                vel.z *= 0.6;
            }
            vel[axis] *= -0.45;
        } else {
            *pos = next;
        }
    }
    hit_ground
}
