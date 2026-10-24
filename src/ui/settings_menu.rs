//! Settings overlay usable from the main menu and in-game.

use bevy::input::keyboard::KeyboardInput;
use bevy::input::ButtonState;
use bevy::clipboard::{Clipboard, ClipboardRead};
use bevy::prelude::*;

use super::widgets::*;
use crate::keybinds::{Action, Binding, Keybinds};
use crate::settings::*;

/// Set to true to open the settings overlay.
#[derive(Resource, Default)]
pub struct SettingsOpen(pub bool);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tab {
    Video,
    Mouse,
    Audio,
    Crosshair,
    Keybinds,
}

const TABS: [(Tab, &str); 5] = [
    (Tab::Video, "Video"),
    (Tab::Mouse, "Mouse"),
    (Tab::Audio, "Audio"),
    (Tab::Crosshair, "Crosshair"),
    (Tab::Keybinds, "Keybinds"),
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Field {
    Resolution,
    DisplayMode,
    VSync,
    FpsLimit,
    ShowFps,
    Fov,
    Shadows,
    AntiAliasing,
    Brightness,
    Sensitivity,
    ZoomSensitivity,
    InvertY,
    Master,
    Effects,
    Interface,
    Hitsound,
    ChSize,
    ChGap,
    ChThickness,
    ChDot,
    ChColor,
    ChDynamic,
}

fn fields(tab: Tab) -> &'static [(Field, &'static str)] {
    match tab {
        Tab::Video => &[
            (Field::Resolution, "Resolution"),
            (Field::DisplayMode, "Display Mode"),
            (Field::VSync, "VSync"),
            (Field::FpsLimit, "FPS Limit"),
            (Field::ShowFps, "Show FPS"),
            (Field::Fov, "Field of View"),
            (Field::Shadows, "Shadows"),
            (Field::AntiAliasing, "Anti-aliasing (4x MSAA)"),
            (Field::Brightness, "Brightness"),
        ],
        Tab::Mouse => &[
            (Field::Sensitivity, "Sensitivity"),
            (Field::ZoomSensitivity, "Zoom Sensitivity"),
            (Field::InvertY, "Invert Mouse Y"),
        ],
        Tab::Audio => &[
            (Field::Master, "Master Volume"),
            (Field::Effects, "Effects Volume"),
            (Field::Interface, "Interface Volume"),
            (Field::Hitsound, "Hit Sound"),
        ],
        Tab::Crosshair => &[
            (Field::ChSize, "Length"),
            (Field::ChGap, "Gap"),
            (Field::ChThickness, "Thickness"),
            (Field::ChDot, "Center Dot"),
            (Field::ChColor, "Color"),
            (Field::ChDynamic, "Dynamic (spread)"),
        ],
        Tab::Keybinds => &[],
    }
}

#[derive(Component, Clone, Copy)]
enum SettingsButton {
    Tab(Tab),
    Step(Field, i32),
    Rebind(Action),
    ResetTab,
    CopyCode,
    PasteCode,
    Close,
}

#[derive(Component)]
struct CodeText;

#[derive(Component)]
struct SettingsRoot;

#[derive(Component)]
struct TabBody;

#[derive(Component)]
struct ValueText(Field);

#[derive(Component)]
struct BindText(Action);

#[derive(Component)]
struct CrosshairPreview;

#[derive(Resource)]
struct SettingsUi {
    tab: Tab,
    built_tab: Option<Tab>,
    capturing: Option<Action>,
    code_status: String,
    paste: Option<ClipboardRead>,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<SettingsOpen>()
        .insert_resource(SettingsUi {
            tab: Tab::Video,
            built_tab: None,
            capturing: None,
            code_status: String::new(),
            paste: None,
        })
        .add_systems(
            Update,
            (escape_closes, open_close, build_tab, handle, crosshair_code, capture_binding, refresh_values, preview_crosshair).chain(),
        );
}

