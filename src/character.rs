//! Builds the blocky player character from primitives based on an `Appearance`.

use bevy::prelude::*;

use crate::game_mode::Team;
use crate::profile::*;

/// Marker on the root entity of a spawned character.
#[derive(Component)]
pub struct CharacterModel;

/// Where held weapons get attached (child of the character root).
#[derive(Component)]
pub struct HandSocket;

struct Builder<'a> {
    meshes: &'a mut Assets<Mesh>,
    mats: &'a mut Assets<StandardMaterial>,
    cache: Vec<(Color, Handle<StandardMaterial>)>,
    parts: Vec<(Mesh3d, MeshMaterial3d<StandardMaterial>, Transform)>,
}

impl Builder<'_> {
    fn mat(&mut self, color: Color) -> Handle<StandardMaterial> {
        if let Some((_, h)) = self.cache.iter().find(|(c, _)| *c == color) {
            return h.clone();
        }
        let h = self.mats.add(StandardMaterial {
            base_color: color,
            perceptual_roughness: 0.8,
            ..default()
        });
        self.cache.push((color, h.clone()));
        h
    }

    fn cube(&mut self, size: Vec3, pos: Vec3, color: Color) {
        self.cube_rot(size, pos, Quat::IDENTITY, color);
    }

    fn cube_rot(&mut self, size: Vec3, pos: Vec3, rot: Quat, color: Color) {
        let mesh = self.meshes.add(Cuboid::from_size(size));
        let mat = self.mat(color);
        self.parts.push((
            Mesh3d(mesh),
            MeshMaterial3d(mat),
            Transform::from_translation(pos).with_rotation(rot),
        ));
    }
}

