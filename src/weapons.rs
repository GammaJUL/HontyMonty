//! Weapon, grenade and equipment definitions plus the CS2-style damage model.
//! Everything in here is plain data so it can be shared by client and server.

use serde::{Deserialize, Serialize};

/// Distances in this game are in meters; CS uses inches ("units").
pub const UNITS_PER_METER: f32 = 39.37;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum WeaponId {
    Knife,
    Glock,
    Usp,
    Deagle,
    Mp9,
    P90,
    Nova,
    Ak47,
    M4a4,
    Aug,
    Sg553,
    Ssg08,
    Awp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WeaponClass {
    Knife,
    Pistol,
    Smg,
    Shotgun,
    Rifle,
    Sniper,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Primary,
    Secondary,
    Melee,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ScopeStyle {
    None,
    /// AUG / SG 553 style: light zoom, see-through overlay.
    Rifle,
    /// AWP / SSG 08 style: full black scope overlay.
    Sniper,
}

pub struct WeaponDef {
    pub id: WeaponId,
    pub name: &'static str,
    pub class: WeaponClass,
    pub slot: Slot,
    pub price: u32,
    pub kill_reward: u32,
    pub damage: f32,
    pub armor_pen: f32,
    /// CS2 range modifier: damage *= range_modifier ^ (distance_units / 500).
    pub range_modifier: f32,
    /// Maximum range in meters.
    pub range: f32,
    pub pellets: u32,
    /// Seconds between shots.
    pub fire_interval: f32,
    pub automatic: bool,
    pub mag: u32,
    pub reserve: u32,
    pub reload_time: f32,
    /// Movement speed in m/s while holding this weapon.
    pub move_speed: f32,
    /// Spread values are cone half-angles in radians.
    pub base_spread: f32,
    pub move_spread: f32,
    pub jump_spread: f32,
    /// Extra spread added per consecutive shot (spray inaccuracy).
    pub recoil_spread: f32,
    /// Upward view kick per shot in radians.
    pub recoil_kick: f32,
    pub scope: ScopeStyle,
    /// Field of view (degrees) per zoom level.
    pub zoom_fovs: &'static [f32],
    /// Spread used while scoped (replaces base_spread).
    pub scoped_spread: f32,
    /// Wallbang power, compared against `map::penetration_cost` (0 = can't shoot through anything).
    pub penetration: f32,
}

impl WeaponId {
    pub fn def(self) -> &'static WeaponDef {
        WEAPONS.iter().find(|w| w.id == self).expect("every weapon has a definition")
    }
}

const fn base() -> WeaponDef {
    WeaponDef {
        id: WeaponId::Knife,
        name: "",
        class: WeaponClass::Pistol,
        slot: Slot::Secondary,
        price: 0,
        kill_reward: 300,
        damage: 30.0,
        armor_pen: 0.5,
        range_modifier: 0.9,
        range: 200.0,
        pellets: 1,
        fire_interval: 0.15,
        automatic: false,
        mag: 20,
        reserve: 120,
        reload_time: 2.2,
        move_speed: 6.1,
        base_spread: 0.006,
        move_spread: 0.04,
        jump_spread: 0.15,
        recoil_spread: 0.002,
        recoil_kick: 0.012,
        scope: ScopeStyle::None,
        zoom_fovs: &[],
        scoped_spread: 0.0,
        penetration: 1.0,
    }
}

pub static WEAPONS: [WeaponDef; 13] = [
    WeaponDef {
        id: WeaponId::Knife,
        name: "Knife",
        class: WeaponClass::Knife,
        slot: Slot::Melee,
        kill_reward: 1500,
        damage: 40.0,
        armor_pen: 0.85,
        range_modifier: 1.0,
        range: 1.9,
        fire_interval: 0.45,
        automatic: true,
        mag: 0,
        reserve: 0,
        move_speed: 6.4,
        base_spread: 0.0,
        move_spread: 0.0,
        jump_spread: 0.0,
        recoil_spread: 0.0,
        recoil_kick: 0.0,
        penetration: 0.0,
        ..base()
    },
    WeaponDef {
        id: WeaponId::Glock,
        name: "Glock-18",
        price: 200,
        damage: 30.0,
        armor_pen: 0.47,
        range_modifier: 0.85,
        fire_interval: 0.15,
        mag: 20,
        reserve: 120,
        reload_time: 2.2,
        ..base()
    },
    WeaponDef {
        id: WeaponId::Usp,
        name: "USP-S",
        price: 200,
        damage: 35.0,
        armor_pen: 0.505,
        range_modifier: 0.91,
        fire_interval: 0.17,
        mag: 12,
        reserve: 24,
        reload_time: 2.2,
        base_spread: 0.004,
        ..base()
    },
    WeaponDef {
        id: WeaponId::Deagle,
        name: "Desert Eagle",
        price: 700,
        damage: 53.0,
        armor_pen: 0.93,
        range_modifier: 0.81,
        fire_interval: 0.225,
        mag: 7,
        reserve: 35,
        reload_time: 2.2,
        move_speed: 5.9,
        base_spread: 0.008,
        move_spread: 0.07,
        recoil_spread: 0.01,
        recoil_kick: 0.04,
        penetration: 2.0,
        ..base()
    },
    WeaponDef {
        id: WeaponId::Mp9,
        name: "MP9",
        class: WeaponClass::Smg,
        slot: Slot::Primary,
        price: 1250,
        kill_reward: 600,
        damage: 26.0,
        armor_pen: 0.6,
        range_modifier: 0.87,
        fire_interval: 0.07,
        automatic: true,
        mag: 30,
        reserve: 120,
        reload_time: 2.1,
        move_speed: 6.1,
        base_spread: 0.008,
        move_spread: 0.025,
        jump_spread: 0.12,
        recoil_spread: 0.0025,
        recoil_kick: 0.006,
        ..base()
    },
    WeaponDef {
        id: WeaponId::P90,
        name: "P90",
        class: WeaponClass::Smg,
        slot: Slot::Primary,
        price: 2350,
        kill_reward: 300,
        damage: 26.0,
        armor_pen: 0.69,
        range_modifier: 0.86,
        fire_interval: 0.07,
        automatic: true,
        mag: 50,
        reserve: 100,
        reload_time: 3.3,
        move_speed: 5.9,
        base_spread: 0.009,
        move_spread: 0.025,
        jump_spread: 0.12,
        recoil_spread: 0.002,
        recoil_kick: 0.005,
        ..base()
    },
    WeaponDef {
        id: WeaponId::Nova,
        name: "Nova",
        class: WeaponClass::Shotgun,
        slot: Slot::Primary,
        price: 1050,
        kill_reward: 900,
        damage: 26.0,
        armor_pen: 0.5,
        range_modifier: 0.7,
        range: 40.0,
        pellets: 9,
        fire_interval: 0.88,
        mag: 8,
        reserve: 32,
        reload_time: 3.5,
        move_speed: 5.7,
        base_spread: 0.06,
        move_spread: 0.02,
        jump_spread: 0.08,
        recoil_spread: 0.0,
        recoil_kick: 0.05,
        penetration: 0.5,
        ..base()
    },
    WeaponDef {
        id: WeaponId::Ak47,
        name: "AK-47",
        class: WeaponClass::Rifle,
        slot: Slot::Primary,
        price: 2700,
        damage: 36.0,
        armor_pen: 0.775,
        range_modifier: 0.98,
        fire_interval: 0.1,
        automatic: true,
        mag: 30,
        reserve: 90,
        reload_time: 2.4,
        move_speed: 5.5,
        base_spread: 0.004,
        move_spread: 0.09,
        jump_spread: 0.25,
        recoil_spread: 0.0045,
        recoil_kick: 0.011,
        penetration: 2.0,
        ..base()
    },
    WeaponDef {
        id: WeaponId::M4a4,
        name: "M4A4",
        class: WeaponClass::Rifle,
        slot: Slot::Primary,
        price: 3100,
        damage: 33.0,
        armor_pen: 0.7,
        range_modifier: 0.97,
        fire_interval: 0.09,
        automatic: true,
        mag: 30,
        reserve: 90,
        reload_time: 3.0,
        move_speed: 5.6,
        base_spread: 0.0035,
        move_spread: 0.085,
        jump_spread: 0.25,
        recoil_spread: 0.0038,
        recoil_kick: 0.009,
        penetration: 2.0,
        ..base()
    },
    WeaponDef {
        id: WeaponId::Aug,
        name: "AUG",
        class: WeaponClass::Rifle,
        slot: Slot::Primary,
        price: 3300,
        damage: 28.0,
        armor_pen: 0.9,
        range_modifier: 0.98,
        fire_interval: 0.09,
        automatic: true,
        mag: 30,
        reserve: 90,
        reload_time: 3.3,
        move_speed: 5.5,
        base_spread: 0.004,
        move_spread: 0.08,
        jump_spread: 0.25,
        recoil_spread: 0.0035,
        recoil_kick: 0.008,
        scope: ScopeStyle::Rifle,
        zoom_fovs: &[45.0],
        scoped_spread: 0.0015,
        penetration: 2.0,
        ..base()
    },
    WeaponDef {
        id: WeaponId::Sg553,
        name: "SG 553",
        class: WeaponClass::Rifle,
        slot: Slot::Primary,
        price: 3000,
        damage: 30.0,
        armor_pen: 1.0,
        range_modifier: 0.98,
        fire_interval: 0.09,
        automatic: true,
        mag: 30,
        reserve: 90,
        reload_time: 2.8,
        move_speed: 5.4,
        base_spread: 0.004,
        move_spread: 0.085,
        jump_spread: 0.25,
        recoil_spread: 0.004,
        recoil_kick: 0.009,
        scope: ScopeStyle::Rifle,
        zoom_fovs: &[45.0],
        scoped_spread: 0.0015,
        penetration: 2.0,
        ..base()
    },
    WeaponDef {
        id: WeaponId::Ssg08,
        name: "SSG 08",
        class: WeaponClass::Sniper,
        slot: Slot::Primary,
        price: 1700,
        damage: 88.0,
        armor_pen: 0.85,
        range_modifier: 0.98,
        fire_interval: 1.25,
        mag: 10,
        reserve: 90,
        reload_time: 3.7,
        move_speed: 5.9,
        base_spread: 0.05,
        move_spread: 0.05,
        jump_spread: 0.03,
        recoil_spread: 0.0,
        recoil_kick: 0.03,
        scope: ScopeStyle::Sniper,
        zoom_fovs: &[40.0, 15.0],
        scoped_spread: 0.0008,
        penetration: 2.2,
        ..base()
    },
    WeaponDef {
        id: WeaponId::Awp,
        name: "AWP",
        class: WeaponClass::Sniper,
        slot: Slot::Primary,
        price: 4750,
        kill_reward: 100,
        damage: 115.0,
        armor_pen: 0.975,
        range_modifier: 0.99,
        fire_interval: 1.46,
        mag: 5,
        reserve: 30,
        reload_time: 3.6,
        move_speed: 5.1,
        base_spread: 0.08,
        move_spread: 0.15,
        jump_spread: 0.3,
        recoil_spread: 0.0,
        recoil_kick: 0.05,
        scope: ScopeStyle::Sniper,
        zoom_fovs: &[40.0, 10.0],
        scoped_spread: 0.0005,
        penetration: 2.8,
        ..base()
    },
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GrenadeKind {
    He,
    Flash,
    Smoke,
    Molotov,
}

impl GrenadeKind {
    pub fn name(self) -> &'static str {
        match self {
            GrenadeKind::He => "HE Grenade",
            GrenadeKind::Flash => "Flashbang",
            GrenadeKind::Smoke => "Smoke Grenade",
            GrenadeKind::Molotov => "Molotov",
        }
    }

    pub fn price(self) -> u32 {
        match self {
            GrenadeKind::He => 300,
            GrenadeKind::Flash => 200,
            GrenadeKind::Smoke => 300,
            GrenadeKind::Molotov => 400,
        }
    }

    pub fn max_carry(self) -> usize {
        match self {
            GrenadeKind::Flash => 2,
            _ => 1,
        }
    }

    /// Seconds until detonation (molotovs also burst on ground contact).
    pub fn fuse(self) -> f32 {
        match self {
            GrenadeKind::He | GrenadeKind::Flash => 1.6,
            GrenadeKind::Smoke => 2.0,
            GrenadeKind::Molotov => 2.2,
        }
    }

    pub fn color(self) -> [f32; 3] {
        match self {
            GrenadeKind::He => [0.2, 0.35, 0.15],
            GrenadeKind::Flash => [0.8, 0.8, 0.85],
            GrenadeKind::Smoke => [0.35, 0.4, 0.45],
            GrenadeKind::Molotov => [0.7, 0.35, 0.1],
        }
    }
}

/// View kick for the `shot`-th bullet of a spray as (yaw, pitch) multipliers of
/// `recoil_kick`. Positive yaw drifts the view left.
pub fn spray_step(w: WeaponId, shot: u32) -> (f32, f32) {
    let wobble = |n: u32, amount: f32| if (n / 4) % 2 == 0 { amount } else { -amount };
    match w {
        // Straight up, then hard left, then hard right.
        WeaponId::Ak47 => match shot {
            0..=2 => (0.0, 0.6),
            3..=8 => (if shot % 2 == 0 { 0.08 } else { -0.08 }, 1.3),
            9..=14 => (0.95, 0.25),
            15..=20 => (-1.15, 0.1),
            21..=25 => (0.8, 0.05),
            _ => (wobble(shot, 0.6), 0.05),
        },
        // Gentler: up, slightly right, then left.
        WeaponId::M4a4 | WeaponId::Aug | WeaponId::Sg553 => match shot {
            0..=2 => (0.0, 0.5),
            3..=9 => (-0.1, 1.0),
            10..=15 => (-0.65, 0.2),
            16..=21 => (0.75, 0.1),
            _ => (wobble(shot, 0.45), 0.05),
        },
        WeaponId::Mp9 | WeaponId::P90 => match shot {
            0..=3 => (0.0, 0.7),
            4..=11 => (wobble(shot * 2, 0.3), 0.8),
            _ => (wobble(shot, 0.5), 0.15),
        },
        _ => (if shot % 2 == 0 { 0.15 } else { -0.15 }, 1.0),
    }
}

/// Knife damage: left-click slash or right-click stab, doubled-ish from behind like CS2.
pub fn knife_damage(heavy: bool, backstab: bool) -> f32 {
    match (heavy, backstab) {
        (false, false) => 40.0,
        (false, true) => 90.0,
        (true, false) => 65.0,
        (true, true) => 180.0,
    }
}

pub fn knife_range(heavy: bool) -> f32 {
    if heavy { 1.6 } else { 1.9 }
}

pub const KNIFE_HEAVY_INTERVAL: f32 = 1.0;

/// Landing faster than this (m/s) hurts.
pub const FALL_SAFE_SPEED: f32 = 11.0;
pub const FALL_DAMAGE_PER_MS: f32 = 11.0;

pub fn fall_damage(impact_speed: f32) -> i32 {
    ((impact_speed - FALL_SAFE_SPEED).max(0.0) * FALL_DAMAGE_PER_MS).round() as i32
}

pub const MAX_GRENADES: usize = 4;
pub const HE_DAMAGE: f32 = 98.0;
pub const HE_RADIUS: f32 = 9.0;
pub const SMOKE_RADIUS: f32 = 3.6;
pub const SMOKE_DURATION: f32 = 18.0;
pub const FIRE_RADIUS: f32 = 2.6;
pub const FIRE_DURATION: f32 = 7.0;
pub const FIRE_DPS: f32 = 40.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShopItem {
    Weapon(WeaponId),
    Grenade(GrenadeKind),
    Kevlar,
    KevlarHelmet,
}

impl ShopItem {
    pub fn name(self) -> &'static str {
        match self {
            ShopItem::Weapon(w) => w.def().name,
            ShopItem::Grenade(g) => g.name(),
            ShopItem::Kevlar => "Kevlar Vest",
            ShopItem::KevlarHelmet => "Kevlar + Helmet",
        }
    }

    /// Base price (helmet-only upgrade discount is handled by the server).
    pub fn price(self) -> u32 {
        match self {
            ShopItem::Weapon(w) => w.def().price,
            ShopItem::Grenade(g) => g.price(),
            ShopItem::Kevlar => 650,
            ShopItem::KevlarHelmet => 1000,
        }
    }
}

