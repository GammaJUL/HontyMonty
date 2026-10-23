//! Procedural textures for weapon skins, cached per skin + wear bracket.

use std::collections::HashMap;

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::cases::{Pattern, SkinDef, Wear};

const SIZE: u32 = 64;

#[derive(Resource, Default)]
pub struct SkinTextures {
    cache: HashMap<(u16, u8), Handle<Image>>,
}

impl SkinTextures {
    pub fn get(&mut self, images: &mut Assets<Image>, def: &SkinDef, float: f32) -> Handle<Image> {
        let wear = Wear::ALL
            .iter()
            .position(|w| *w == Wear::from_float(float))
            .unwrap_or(0) as u8;
        self.cache
            .entry((def.id, wear))
            .or_insert_with(|| images.add(render(def, wear)))
            .clone()
    }
}

fn hash(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(374_761_393)
        ^ (y as u32).wrapping_mul(668_265_263)
        ^ seed.wrapping_mul(2_246_822_519);
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    ((h ^ (h >> 16)) & 0xffff) as f32 / 65535.0
}

fn value_noise(u: f32, v: f32, scale: f32, seed: u32) -> f32 {
    let (x, y) = (u * scale, v * scale);
    let (xi, yi) = (x.floor() as i32, y.floor() as i32);
    let (fx, fy) = (x.fract(), y.fract());
    let s = |t: f32| t * t * (3.0 - 2.0 * t);
    let (sx, sy) = (s(fx), s(fy));
    let wrap = scale as i32;
    let h = |a: i32, b: i32| hash(a.rem_euclid(wrap), b.rem_euclid(wrap), seed);
    let a = h(xi, yi) + (h(xi + 1, yi) - h(xi, yi)) * sx;
    let b = h(xi, yi + 1) + (h(xi + 1, yi + 1) - h(xi, yi + 1)) * sx;
    a + (b - a) * sy
}

fn pattern_mix(p: Pattern, u: f32, v: f32, seed: u32) -> f32 {
    match p {
        Pattern::Solid => 0.0,
        Pattern::Stripes => ((((u + v) * 5.0).fract()) < 0.35) as u8 as f32,
        Pattern::Camo => {
            let n = value_noise(u, v, 4.0, seed) * 0.65 + value_noise(u, v, 8.0, seed + 1) * 0.35;
            (n > 0.52) as u8 as f32
        }
        Pattern::Fade => (u * 0.6 + v * 0.4).clamp(0.0, 1.0),
        Pattern::Web => {
            let (dx, dy) = (u - 0.5, v - 0.5);
            let r = (dx * dx + dy * dy).sqrt();
            let a = dy.atan2(dx);
            let spoke = ((a * 8.0 / std::f32::consts::TAU).fract() - 0.5).abs() < 0.04;
            let ring = ((r * 9.0).fract() - 0.5).abs() < 0.06;
            (spoke || ring) as u8 as f32
        }
        Pattern::Tiger => {
            let n = value_noise(u, v, 4.0, seed);
            (((u * 9.0 + n * 3.0) * std::f32::consts::PI).sin() > 0.55) as u8 as f32
        }
        Pattern::Pixel => {
            let (x, y) = ((u * 8.0) as i32, (v * 8.0) as i32);
            (hash(x, y, seed) > 0.55) as u8 as f32
        }
        Pattern::Doppler => {
            let n = value_noise(u, v, 4.0, seed) * 0.6 + value_noise(u, v, 16.0, seed + 3) * 0.4;
            n.clamp(0.0, 1.0)
        }
        Pattern::Circuit => {
            let gx = (u * 8.0).fract();
            let gy = (v * 8.0).fract();
            let (cx, cy) = ((u * 8.0) as i32, (v * 8.0) as i32);
            let horiz = gy.abs() < 0.12 && hash(cx, cy, seed) > 0.4;
            let vert = gx.abs() < 0.12 && hash(cx, cy, seed + 9) > 0.5;
            let pad = (gx - 0.5).abs() < 0.12 && (gy - 0.5).abs() < 0.12 && hash(cx, cy, seed + 4) > 0.8;
            (horiz || vert || pad) as u8 as f32
        }
    }
}

fn render(def: &SkinDef, wear_index: u8) -> Image {
    let seed = def.id as u32 * 31 + 7;
    // More scratches the further down the wear scale.
    let scratch = [0.0, 0.03, 0.08, 0.14, 0.22][wear_index as usize];
    let mut data = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let u = x as f32 / SIZE as f32;
            let v = y as f32 / SIZE as f32;
            let t = pattern_mix(def.pattern, u, v, seed);
            let mut c = [0.0f32; 3];
            for i in 0..3 {
                c[i] = def.primary[i] + (def.secondary[i] - def.primary[i]) * t;
            }
            let worn = hash(x as i32, y as i32, seed + 99) < scratch;
            if worn {
                c = [0.45, 0.45, 0.47];
            }
            for ch in c {
                data.push((ch.clamp(0.0, 1.0) * 255.0) as u8);
            }
            data.push(255);
        }
    }
    Image::new(
        Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}
