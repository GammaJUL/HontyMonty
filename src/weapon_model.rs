//! Blocky weapon meshes with skin textures applied to the body parts.

use bevy::prelude::*;

use crate::cases::{skin, KnifeModel, Pattern};
use crate::profile::EquippedSkin;
use crate::skin_textures::SkinTextures;
use crate::weapons::{GrenadeKind, WeaponId};

#[derive(Clone, Copy, PartialEq)]
enum Role {
    /// Receives the skin texture.
    Body,
    Metal,
    Accent,
    Wood,
}

struct Part {
    size: Vec3,
    pos: Vec3,
    rot_x: f32,
    role: Role,
}

fn p(size: [f32; 3], pos: [f32; 3], role: Role) -> Part {
    Part {
        size: Vec3::from(size),
        pos: Vec3::from(pos),
        rot_x: 0.0,
        role,
    }
}

fn pr(size: [f32; 3], pos: [f32; 3], rot_x: f32, role: Role) -> Part {
    Part {
        size: Vec3::from(size),
        pos: Vec3::from(pos),
        rot_x,
        role,
    }
}

fn parts(weapon: WeaponId, knife: KnifeModel) -> Vec<Part> {
    use Role::*;
    match weapon {
        WeaponId::Knife => match knife {
            KnifeModel::Default => vec![
                p([0.03, 0.035, 0.11], [0.0, 0.0, 0.0], Accent),
                p([0.05, 0.05, 0.012], [0.0, 0.0, -0.06], Metal),
                p([0.008, 0.035, 0.16], [0.0, 0.005, -0.145], Body),
            ],
            KnifeModel::Karambit => vec![
                p([0.03, 0.035, 0.1], [0.0, 0.0, 0.0], Accent),
                pr([0.008, 0.03, 0.08], [0.0, 0.02, -0.08], 0.5, Body),
                pr([0.008, 0.03, 0.07], [0.0, -0.01, -0.14], -0.6, Body),
                p([0.04, 0.04, 0.01], [0.0, 0.0, 0.06], Metal),
            ],
            KnifeModel::Butterfly => vec![
                p([0.012, 0.03, 0.12], [-0.012, 0.0, 0.0], Accent),
                p([0.012, 0.03, 0.12], [0.012, 0.0, 0.0], Accent),
                p([0.008, 0.032, 0.15], [0.0, 0.005, -0.13], Body),
            ],
            KnifeModel::Bayonet => vec![
                p([0.035, 0.04, 0.12], [0.0, 0.0, 0.0], Accent),
                p([0.07, 0.025, 0.015], [0.0, 0.0, -0.065], Metal),
                p([0.01, 0.04, 0.21], [0.0, 0.005, -0.175], Body),
            ],
        },
        WeaponId::Glock | WeaponId::Usp | WeaponId::Deagle => {
            let big = if weapon == WeaponId::Deagle { 1.2 } else { 1.0 };
            let mut v = vec![
                p([0.04 * big, 0.05 * big, 0.2 * big], [0.0, 0.05, -0.06], Body),
                pr([0.035, 0.11, 0.05], [0.0, -0.02, 0.01], -0.25, Accent),
            ];
            if weapon == WeaponId::Usp {
                v.push(p([0.032, 0.032, 0.12], [0.0, 0.05, -0.22], Metal));
            }
            v
        }
        WeaponId::Mp9 => vec![
            p([0.05, 0.07, 0.28], [0.0, 0.04, -0.08], Body),
            p([0.03, 0.14, 0.04], [0.0, -0.06, -0.02], Accent),
            p([0.02, 0.02, 0.1], [0.0, 0.05, -0.27], Metal),
            p([0.03, 0.05, 0.14], [0.0, 0.03, 0.12], Accent),
        ],
        WeaponId::P90 => vec![
            p([0.07, 0.1, 0.42], [0.0, 0.04, -0.06], Body),
            p([0.05, 0.03, 0.3], [0.0, 0.105, -0.06], Accent),
            p([0.02, 0.02, 0.08], [0.0, 0.04, -0.3], Metal),
        ],
        WeaponId::Nova => vec![
            p([0.05, 0.07, 0.3], [0.0, 0.04, -0.05], Body),
            p([0.03, 0.03, 0.5], [0.0, 0.07, -0.42], Metal),
            p([0.045, 0.045, 0.16], [0.0, 0.03, -0.32], Wood),
            p([0.045, 0.09, 0.26], [0.0, 0.0, 0.22], Wood),
        ],
        WeaponId::Ak47 | WeaponId::M4a4 | WeaponId::Aug | WeaponId::Sg553 => {
            let stock = if weapon == WeaponId::Ak47 { Wood } else { Accent };
            let mut v = vec![
                p([0.05, 0.08, 0.42], [0.0, 0.04, -0.1], Body),
                p([0.024, 0.024, 0.32], [0.0, 0.06, -0.47], Metal),
                p([0.045, 0.08, 0.24], [0.0, 0.02, 0.22], stock),
                pr([0.035, 0.15, 0.06], [0.0, -0.07, -0.13], if weapon == WeaponId::Ak47 { 0.35 } else { 0.1 }, Accent),
                pr([0.035, 0.1, 0.04], [0.0, -0.04, 0.04], -0.3, Accent),
            ];
            if matches!(weapon, WeaponId::Aug | WeaponId::Sg553) {
                v.push(p([0.04, 0.04, 0.15], [0.0, 0.12, -0.1], Metal));
            }
            v
        }
        WeaponId::Ssg08 | WeaponId::Awp => {
            let big = if weapon == WeaponId::Awp { 1.15 } else { 1.0 };
            vec![
                p([0.055 * big, 0.08 * big, 0.5], [0.0, 0.04, -0.1], Body),
                p([0.026, 0.026, 0.55 * big], [0.0, 0.06, -0.62], Metal),
                p([0.05, 0.05, 0.28], [0.0, 0.135, -0.12], Metal),
                p([0.05, 0.1, 0.28], [0.0, 0.01, 0.27], Body),
                p([0.035, 0.08, 0.06], [0.0, -0.04, -0.05], Accent),
            ]
        }
    }
}

