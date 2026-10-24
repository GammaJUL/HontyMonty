#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod cases;
mod character;
mod dev;
mod game;
mod game_mode;
mod keybinds;
mod map;
mod menu_scene;
mod net;
mod profile;
mod render_scale;
mod save;
mod settings;
mod skin_textures;
mod states;
mod ui;
mod weapon_model;
mod weapons;

use bevy::prelude::*;
use bevy::window::{PresentMode, WindowResolution};
use bevy::winit::{UpdateMode, WinitSettings};

use profile::Profile;
use settings::Settings;
use states::AppState;

fn main() {
    let settings = Settings::load();
    let profile = Profile::load();
    let (w, h) = settings.window_size;

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Honty Monty".into(),
                resolution: WindowResolution::new(w, h),
                present_mode: if settings.vsync { PresentMode::AutoVsync } else { PresentMode::AutoNoVsync },
                mode: settings::window_mode(settings.display_mode),
                ..default()
            }),
            ..default()
        }))
        // Keep running at full speed when unfocused: the host's window runs the server.
        .insert_resource(WinitSettings {
            focused_mode: UpdateMode::Continuous,
            unfocused_mode: UpdateMode::Continuous,
        })
        .insert_resource(ClearColor(Color::srgb(0.05, 0.06, 0.08)))
        .insert_resource(settings)
        .insert_resource(profile)
        .init_state::<AppState>()
        .add_plugins((
            settings::SettingsPlugin,
            audio::AudioFxPlugin,
            net::NetPlugin,
            ui::UiPlugin,
            menu_scene::MenuScenePlugin,
            game::GamePlugin,
            dev::DevPlugin,
            render_scale::RenderScalePlugin,
        ))
        .init_resource::<skin_textures::SkinTextures>()
        .run();
}