/// Applies a pasted crosshair code once the clipboard read finishes, and shows the current code.
fn crosshair_code(
    mut ui: ResMut<SettingsUi>,
    mut settings: ResMut<Settings>,
    mut text: Query<&mut Text, With<CodeText>>,
) {
    if let Some(result) = ui.paste.as_mut().and_then(|r| r.poll_result()) {
        ui.paste = None;
        ui.code_status = match result.ok().and_then(|s| Crosshair::from_code(&s)) {
            Some(c) => {
                settings.crosshair = c;
                "Crosshair imported!".into()
            }
            None => "Clipboard has no valid crosshair code".into(),
        };
    }
    if let Ok(mut t) = text.single_mut() {
        let s = format!("{}   {}", settings.crosshair.to_code(), ui.code_status);
        if t.0 != s {
            t.0 = s;
        }
    }
}

/// In menus Escape closes the overlay (in-game the pause menu handles Escape).
fn escape_closes(
    keys: Res<ButtonInput<KeyCode>>,
    state: Res<State<crate::states::AppState>>,
    ui: Res<SettingsUi>,
    mut open: ResMut<SettingsOpen>,
) {
    if open.0 && ui.capturing.is_none() && state.get().is_menu() && keys.just_pressed(KeyCode::Escape) {
        open.0 = false;
    }
}

fn open_close(
    mut commands: Commands,
    open: Res<SettingsOpen>,
    mut ui: ResMut<SettingsUi>,
    root: Query<Entity, With<SettingsRoot>>,
) {
    let exists = !root.is_empty();
    if open.0 && !exists {
        ui.built_tab = None;
        ui.capturing = None;
        commands
            .spawn((
                SettingsRoot,
                GlobalZIndex(100),
                Node {
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..screen_root()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
            ))
            .with_children(|root| {
                root.spawn(panel(px(720))).with_children(|p| {
                    p.spawn(Node {
                        justify_content: JustifyContent::SpaceBetween,
                        ..row()
                    })
                    .with_children(|r| {
                        title(r, "Settings");
                        button(r, "Close", SettingsButton::Close, px(100));
                    });
                    p.spawn(row()).with_children(|r| {
                        for (tab, name) in TABS {
                            button_sized(r, name, SettingsButton::Tab(tab), px(124), 36.0, 16.0);
                        }
                    });
                    p.spawn((
                        TabBody,
                        Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: px(6),
                            min_height: px(420),
                            margin: UiRect::top(px(10)),
                            ..default()
                        },
                    ));
                    p.spawn(row()).with_children(|r| {
                        button(r, "Reset this tab", SettingsButton::ResetTab, px(210));
                        label(r, "Settings are saved automatically.", 14.0, TEXT_DIM);
                    });
                });
            });
    } else if !open.0 && exists {
        for e in &root {
            commands.entity(e).despawn();
        }
    }
}