/// Spawns a character with its feet at the entity origin, facing -Z.
/// Returns (root, hand socket).
pub fn spawn_character(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    mats: &mut Assets<StandardMaterial>,
    look: &Appearance,
    team: Option<Team>,
) -> (Entity, Entity) {
    let mut b = Builder {
        meshes,
        mats,
        cache: Vec::new(),
        parts: Vec::new(),
    };
    let skin = pal(SKIN_TONES, look.skin_tone);
    let hair = pal(HAIR_COLORS, look.hair_color);
    let eye = pal(EYE_COLORS, look.eye_color);
    let shirt = pal(CLOTHING_COLORS, look.shirt_color);
    let pants = pal(CLOTHING_COLORS, look.pants_color);
    let female = look.gender == Gender::Female;
    let shoe = Color::srgb(0.08, 0.07, 0.06);

    let torso_w = if female { 0.42 } else { 0.5 };
    let torso_d = if female { 0.24 } else { 0.26 };

    // Legs and shoes.
    for side in [-1.0f32, 1.0] {
        b.cube(Vec3::new(0.18, 0.85, 0.2), Vec3::new(side * 0.11, 0.425, 0.0), pants);
        b.cube(Vec3::new(0.2, 0.08, 0.28), Vec3::new(side * 0.11, 0.04, -0.03), shoe);
    }
    if female {
        b.cube(Vec3::new(0.44, 0.14, 0.25), Vec3::new(0.0, 0.88, 0.0), pants);
    }
    // Torso.
    b.cube(Vec3::new(torso_w, 0.62, torso_d), Vec3::new(0.0, 1.16, 0.0), shirt);
    if let Some(team) = team {
        let c = team.color();
        let vest = Color::srgb(c[0], c[1], c[2]);
        b.cube(
            Vec3::new(torso_w + 0.03, 0.38, torso_d + 0.03),
            Vec3::new(0.0, 1.2, 0.0),
            vest,
        );
    }
    // Arms held forward (weapon pose).
    for side in [-1.0f32, 1.0] {
        let shoulder = Vec3::new(side * (torso_w * 0.5 + 0.06), 1.4, 0.0);
        let rot = Quat::from_rotation_x(1.25) * Quat::from_rotation_z(side * -0.25);
        let down = rot * Vec3::new(0.0, -0.17, 0.0);
        b.cube_rot(Vec3::new(0.13, 0.34, 0.14), shoulder + down, rot, shirt);
        let hand = rot * Vec3::new(0.0, -0.42, 0.0);
        b.cube_rot(Vec3::new(0.11, 0.16, 0.11), shoulder + hand, rot, skin);
    }
    // Neck & head.
    b.cube(Vec3::new(0.12, 0.08, 0.12), Vec3::new(0.0, 1.5, 0.0), skin);
    b.cube(Vec3::new(0.26, 0.28, 0.26), Vec3::new(0.0, 1.67, 0.0), skin);
    // Eyes: whites + iris on the front face (-Z).
    for side in [-1.0f32, 1.0] {
        b.cube(Vec3::new(0.065, 0.045, 0.01), Vec3::new(side * 0.06, 1.69, -0.131), Color::WHITE);
        b.cube(Vec3::new(0.032, 0.04, 0.012), Vec3::new(side * 0.06, 1.69, -0.137), eye);
        if female {
            b.cube(Vec3::new(0.07, 0.012, 0.012), Vec3::new(side * 0.06, 1.718, -0.138), Color::BLACK);
        } else {
            b.cube(Vec3::new(0.07, 0.016, 0.012), Vec3::new(side * 0.06, 1.725, -0.136), hair);
        }
    }
    let lips = if female { Color::srgb(0.75, 0.3, 0.35) } else { Color::srgb(0.35, 0.18, 0.15) };
    b.cube(Vec3::new(0.08, 0.018, 0.01), Vec3::new(0.0, 1.6, -0.131), lips);

    // Hair.
    let cap = |b: &mut Builder, h: f32| {
        b.cube(Vec3::new(0.28, h, 0.28), Vec3::new(0.0, 1.81 + h * 0.5 - 0.02, 0.005), hair);
    };
    match look.hair_style {
        HairStyle::Bald => {}
        HairStyle::Buzz => cap(&mut b, 0.035),
        HairStyle::Short => {
            cap(&mut b, 0.07);
            b.cube(Vec3::new(0.28, 0.16, 0.04), Vec3::new(0.0, 1.74, 0.13), hair);
        }
        HairStyle::Spiky => {
            cap(&mut b, 0.05);
            for (i, x) in [-0.09f32, -0.03, 0.03, 0.09].iter().enumerate() {
                let rot = Quat::from_rotation_z(-x * 3.0) * Quat::from_rotation_x(0.3);
                let z = if i % 2 == 0 { -0.04 } else { 0.05 };
                b.cube_rot(Vec3::new(0.06, 0.12, 0.06), Vec3::new(*x, 1.88, z), rot, hair);
            }
        }
        HairStyle::Mohawk => {
            b.cube(Vec3::new(0.07, 0.14, 0.29), Vec3::new(0.0, 1.87, 0.0), hair);
        }
        HairStyle::Long => {
            cap(&mut b, 0.07);
            b.cube(Vec3::new(0.3, 0.42, 0.06), Vec3::new(0.0, 1.6, 0.14), hair);
            for side in [-1.0f32, 1.0] {
                b.cube(Vec3::new(0.035, 0.3, 0.22), Vec3::new(side * 0.145, 1.66, 0.03), hair);
            }
        }
        HairStyle::Ponytail => {
            cap(&mut b, 0.07);
            b.cube(Vec3::new(0.28, 0.12, 0.04), Vec3::new(0.0, 1.76, 0.13), hair);
            b.cube(Vec3::new(0.08, 0.28, 0.08), Vec3::new(0.0, 1.62, 0.19), hair);
        }
        HairStyle::Bun => {
            cap(&mut b, 0.06);
            b.cube(Vec3::new(0.13, 0.12, 0.13), Vec3::new(0.0, 1.86, 0.11), hair);
        }
    }

    let scale = look.height.clamp(0.9, 1.1) * if female { 0.96 } else { 1.0 };
    let root = commands
        .spawn((
            CharacterModel,
            Transform::default(),
            Visibility::default(),
        ))
        .id();
    let body = commands
        .spawn((Transform::from_scale(Vec3::splat(scale)), Visibility::default()))
        .id();
    commands.entity(root).add_child(body);
    for part in b.parts {
        let e = commands.spawn(part).id();
        commands.entity(body).add_child(e);
    }
    let hand = commands
        .spawn((
            HandSocket,
            Transform::from_xyz(0.0, 1.3, -0.42),
            Visibility::default(),
        ))
        .id();
    commands.entity(body).add_child(hand);
    (root, hand)
}
