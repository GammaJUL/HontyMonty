//! User settings: video, mouse, audio, crosshair and keybinds.
//! Changes are applied live and saved to `saves/settings.json`.

use std::time::{Duration, Instant};

use bevy::camera::Exposure;
use bevy::prelude::*;
use bevy::render::view::Msaa;
use bevy::window::{MonitorSelection, PresentMode, PrimaryWindow, VideoModeSelection, WindowMode};
use serde::{Deserialize, Serialize};

use crate::keybinds::Keybinds;
use crate::save;

const FILE: &str = "settings.json";

pub const RESOLUTIONS: [(u32, u32); 6] = [
    (1280, 720),
    (1366, 768),
    (1600, 900),
    (1920, 1080),
    (2560, 1440),
    (3840, 2160),
];

pub const FPS_LIMITS: [u32; 8] = [30, 60, 120, 144, 165, 240, 360, 0];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DisplayMode {
    Windowed,
    Borderless,
    Fullscreen,
}

impl DisplayMode {
    pub fn name(self) -> &'static str {
        match self {
            DisplayMode::Windowed => "Windowed",
            DisplayMode::Borderless => "Borderless",
            DisplayMode::Fullscreen => "Fullscreen",
        }
    }
}

pub const CROSSHAIR_COLORS: [(&str, [f32; 3]); 6] = [
    ("Green", [0.2, 1.0, 0.3]),
    ("Cyan", [0.2, 0.95, 1.0]),
    ("Yellow", [1.0, 0.95, 0.2]),
    ("Red", [1.0, 0.25, 0.25]),
    ("White", [1.0, 1.0, 1.0]),
    ("Pink", [1.0, 0.4, 0.85]),
];

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Crosshair {
    pub size: f32,
    pub gap: f32,
    pub thickness: f32,
    pub dot: bool,
    pub color: usize,
    pub dynamic: bool,
}

impl Crosshair {
    /// Short shareable code, e.g. `HM1;7;4;2;0;0;1`.
    pub fn to_code(&self) -> String {
        format!(
            "HM1;{};{};{};{};{};{}",
            self.size,
            self.gap,
            self.thickness,
            self.dot as u8,
            self.color,
            self.dynamic as u8
        )
    }

    pub fn from_code(code: &str) -> Option<Crosshair> {
        let parts: Vec<&str> = code.trim().split(';').collect();
        if parts.len() != 7 || parts[0] != "HM1" {
            return None;
        }
        let f = |i: usize| parts[i].parse::<f32>().ok().filter(|v| v.is_finite());
        Some(Crosshair {
            size: f(1)?.clamp(0.0, 30.0),
            gap: f(2)?.clamp(-5.0, 30.0),
            thickness: f(3)?.clamp(0.5, 8.0),
            dot: parts[4] == "1",
            color: parts[5].parse::<usize>().ok()? % CROSSHAIR_COLORS.len(),
            dynamic: parts[6] == "1",
        })
    }
}

impl Default for Crosshair {
    fn default() -> Self {
        Self {
            size: 7.0,
            gap: 4.0,
            thickness: 2.0,
            dot: false,
            color: 0,
            dynamic: true,
        }
    }
}

#[derive(Resource, Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub player_name: String,
    pub resolution: (u32, u32),
    pub display_mode: DisplayMode,
    pub vsync: bool,
    /// 0 = unlimited.
    pub fps_limit: u32,
    pub show_fps: bool,
    pub fov: f32,
    pub shadows: bool,
    /// 4x MSAA when on.
    pub anti_aliasing: bool,
    /// Exposure multiplier, 1.0 = default.
    pub brightness: f32,
    /// CS-style sensitivity: degrees per mouse count = sensitivity * 0.022.
    pub sensitivity: f32,
    pub zoom_sensitivity: f32,
    pub invert_y: bool,
    pub master_volume: f32,
    pub effects_volume: f32,
    pub ui_volume: f32,
    pub hitsound: bool,
    pub crosshair: Crosshair,
    pub keybinds: Keybinds,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            player_name: format!("Player{}", fastrand::u16(100..999)),
            resolution: (1600, 900),
            display_mode: DisplayMode::Windowed,
            vsync: false,
            fps_limit: 144,
            show_fps: true,
            fov: 90.0,
            shadows: true,
            anti_aliasing: true,
            brightness: 1.0,
            sensitivity: 2.0,
            zoom_sensitivity: 1.0,
            invert_y: false,
            master_volume: 0.7,
            effects_volume: 0.8,
            ui_volume: 0.6,
            hitsound: true,
            crosshair: Crosshair::default(),
            keybinds: Keybinds::default(),
        }
    }
}

