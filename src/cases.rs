//! CS2-style rarity system, skin catalog and the (single) case.

use serde::{Deserialize, Serialize};

use crate::weapons::WeaponId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Rarity {
    MilSpec,
    Restricted,
    Classified,
    Covert,
    RareSpecial,
}

impl Rarity {
    pub const ALL: [Rarity; 5] = [
        Rarity::MilSpec,
        Rarity::Restricted,
        Rarity::Classified,
        Rarity::Covert,
        Rarity::RareSpecial,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Rarity::MilSpec => "Mil-Spec Grade",
            Rarity::Restricted => "Restricted",
            Rarity::Classified => "Classified",
            Rarity::Covert => "Covert",
            Rarity::RareSpecial => "Rare Special Item",
        }
    }

    /// Official CS2 case odds in percent.
    pub fn odds(self) -> f32 {
        match self {
            Rarity::MilSpec => 79.92,
            Rarity::Restricted => 15.98,
            Rarity::Classified => 3.20,
            Rarity::Covert => 0.64,
            Rarity::RareSpecial => 0.26,
        }
    }

    pub fn color(self) -> [f32; 3] {
        match self {
            Rarity::MilSpec => [0.29, 0.41, 1.0],
            Rarity::Restricted => [0.53, 0.28, 1.0],
            Rarity::Classified => [0.83, 0.17, 0.90],
            Rarity::Covert => [0.92, 0.29, 0.29],
            Rarity::RareSpecial => [0.89, 0.68, 0.22],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wear {
    FactoryNew,
    MinimalWear,
    FieldTested,
    WellWorn,
    BattleScarred,
}

impl Wear {
    pub const ALL: [Wear; 5] = [
        Wear::FactoryNew,
        Wear::MinimalWear,
        Wear::FieldTested,
        Wear::WellWorn,
        Wear::BattleScarred,
    ];

    pub fn from_float(f: f32) -> Wear {
        match f {
            f if f < 0.07 => Wear::FactoryNew,
            f if f < 0.15 => Wear::MinimalWear,
            f if f < 0.38 => Wear::FieldTested,
            f if f < 0.45 => Wear::WellWorn,
            _ => Wear::BattleScarred,
        }
    }

    pub fn range(self) -> (f32, f32) {
        match self {
            Wear::FactoryNew => (0.0, 0.07),
            Wear::MinimalWear => (0.07, 0.15),
            Wear::FieldTested => (0.15, 0.38),
            Wear::WellWorn => (0.38, 0.45),
            Wear::BattleScarred => (0.45, 1.0),
        }
    }

    /// How likely each wear bracket is to be picked before rolling a float inside it.
    fn weight(self) -> f32 {
        match self {
            Wear::FactoryNew => 3.0,
            Wear::MinimalWear => 24.0,
            Wear::FieldTested => 33.0,
            Wear::WellWorn => 24.0,
            Wear::BattleScarred => 16.0,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Wear::FactoryNew => "Factory New",
            Wear::MinimalWear => "Minimal Wear",
            Wear::FieldTested => "Field-Tested",
            Wear::WellWorn => "Well-Worn",
            Wear::BattleScarred => "Battle-Scarred",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Pattern {
    Solid,
    Stripes,
    Camo,
    Fade,
    Web,
    Tiger,
    Pixel,
    Doppler,
    Circuit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum KnifeModel {
    Default,
    Karambit,
    Butterfly,
    Bayonet,
}

impl KnifeModel {
    pub fn name(self) -> &'static str {
        match self {
            KnifeModel::Default => "Knife",
            KnifeModel::Karambit => "Karambit",
            KnifeModel::Butterfly => "Butterfly Knife",
            KnifeModel::Bayonet => "Bayonet",
        }
    }
}

pub struct SkinDef {
    pub id: u16,
    pub weapon: WeaponId,
    pub knife: KnifeModel,
    pub name: &'static str,
    pub rarity: Rarity,
    pub primary: [f32; 3],
    pub secondary: [f32; 3],
    pub pattern: Pattern,
    pub min_float: f32,
    pub max_float: f32,
}

impl SkinDef {
    pub fn full_name(&self) -> String {
        if self.weapon == WeaponId::Knife {
            format!("* {} | {}", self.knife.name(), self.name)
        } else {
            format!("{} | {}", self.weapon.def().name, self.name)
        }
    }
}

pub fn skin(id: u16) -> Option<&'static SkinDef> {
    SKINS.iter().find(|s| s.id == id)
}

const fn s(
    id: u16,
    weapon: WeaponId,
    name: &'static str,
    rarity: Rarity,
    primary: [f32; 3],
    secondary: [f32; 3],
    pattern: Pattern,
) -> SkinDef {
    SkinDef {
        id,
        weapon,
        knife: KnifeModel::Default,
        name,
        rarity,
        primary,
        secondary,
        pattern,
        min_float: 0.0,
        max_float: 1.0,
    }
}

const fn knife(
    id: u16,
    model: KnifeModel,
    name: &'static str,
    primary: [f32; 3],
    secondary: [f32; 3],
    pattern: Pattern,
    min_float: f32,
    max_float: f32,
) -> SkinDef {
    SkinDef {
        id,
        weapon: WeaponId::Knife,
        knife: model,
        name,
        rarity: Rarity::RareSpecial,
        primary,
        secondary,
        pattern,
        min_float,
        max_float,
    }
}

use Pattern::*;
use Rarity::*;
use WeaponId as W;

/// Contents of the Honty Case.
pub static SKINS: [SkinDef; 21] = [
    // Mil-Spec
    s(1, W::Glock, "Neon Drip", MilSpec, [0.1, 0.1, 0.12], [0.2, 1.0, 0.6], Stripes),
    s(2, W::Usp, "Tidewater", MilSpec, [0.1, 0.3, 0.45], [0.6, 0.85, 0.9], Camo),
    s(3, W::Mp9, "Moss Grid", MilSpec, [0.2, 0.3, 0.15], [0.45, 0.55, 0.3], Pixel),
    s(4, W::Nova, "Rust Belt", MilSpec, [0.45, 0.25, 0.12], [0.25, 0.2, 0.18], Camo),
    s(5, W::Sg553, "Desert Static", MilSpec, [0.75, 0.65, 0.45], [0.55, 0.45, 0.3], Pixel),
    s(6, W::P90, "Bubblegum", MilSpec, [1.0, 0.55, 0.75], [0.6, 0.85, 1.0], Stripes),
    // Restricted
    s(7, W::Deagle, "Ember Line", Restricted, [0.12, 0.1, 0.1], [1.0, 0.45, 0.1], Circuit),
    s(8, W::Aug, "Arctic Fox", Restricted, [0.9, 0.93, 0.97], [0.55, 0.65, 0.75], Camo),
    s(9, W::M4a4, "Circuit Board", Restricted, [0.05, 0.25, 0.12], [0.3, 1.0, 0.4], Circuit),
    s(10, W::Ssg08, "Hornet", Restricted, [0.95, 0.8, 0.1], [0.08, 0.08, 0.08], Tiger),
    // Classified
    s(11, W::Ak47, "Jade Serpent", Classified, [0.05, 0.45, 0.3], [0.85, 0.75, 0.3], Web),
    s(12, W::Awp, "Pixel Storm", Classified, [0.25, 0.1, 0.5], [0.1, 0.9, 1.0], Pixel),
    s(13, W::Glock, "Nuclear Bloom", Classified, [0.15, 0.15, 0.1], [0.7, 1.0, 0.1], Fade),
    // Covert
    s(14, W::Ak47, "Inferno King", Covert, [0.6, 0.05, 0.02], [1.0, 0.7, 0.1], Fade),
    s(15, W::Awp, "Dragon's Hoard", Covert, [0.15, 0.4, 0.2], [0.95, 0.75, 0.3], Tiger),
    // Rare special: knives
    knife(16, KnifeModel::Karambit, "Fade", [1.0, 0.85, 0.1], [0.6, 0.2, 0.9], Fade, 0.0, 0.08),
    knife(17, KnifeModel::Karambit, "Doppler", [0.1, 0.05, 0.3], [0.9, 0.2, 0.6], Doppler, 0.0, 0.08),
    knife(18, KnifeModel::Butterfly, "Crimson Web", [0.6, 0.02, 0.05], [0.05, 0.02, 0.02], Web, 0.06, 0.8),
    knife(19, KnifeModel::Butterfly, "Tiger Tooth", [1.0, 0.65, 0.1], [0.3, 0.12, 0.02], Tiger, 0.0, 0.08),
    knife(20, KnifeModel::Bayonet, "Slaughter", [0.85, 0.2, 0.25], [0.95, 0.85, 0.85], Camo, 0.01, 0.26),
    knife(21, KnifeModel::Bayonet, "Night", [0.12, 0.13, 0.18], [0.3, 0.32, 0.4], Stripes, 0.06, 0.8),
];

pub const CASE_NAME: &str = "Honty Case";

/// A rolled item in a player's inventory.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OwnedSkin {
    pub uid: u64,
    pub skin_id: u16,
    pub float: f32,
    pub stattrak: bool,
    pub pattern_seed: u16,
    /// StatTrak kill counter.
    #[serde(default)]
    pub kills: u32,
}

impl OwnedSkin {
    pub fn def(&self) -> &'static SkinDef {
        skin(self.skin_id).expect("owned skin refers to a catalog entry")
    }

    pub fn wear(&self) -> Wear {
        Wear::from_float(self.float)
    }

    pub fn display_name(&self) -> String {
        let st = if self.stattrak { "StatTrak " } else { "" };
        format!("{st}{}", self.def().full_name())
    }
}

pub fn roll_rarity(rng: &mut fastrand::Rng) -> Rarity {
    let mut r = rng.f32() * 100.0;
    for rarity in Rarity::ALL {
        if r < rarity.odds() {
            return rarity;
        }
        r -= rarity.odds();
    }
    Rarity::MilSpec
}

pub fn skins_of(rarity: Rarity) -> impl Iterator<Item = &'static SkinDef> {
    SKINS.iter().filter(move |s| s.rarity == rarity)
}

/// Pick a random item of the given rarity (used for the reel filler too).
pub fn random_skin_of(rng: &mut fastrand::Rng, rarity: Rarity) -> &'static SkinDef {
    let pool: Vec<_> = skins_of(rarity).collect();
    pool[rng.usize(..pool.len())]
}