/// Buy menu layout: (category name, items).
pub fn shop_categories() -> Vec<(&'static str, Vec<ShopItem>)> {
    use ShopItem::*;
    vec![
        (
            "Pistols",
            vec![
                Weapon(WeaponId::Glock),
                Weapon(WeaponId::Usp),
                Weapon(WeaponId::Deagle),
            ],
        ),
        (
            "SMGs & Heavy",
            vec![
                Weapon(WeaponId::Mp9),
                Weapon(WeaponId::P90),
                Weapon(WeaponId::Nova),
            ],
        ),
        (
            "Rifles",
            vec![
                Weapon(WeaponId::Ak47),
                Weapon(WeaponId::M4a4),
                Weapon(WeaponId::Aug),
                Weapon(WeaponId::Sg553),
                Weapon(WeaponId::Ssg08),
                Weapon(WeaponId::Awp),
            ],
        ),
        (
            "Grenades",
            vec![
                Grenade(GrenadeKind::He),
                Grenade(GrenadeKind::Flash),
                Grenade(GrenadeKind::Smoke),
                Grenade(GrenadeKind::Molotov),
            ],
        ),
        ("Gear", vec![Kevlar, KevlarHelmet]),
    ]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HitGroup {
    Head,
    Chest,
    Stomach,
    Legs,
}

impl HitGroup {
    /// CS2 hitgroup damage multipliers.
    pub fn multiplier(self) -> f32 {
        match self {
            HitGroup::Head => 4.0,
            HitGroup::Chest => 1.0,
            HitGroup::Stomach => 1.25,
            HitGroup::Legs => 0.75,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DamageSource {
    Weapon(WeaponId),
    Grenade(GrenadeKind),
    Fall,
}

impl DamageSource {
    pub fn name(self) -> &'static str {
        match self {
            DamageSource::Weapon(w) => w.def().name,
            DamageSource::Grenade(g) => g.name(),
            DamageSource::Fall => "Fall",
        }
    }
}

/// Returns (health damage, armor damage) after range falloff, hitgroup and armor.
pub fn compute_damage(
    def: &WeaponDef,
    distance_m: f32,
    group: HitGroup,
    armor: i32,
    helmet: bool,
) -> (i32, i32) {
    let mut dmg = def.damage
        * def
            .range_modifier
            .powf(distance_m * UNITS_PER_METER / 500.0);
    if def.class != WeaponClass::Knife {
        dmg *= group.multiplier();
    }
    let armored = armor > 0 && (group != HitGroup::Head || helmet) && group != HitGroup::Legs;
    apply_armor(dmg, def.armor_pen, armor, armored)
}

pub fn apply_armor(dmg: f32, armor_pen: f32, armor: i32, armored: bool) -> (i32, i32) {
    if !armored {
        return (dmg.round() as i32, 0);
    }
    let mut health = dmg * armor_pen;
    let mut armor_dmg = (dmg - health) * 0.5;
    if armor_dmg > armor as f32 {
        health += (armor_dmg - armor as f32) * 2.0;
        armor_dmg = armor as f32;
    }
    (health.round() as i32, armor_dmg.round() as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ak_headshot_one_taps_even_with_helmet() {
        let ak = WeaponId::Ak47.def();
        let (hp, _) = compute_damage(ak, 20.0, HitGroup::Head, 100, true);
        assert!(hp >= 100, "AK headshot did {hp}");
    }

    #[test]
    fn m4_headshot_does_not_one_tap_helmet_at_range() {
        let m4 = WeaponId::M4a4.def();
        let (hp, _) = compute_damage(m4, 30.0, HitGroup::Head, 100, true);
        assert!(hp < 100, "M4 headshot did {hp}");
    }

    #[test]
    fn headshot_is_four_times_body() {
        let glock = WeaponId::Glock.def();
        let (head, _) = compute_damage(glock, 0.0, HitGroup::Head, 0, false);
        let (body, _) = compute_damage(glock, 0.0, HitGroup::Chest, 0, false);
        assert_eq!(head, body * 4);
    }
}