impl Settings {
    pub fn load() -> Self {
        save::load(FILE)
    }

    pub fn save(&self) {
        save::store(FILE, self);
    }

    pub fn crosshair_color(&self) -> Color {
        let c = CROSSHAIR_COLORS[self.crosshair.color % CROSSHAIR_COLORS.len()].1;
        Color::srgb(c[0], c[1], c[2])
    }
}

pub struct SettingsPlugin;

impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(FrameLimiter { last: Instant::now() })
            .add_systems(Startup, apply_window_settings)
            .add_systems(
                Update,
                (apply_window_settings, apply_audio_settings, save_settings)
                    .run_if(resource_changed::<Settings>),
            )
            .add_systems(PostUpdate, apply_graphics)
            .add_systems(Last, limit_frame_rate);
    }
}

pub fn window_mode(mode: DisplayMode) -> WindowMode {
    match mode {
        DisplayMode::Windowed => WindowMode::Windowed,
        DisplayMode::Borderless => WindowMode::BorderlessFullscreen(MonitorSelection::Current),
        DisplayMode::Fullscreen => {
            WindowMode::Fullscreen(MonitorSelection::Current, VideoModeSelection::Current)
        }
    }
}

fn apply_window_settings(settings: Res<Settings>, mut window: Single<&mut Window, With<PrimaryWindow>>) {
    let present = if settings.vsync { PresentMode::AutoVsync } else { PresentMode::AutoNoVsync };
    if window.present_mode != present {
        window.present_mode = present;
    }
    let mode = window_mode(settings.display_mode);
    if window.mode != mode {
        window.mode = mode;
    }
    let (w, h) = settings.resolution;
    if settings.display_mode == DisplayMode::Windowed
        && (window.resolution.physical_width() != w || window.resolution.physical_height() != h)
    {
        window.resolution.set_physical_resolution(w, h);
    }
}

/// Runs every frame so newly spawned cameras and lights pick the settings up too.
fn apply_graphics(
    mut commands: Commands,
    settings: Res<Settings>,
    cams: Query<(Entity, Option<&Msaa>, Option<&Exposure>), With<Camera3d>>,
    mut lights: Query<&mut DirectionalLight>,
) {
    let msaa = if settings.anti_aliasing { Msaa::Sample4 } else { Msaa::Off };
    let exposure = Exposure {
        ev100: Exposure::default().ev100 - settings.brightness.max(0.1).log2(),
    };
    for (e, current_msaa, current_exposure) in &cams {
        if current_msaa != Some(&msaa) {
            commands.entity(e).insert(msaa);
        }
        if current_exposure.map(|x| x.ev100) != Some(exposure.ev100) {
            commands.entity(e).insert(exposure);
        }
    }
    for mut light in &mut lights {
        if light.shadow_maps_enabled != settings.shadows {
            light.shadow_maps_enabled = settings.shadows;
        }
    }
}

fn apply_audio_settings(settings: Res<Settings>, mut global: ResMut<GlobalVolume>) {
    global.volume = bevy::audio::Volume::Linear(settings.master_volume);
}

fn save_settings(settings: Res<Settings>) {
    settings.save();
}

#[derive(Resource)]
struct FrameLimiter {
    last: Instant,
}

/// Sleeps at the end of the frame to honour the FPS cap (ignored with VSync on).
fn limit_frame_rate(settings: Res<Settings>, mut limiter: ResMut<FrameLimiter>) {
    if settings.fps_limit > 0 && !settings.vsync {
        let target = Duration::from_secs_f64(1.0 / settings.fps_limit as f64);
        let elapsed = limiter.last.elapsed();
        if elapsed < target {
            let remaining = target - elapsed;
            // Sleep most of it, spin the rest for accuracy.
            if remaining > Duration::from_millis(2) {
                std::thread::sleep(remaining - Duration::from_millis(1));
            }
            while limiter.last.elapsed() < target {
                std::hint::spin_loop();
            }
        }
    }
    limiter.last = Instant::now();
}