pub fn roll_float(rng: &mut fastrand::Rng, def: &SkinDef) -> f32 {
    let total: f32 = Wear::ALL.iter().map(|w| w.weight()).sum();
    let mut r = rng.f32() * total;
    let mut wear = Wear::FieldTested;
    for w in Wear::ALL {
        if r < w.weight() {
            wear = w;
            break;
        }
        r -= w.weight();
    }
    let (lo, hi) = wear.range();
    let f = lo + rng.f32() * (hi - lo);
    // Remap into the skin's own float cap, like CS2 does.
    def.min_float + f * (def.max_float - def.min_float)
}

/// Open the case once. Unlimited rolls, no keys needed.
pub fn open_case(rng: &mut fastrand::Rng, uid: u64) -> OwnedSkin {
    let rarity = roll_rarity(rng);
    let def = random_skin_of(rng, rarity);
    OwnedSkin {
        uid,
        skin_id: def.id,
        float: roll_float(rng, def),
        stattrak: rng.f32() < 0.10,
        pattern_seed: rng.u16(0..1000),
        kills: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn odds_add_up_to_100() {
        let total: f32 = Rarity::ALL.iter().map(|r| r.odds()).sum();
        assert!((total - 100.0).abs() < 0.01);
    }

    #[test]
    fn every_rarity_has_items_and_rolls_match_odds() {
        for r in Rarity::ALL {
            assert!(skins_of(r).count() > 0, "{r:?} has no skins");
        }
        let mut rng = fastrand::Rng::with_seed(42);
        let n = 200_000;
        let mut counts = [0usize; 5];
        for i in 0..n {
            let item = open_case(&mut rng, i as u64);
            counts[item.def().rarity as usize] += 1;
            let d = item.def();
            assert!(item.float >= d.min_float && item.float <= d.max_float);
        }
        for r in Rarity::ALL {
            let got = counts[r as usize] as f32 / n as f32 * 100.0;
            assert!((got - r.odds()).abs() < r.odds() * 0.15 + 0.05, "{r:?}: {got}% vs {}%", r.odds());
        }
    }
}
