//! The local player's profile: character appearance, skin inventory and loadout.
//! Saved to `saves/profile.json`.

use std::collections::BTreeMap;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::cases::OwnedSkin;
use crate::save;
use crate::weapons::WeaponId;

const FILE: &str = "profile.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Gender {
    Male,
    Female,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HairStyle {
    Bald,
    Buzz,
    Short,
    Spiky,
    Mohawk,
    Long,
    Ponytail,
    Bun,
}

impl HairStyle {
    pub const ALL: [HairStyle; 8] = [
        HairStyle::Bald,
        HairStyle::Buzz,
        HairStyle::Short,
        HairStyle::Spiky,
        HairStyle::Mohawk,
        HairStyle::Long,
        HairStyle::Ponytail,
        HairStyle::Bun,
    ];

    pub fn name(self) -> &'static str {
        match self {
            HairStyle::Bald => "Bald",
            HairStyle::Buzz => "Buzz Cut",
            HairStyle::Short => "Short",
            HairStyle::Spiky => "Spiky",
            HairStyle::Mohawk => "Mohawk",
            HairStyle::Long => "Long",
            HairStyle::Ponytail => "Ponytail",
            HairStyle::Bun => "Bun",
        }
    }
}

pub type Palette = &'static [(&'static str, [f32; 3])];

pub const SKIN_TONES: Palette = &[
    ("Porcelain", [0.98, 0.87, 0.80]),
    ("Light", [0.94, 0.78, 0.66]),
    ("Medium", [0.84, 0.64, 0.49]),
    ("Tan", [0.72, 0.52, 0.36]),
    ("Brown", [0.52, 0.36, 0.24]),
    ("Dark", [0.33, 0.22, 0.15]),
];

pub const HAIR_COLORS: Palette = &[
    ("Black", [0.05, 0.04, 0.04]),
    ("Dark Brown", [0.22, 0.13, 0.07]),
    ("Brown", [0.42, 0.26, 0.13]),
    ("Blonde", [0.88, 0.74, 0.42]),
    ("Ginger", [0.72, 0.30, 0.10]),
    ("Grey", [0.62, 0.62, 0.64]),
    ("Platinum", [0.93, 0.92, 0.86]),
    ("Pink", [0.95, 0.42, 0.68]),
    ("Blue", [0.22, 0.42, 0.92]),
    ("Green", [0.22, 0.72, 0.36]),
];

pub const EYE_COLORS: Palette = &[
    ("Brown", [0.36, 0.20, 0.08]),
    ("Hazel", [0.55, 0.42, 0.20]),
    ("Green", [0.20, 0.55, 0.28]),
    ("Blue", [0.22, 0.48, 0.88]),
    ("Grey", [0.55, 0.60, 0.64]),
    ("Amber", [0.85, 0.55, 0.12]),
    ("Violet", [0.55, 0.30, 0.85]),
];

pub const CLOTHING_COLORS: Palette = &[
    ("Olive", [0.30, 0.35, 0.20]),
    ("Charcoal", [0.15, 0.15, 0.17]),
    ("Navy", [0.10, 0.15, 0.32]),
    ("Desert", [0.70, 0.60, 0.42]),
    ("Crimson", [0.55, 0.08, 0.10]),
    ("Arctic", [0.85, 0.88, 0.90]),
    ("Forest", [0.10, 0.30, 0.15]),
    ("Purple", [0.35, 0.15, 0.50]),
];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Appearance {
    pub gender: Gender,
    pub skin_tone: usize,
    pub hair_style: HairStyle,
    pub hair_color: usize,
    pub eye_color: usize,
    pub shirt_color: usize,
    pub pants_color: usize,
    /// 0.9 ..= 1.1 body height scale.
    pub height: f32,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            gender: Gender::Male,
            skin_tone: 1,
            hair_style: HairStyle::Short,
            hair_color: 1,
            eye_color: 3,
            shirt_color: 0,
            pants_color: 1,
            height: 1.0,
        }
    }
}

pub fn pal(p: Palette, i: usize) -> Color {
    let c = p[i % p.len()].1;
    Color::srgb(c[0], c[1], c[2])
}

/// What other players need to know to render my weapons.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct EquippedSkin {
    pub weapon: WeaponId,
    pub skin_id: u16,
    pub float: f32,
}

#[derive(Resource, Clone, Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Profile {
    pub appearance: Appearance,
    pub inventory: Vec<OwnedSkin>,
    /// weapon -> uid of the equipped inventory item
    pub equipped: BTreeMap<WeaponId, u64>,
    pub next_uid: u64,
    pub cases_opened: u64,
}

impl Profile {
    pub fn load() -> Self {
        save::load(FILE)
    }

    pub fn save(&self) {
        save::store(FILE, self);
    }

    pub fn equipped_skin(&self, weapon: WeaponId) -> Option<&OwnedSkin> {
        let uid = self.equipped.get(&weapon)?;
        self.inventory.iter().find(|s| s.uid == *uid)
    }

    pub fn loadout(&self) -> Vec<EquippedSkin> {
        self.equipped
            .keys()
            .filter_map(|w| {
                self.equipped_skin(*w).map(|s| EquippedSkin {
                    weapon: *w,
                    skin_id: s.skin_id,
                    float: s.float,
                })
            })
            .collect()
    }
}