fn build_tab(
    mut commands: Commands,
    mut ui: ResMut<SettingsUi>,
    body: Query<Entity, With<TabBody>>,
    tab_buttons: Query<(Entity, &SettingsButton)>,
) {
    let Ok(body) = body.single() else { return };
    if ui.built_tab == Some(ui.tab) {
        return;
    }
    ui.built_tab = Some(ui.tab);
    for (e, b) in &tab_buttons {
        if let SettingsButton::Tab(t) = b {
            if *t == ui.tab {
                commands.entity(e).insert(Selected);
            } else {
                commands.entity(e).remove::<Selected>();
            }
        }
    }
    commands.entity(body).despawn_children();
    let tab = ui.tab;
    commands.entity(body).with_children(|b| {
        if tab == Tab::Keybinds {
            b.spawn(Node {
                display: Display::Grid,
                grid_template_columns: RepeatedGridTrack::flex(2, 1.0),
                column_gap: px(18),
                row_gap: px(4),
                ..default()
            })
            .with_children(|g| {
                for action in Action::ALL {
                    g.spawn(Node {
                        justify_content: JustifyContent::SpaceBetween,
                        ..row()
                    })
                    .with_children(|r| {
                        label(r, action.label(), 16.0, TEXT_DIM);
                        r.spawn((
                            UiButton,
                            SettingsButton::Rebind(action),
                            button_node(px(140), 32.0),
                            BackgroundColor(BG_BUTTON),
                            BorderColor::all(BORDER),
                        ))
                        .with_children(|bt| {
                            bt.spawn((BindText(action), Text::new(""), font(16.0), TextColor(TEXT)));
                        });
                    });
                }
            });
            label(b, "Click a binding, then press a key or mouse button (Esc cancels).", 14.0, TEXT_DIM);
            return;
        }
        for (field, name) in fields(tab) {
            b.spawn(Node {
                justify_content: JustifyContent::SpaceBetween,
                ..row()
            })
            .with_children(|r| {
                r.spawn((Text::new(*name), font(18.0), TextColor(TEXT_DIM), Node {
                    width: px(260),
                    ..default()
                }));
                button_sized(r, "<", SettingsButton::Step(*field, -1), px(40), 34.0, 18.0);
                r.spawn((
                    ValueText(*field),
                    Text::new(""),
                    font(18.0),
                    TextColor(TEXT),
                    TextLayout::justify(Justify::Center),
                    Node {
                        width: px(220),
                        ..default()
                    },
                ));
                button_sized(r, ">", SettingsButton::Step(*field, 1), px(40), 34.0, 18.0);
            });
        }
        if tab == Tab::Crosshair {
            b.spawn(row()).with_children(|r| {
                button_sized(r, "Copy code", SettingsButton::CopyCode, px(120), 32.0, 15.0);
                button_sized(r, "Paste code", SettingsButton::PasteCode, px(120), 32.0, 15.0);
                r.spawn((CodeText, Text::new(""), font(14.0), TextColor(TEXT_DIM)));
            });
            b.spawn((
                Node {
                    width: px(160),
                    height: px(160),
                    margin: UiRect::top(px(14)),
                    align_self: AlignSelf::Center,
                    border_radius: BorderRadius::all(px(6)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.25, 0.3, 0.35)),
            ))
            .with_children(|p| {
                p.spawn((
                    CrosshairPreview,
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(80),
                        top: px(80),
                        ..default()
                    },
                ));
            });
        }
    });
}

fn cycle<T: Copy + PartialEq>(list: &[T], cur: T, d: i32) -> T {
    let i = list.iter().position(|x| *x == cur).unwrap_or(0) as i32;
    list[(i + d).rem_euclid(list.len() as i32) as usize]
}

