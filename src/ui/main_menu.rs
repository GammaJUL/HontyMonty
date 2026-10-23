use bevy::prelude::*;

use super::settings_menu::SettingsOpen;
use super::widgets::*;
use crate::profile::Profile;
use crate::settings::Settings;
use crate::states::{AppState, MenuMessage};

#[derive(Component, Clone, Copy)]
enum MenuButton {
    Play,
    Customize,
    Inventory,
    Settings,
    Quit,
}

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(AppState::MainMenu), spawn)
        .add_systems(Update, handle.run_if(in_state(AppState::MainMenu)));
}

fn spawn(mut commands: Commands, settings: Res<Settings>, profile: Res<Profile>, mut msg: ResMut<MenuMessage>) {
    let message = msg.0.take();
    commands
        .spawn((
            DespawnOnExit(AppState::MainMenu),
            Node {
                padding: UiRect::all(px(60)),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                ..screen_root()
            },
        ))
        .with_children(|root| {
            root.spawn((
                Text::new("HONTY MONTY"),
                font(64.0),
                TextColor(ACCENT),
            ));
            root.spawn((
                Text::new("tactical shooter"),
                font(20.0),
                TextColor(TEXT_DIM),
                Node {
                    margin: UiRect::bottom(px(30)),
                    ..default()
                },
            ));
            root.spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: px(10),
                width: px(320),
                ..default()
            })
            .with_children(|col| {
                button_sized(col, "PLAY", MenuButton::Play, percent(100), 54.0, 24.0);
                button(col, "Customize Character", MenuButton::Customize, percent(100));
                button(col, "Cases & Inventory", MenuButton::Inventory, percent(100));
                button(col, "Settings", MenuButton::Settings, percent(100));
                button(col, "Quit", MenuButton::Quit, percent(100));
            });
            root.spawn((
                Text::new(format!(
                    "Signed in as {}  |  {} items  |  {} cases opened",
                    settings.player_name,
                    profile.inventory.len(),
                    profile.cases_opened
                )),
                font(16.0),
                TextColor(TEXT_DIM),
                Node {
                    margin: UiRect::top(px(24)),
                    ..default()
                },
            ));
            if let Some(m) = message {
                root.spawn((
                    Text::new(m),
                    font(18.0),
                    TextColor(Color::srgb(1.0, 0.45, 0.4)),
                    Node {
                        margin: UiRect::top(px(10)),
                        ..default()
                    },
                ));
            }
        });
}

fn handle(
    q: Clicked<MenuButton>,
    mut next: ResMut<NextState<AppState>>,
    mut open: ResMut<SettingsOpen>,
    mut exit: MessageWriter<AppExit>,
) {
    for action in clicked(&q) {
        match action {
            MenuButton::Play => next.set(AppState::Browser),
            MenuButton::Customize => next.set(AppState::Customize),
            MenuButton::Inventory => next.set(AppState::Inventory),
            MenuButton::Settings => open.0 = true,
            MenuButton::Quit => {
                exit.write(AppExit::Success);
            }
        }
    }
}