fn default_colors(weapon: WeaponId) -> (Color, Color) {
    match weapon {
        WeaponId::Ak47 => (Color::srgb(0.12, 0.12, 0.12), Color::srgb(0.1, 0.1, 0.1)),
        WeaponId::Awp => (Color::srgb(0.22, 0.32, 0.18), Color::srgb(0.1, 0.1, 0.1)),
        WeaponId::Aug | WeaponId::Sg553 => (Color::srgb(0.25, 0.3, 0.22), Color::srgb(0.15, 0.15, 0.15)),
        WeaponId::Knife => (Color::srgb(0.75, 0.75, 0.78), Color::srgb(0.1, 0.1, 0.1)),
        WeaponId::P90 => (Color::srgb(0.2, 0.2, 0.2), Color::srgb(0.12, 0.12, 0.12)),
        _ => (Color::srgb(0.18, 0.18, 0.2), Color::srgb(0.1, 0.1, 0.11)),
    }
}

/// Spawns a weapon with the grip at the origin, barrel along -Z.
pub fn spawn_weapon_model(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    mats: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
    textures: &mut SkinTextures,
    weapon: WeaponId,
    skin_item: Option<EquippedSkin>,
) -> Entity {
    let def = skin_item.and_then(|s| skin(s.skin_id).filter(|d| d.weapon == weapon).map(|d| (d, s)));
    let knife = def.map(|(d, _)| d.knife).unwrap_or(KnifeModel::Default);
    let (body_color, accent_color) = default_colors(weapon);

    let body = match def {
        Some((d, item)) => {
            let shiny = matches!(d.pattern, Pattern::Fade | Pattern::Doppler) || weapon == WeaponId::Knife;
            // Worn skins are duller and rougher.
            let wear = item.float.clamp(0.0, 1.0);
            mats.add(StandardMaterial {
                base_color_texture: Some(textures.get(images, d, item.float, item.pattern_seed)),
                metallic: if shiny { 0.8 } else { 0.2 } * (1.0 - wear * 0.7),
                perceptual_roughness: (if shiny { 0.25 } else { 0.6 } + wear * 0.5).min(1.0),
                ..default()
            })
        }
        None => mats.add(StandardMaterial {
            base_color: body_color,
            metallic: if weapon == WeaponId::Knife { 0.9 } else { 0.3 },
            perceptual_roughness: 0.45,
            ..default()
        }),
    };
    let accent = mats.add(StandardMaterial {
        base_color: match def {
            Some((d, _)) => Color::srgb(d.secondary[0] * 0.5, d.secondary[1] * 0.5, d.secondary[2] * 0.5),
            None => accent_color,
        },
        perceptual_roughness: 0.7,
        ..default()
    });
    let metal = mats.add(StandardMaterial {
        base_color: Color::srgb(0.08, 0.08, 0.09),
        metallic: 0.7,
        perceptual_roughness: 0.4,
        ..default()
    });
    let wood = mats.add(StandardMaterial {
        base_color: Color::srgb(0.45, 0.25, 0.1),
        perceptual_roughness: 0.8,
        ..default()
    });

    let root = commands.spawn((Transform::default(), Visibility::default())).id();
    for part in parts(weapon, knife) {
        let mat = match part.role {
            Role::Body => body.clone(),
            Role::Accent => accent.clone(),
            Role::Metal => metal.clone(),
            Role::Wood => wood.clone(),
        };
        let child = commands
            .spawn((
                Mesh3d(meshes.add(Cuboid::from_size(part.size))),
                MeshMaterial3d(mat),
                Transform::from_translation(part.pos).with_rotation(Quat::from_rotation_x(part.rot_x)),
            ))
            .id();
        commands.entity(root).add_child(child);
    }
    root
}

pub fn spawn_grenade_model(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    mats: &mut Assets<StandardMaterial>,
    kind: GrenadeKind,
) -> Entity {
    let c = kind.color();
    let body = mats.add(StandardMaterial {
        base_color: Color::srgb(c[0], c[1], c[2]),
        perceptual_roughness: 0.6,
        ..default()
    });
    let root = commands.spawn((Transform::default(), Visibility::default())).id();
    let shape: Mesh = if kind == GrenadeKind::Molotov {
        Cylinder::new(0.035, 0.16).into()
    } else if kind == GrenadeKind::He {
        Sphere::new(0.045).into()
    } else {
        Cylinder::new(0.035, 0.11).into()
    };
    let child = commands
        .spawn((Mesh3d(meshes.add(shape)), MeshMaterial3d(body), Transform::default()))
        .id();
    commands.entity(root).add_child(child);
    root
}