fn step_field(s: &mut Settings, field: Field, d: i32) {
    let df = d as f32;
    match field {
        Field::Resolution => s.resolution = cycle(&RESOLUTIONS, s.resolution, d),
        Field::DisplayMode => {
            s.display_mode = cycle(
                &[DisplayMode::Windowed, DisplayMode::Borderless, DisplayMode::Fullscreen],
                s.display_mode,
                d,
            )
        }
        Field::VSync => s.vsync = !s.vsync,
        Field::FpsLimit => s.fps_limit = cycle(&FPS_LIMITS, s.fps_limit, d),
        Field::ShowFps => s.show_fps = !s.show_fps,
        Field::Fov => s.fov = (s.fov + df * 5.0).clamp(60.0, 120.0),
        Field::Shadows => s.shadows = !s.shadows,
        Field::AntiAliasing => s.anti_aliasing = !s.anti_aliasing,
        Field::Brightness => s.brightness = ((s.brightness + df * 0.1) * 10.0).round().clamp(5.0, 20.0) / 10.0,
        Field::Sensitivity => s.sensitivity = ((s.sensitivity + df * 0.1) * 100.0).round().clamp(5.0, 1000.0) / 100.0,
        Field::ZoomSensitivity => {
            s.zoom_sensitivity = ((s.zoom_sensitivity + df * 0.05) * 100.0).round().clamp(5.0, 300.0) / 100.0
        }
        Field::InvertY => s.invert_y = !s.invert_y,
        Field::Master => s.master_volume = ((s.master_volume + df * 0.05) * 100.0).round().clamp(0.0, 100.0) / 100.0,
        Field::Effects => s.effects_volume = ((s.effects_volume + df * 0.05) * 100.0).round().clamp(0.0, 100.0) / 100.0,
        Field::Interface => s.ui_volume = ((s.ui_volume + df * 0.05) * 100.0).round().clamp(0.0, 100.0) / 100.0,
        Field::Hitsound => s.hitsound = !s.hitsound,
        Field::ChSize => s.crosshair.size = (s.crosshair.size + df).clamp(0.0, 30.0),
        Field::ChGap => s.crosshair.gap = (s.crosshair.gap + df).clamp(-5.0, 30.0),
        Field::ChThickness => s.crosshair.thickness = (s.crosshair.thickness + df * 0.5).clamp(0.5, 8.0),
        Field::ChDot => s.crosshair.dot = !s.crosshair.dot,
        Field::ChColor => {
            s.crosshair.color = (s.crosshair.color as i32 + d).rem_euclid(CROSSHAIR_COLORS.len() as i32) as usize
        }
        Field::ChDynamic => s.crosshair.dynamic = !s.crosshair.dynamic,
    }
}

fn field_text(s: &Settings, field: Field) -> String {
    let onoff = |b: bool| if b { "On" } else { "Off" }.to_string();
    match field {
        Field::Resolution => format!("{} x {}", s.resolution.0, s.resolution.1),
        Field::DisplayMode => s.display_mode.name().to_string(),
        Field::VSync => onoff(s.vsync),
        Field::FpsLimit => {
            if s.fps_limit == 0 { "Unlimited".into() } else { format!("{} FPS", s.fps_limit) }
        }
        Field::ShowFps => onoff(s.show_fps),
        Field::Fov => format!("{:.0} deg", s.fov),
        Field::Shadows => onoff(s.shadows),
        Field::AntiAliasing => onoff(s.anti_aliasing),
        Field::Brightness => format!("{:.0}%", s.brightness * 100.0),
        Field::Sensitivity => format!("{:.2}", s.sensitivity),
        Field::ZoomSensitivity => format!("{:.2}x", s.zoom_sensitivity),
        Field::InvertY => onoff(s.invert_y),
        Field::Master => format!("{:.0}%", s.master_volume * 100.0),
        Field::Effects => format!("{:.0}%", s.effects_volume * 100.0),
        Field::Interface => format!("{:.0}%", s.ui_volume * 100.0),
        Field::Hitsound => onoff(s.hitsound),
        Field::ChSize => format!("{:.0}", s.crosshair.size),
        Field::ChGap => format!("{:.0}", s.crosshair.gap),
        Field::ChThickness => format!("{:.1}", s.crosshair.thickness),
        Field::ChDot => onoff(s.crosshair.dot),
        Field::ChColor => CROSSHAIR_COLORS[s.crosshair.color % CROSSHAIR_COLORS.len()].0.to_string(),
        Field::ChDynamic => onoff(s.crosshair.dynamic),
    }
}

