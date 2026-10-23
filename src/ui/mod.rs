//! Menu screens and shared UI.

pub mod browser;
pub mod customize;
pub mod inventory;
pub mod lobby;
pub mod main_menu;
pub mod settings_menu;
pub mod widgets;

use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;

use crate::settings::Settings;
use crate::states::MenuMessage;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((FrameTimeDiagnosticsPlugin::default(), widgets::WidgetsPlugin))
            .init_resource::<MenuMessage>()
            .add_plugins((
                main_menu::plugin,
                customize::plugin,
                browser::plugin,
                lobby::plugin,
                inventory::plugin,
                settings_menu::plugin,
            ))
            .add_systems(Startup, spawn_fps_counter)
            .add_systems(Update, update_fps_counter);
    }
}

#[derive(Component)]
struct FpsText;

fn spawn_fps_counter(mut commands: Commands) {
    commands.spawn((
        FpsText,
        Text::new(""),
        widgets::font(14.0),
        TextColor(Color::srgb(0.6, 1.0, 0.6)),
        GlobalZIndex(200),
        Node {
            position_type: PositionType::Absolute,
            right: px(8),
            top: px(4),
            ..default()
        },
    ));
}

fn update_fps_counter(
    time: Res<Time>,
    diagnostics: Res<DiagnosticsStore>,
    settings: Res<Settings>,
    mut q: Query<&mut Text, With<FpsText>>,
    mut timer: Local<f32>,
) {
    *timer -= time.delta_secs();
    if *timer > 0.0 && !settings.is_changed() {
        return;
    }
    *timer = 0.25;
    let fps = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|d| d.smoothed())
        .unwrap_or(0.0);
    for mut t in &mut q {
        t.0 = if settings.show_fps { format!("{fps:.0} FPS") } else { String::new() };
    }
}

/// Spawns crosshair lines centered on the parent's origin.
/// `spread_px` pushes the lines outward (dynamic crosshair).
pub fn spawn_crosshair_lines(p: &mut ChildSpawnerCommands, settings: &Settings, spread_px: f32) {
    let c = &settings.crosshair;
    let color = settings.crosshair_color();
    let gap = c.gap + spread_px;
    let th = c.thickness;
    let len = c.size;
    let mut line = |left: f32, top: f32, w: f32, h: f32| {
        p.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(left),
                top: px(top),
                width: px(w),
                height: px(h),
                ..default()
            },
            BackgroundColor(color),
            Outline::new(px(1), px(0), Color::srgba(0.0, 0.0, 0.0, 0.6)),
        ));
    };
    if len > 0.0 {
        line(-gap - len, -th / 2.0, len, th);
        line(gap, -th / 2.0, len, th);
        line(-th / 2.0, -gap - len, th, len);
        line(-th / 2.0, gap, th, len);
    }
    if c.dot {
        line(-th / 2.0, -th / 2.0, th, th);
    }
}