fn handle(
    q: Clicked<SettingsButton>,
    mut ui: ResMut<SettingsUi>,
    mut settings: ResMut<Settings>,
    mut open: ResMut<SettingsOpen>,
    mut clipboard: ResMut<Clipboard>,
) {
    for action in clicked(&q) {
        match *action {
            SettingsButton::Tab(t) => ui.tab = t,
            SettingsButton::Step(f, d) => step_field(&mut settings, f, d),
            SettingsButton::Rebind(a) => ui.capturing = Some(a),
            SettingsButton::ResetTab => {
                let d = Settings::default();
                match ui.tab {
                    Tab::Video => {
                        settings.resolution = d.resolution;
                        settings.display_mode = d.display_mode;
                        settings.vsync = d.vsync;
                        settings.fps_limit = d.fps_limit;
                        settings.show_fps = d.show_fps;
                        settings.fov = d.fov;
                        settings.shadows = d.shadows;
                        settings.anti_aliasing = d.anti_aliasing;
                        settings.brightness = d.brightness;
                    }
                    Tab::Mouse => {
                        settings.sensitivity = d.sensitivity;
                        settings.zoom_sensitivity = d.zoom_sensitivity;
                        settings.invert_y = d.invert_y;
                    }
                    Tab::Audio => {
                        settings.master_volume = d.master_volume;
                        settings.effects_volume = d.effects_volume;
                        settings.ui_volume = d.ui_volume;
                        settings.hitsound = d.hitsound;
                    }
                    Tab::Crosshair => settings.crosshair = d.crosshair,
                    Tab::Keybinds => settings.keybinds = Keybinds::default(),
                }
            }
            SettingsButton::CopyCode => {
                ui.code_status = match clipboard.set_text(settings.crosshair.to_code()) {
                    Ok(()) => "Copied!".into(),
                    Err(e) => format!("Copy failed: {e:?}"),
                };
            }
            SettingsButton::PasteCode => ui.paste = Some(clipboard.fetch_text()),
            SettingsButton::Close => open.0 = false,
        }
    }
}

fn capture_binding(
    mut ui: ResMut<SettingsUi>,
    mut settings: ResMut<Settings>,
    mut keys: MessageReader<KeyboardInput>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut skip_frame: Local<bool>,
) {
    let Some(action) = ui.capturing else {
        *skip_frame = true;
        keys.clear();
        return;
    };
    // Ignore the click that started the capture.
    if *skip_frame {
        *skip_frame = false;
        keys.clear();
        return;
    }
    let mut chosen = None;
    for ev in keys.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }
        if ev.key_code == KeyCode::Escape {
            ui.capturing = None;
            return;
        }
        if Binding::is_bindable_key(ev.key_code) {
            chosen = Some(Binding::Key(ev.key_code));
        }
    }
    for m in [
        MouseButton::Left,
        MouseButton::Right,
        MouseButton::Middle,
        MouseButton::Back,
        MouseButton::Forward,
    ] {
        if mouse.just_pressed(m) {
            chosen = Some(Binding::Mouse(m));
        }
    }
    if let Some(b) = chosen {
        // Swap with whichever action already used this binding.
        let previous = settings.keybinds.get(action);
        let clash = Action::ALL
            .iter()
            .copied()
            .find(|a| *a != action && settings.keybinds.get(*a) == b);
        if let Some(other) = clash {
            settings.keybinds.0.insert(other, previous);
        }
        settings.keybinds.0.insert(action, b);
        ui.capturing = None;
    }
}

fn refresh_values(
    settings: Res<Settings>,
    ui: Res<SettingsUi>,
    mut values: Query<(&ValueText, &mut Text), Without<BindText>>,
    mut binds: Query<(&BindText, &mut Text), Without<ValueText>>,
) {
    for (ValueText(f), mut t) in &mut values {
        let s = field_text(&settings, *f);
        if t.0 != s {
            t.0 = s;
        }
    }
    for (BindText(a), mut t) in &mut binds {
        let s = if ui.capturing == Some(*a) {
            "press a key...".to_string()
        } else {
            settings.keybinds.get(*a).name()
        };
        if t.0 != s {
            t.0 = s;
        }
    }
}

fn preview_crosshair(
    mut commands: Commands,
    settings: Res<Settings>,
    q: Query<Entity, Added<CrosshairPreview>>,
    all: Query<Entity, With<CrosshairPreview>>,
) {
    if q.is_empty() && !settings.is_changed() {
        return;
    }
    for e in &all {
        commands.entity(e).despawn_children();
        commands.entity(e).with_children(|p| {
            super::spawn_crosshair_lines(p, &settings, 0.0);
        });
    }
}
