//! Escape menu: resume, settings, back to lobby (host), leave.

use bevy::prelude::*;

use super::{GameSet, GameUi, InGameEntity};
use crate::net::protocol::Cmd;
use crate::net::{leave_lobby, GameServer, NetClient};
use crate::states::AppState;
use crate::ui::settings_menu::SettingsOpen;
use crate::ui::widgets::*;

#[derive(Component)]
struct PauseRoot;

#[derive(Component, Clone, Copy)]
enum PauseButton {
    Resume,
    Settings,
    EndMatch,
    Leave,
    Quit,
}

pub fn plugin(app: &mut App) {
    app.add_systems(Update, (escape, show, handle).chain().in_set(GameSet::Input));
}

fn escape(keys: Res<ButtonInput<KeyCode>>, mut ui: ResMut<GameUi>, mut open: ResMut<SettingsOpen>) {
    if !keys.just_pressed(KeyCode::Escape) {
        return;
    }
    if ui.chat_open {
        ui.chat_open = false;
    } else if open.0 {
        open.0 = false;
    } else if ui.buy_open {
        ui.buy_open = false;
    } else {
        ui.pause_open = !ui.pause_open;
    }
}

fn show(
    mut commands: Commands,
    ui: Res<GameUi>,
    client: Option<Res<NetClient>>,
    root: Query<Entity, With<PauseRoot>>,
) {
    let exists = !root.is_empty();
    if ui.pause_open && !exists {
        let host = client.as_ref().is_some_and(|c| c.is_host());
        commands
            .spawn((
                InGameEntity,
                PauseRoot,
                GlobalZIndex(60),
                Node {
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..screen_root()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
            ))
            .with_children(|root| {
                root.spawn(panel(px(340))).with_children(|p| {
                    title(p, "Paused");
                    label(p, "The match keeps running.", 14.0, TEXT_DIM);
                    button(p, "Resume", PauseButton::Resume, percent(100));
                    button(p, "Settings", PauseButton::Settings, percent(100));
                    if host {
                        button(p, "End Match (back to lobby)", PauseButton::EndMatch, percent(100));
                    }
                    button(p, "Leave Lobby", PauseButton::Leave, percent(100));
                    button(p, "Quit Game", PauseButton::Quit, percent(100));
                });
            });
    } else if !ui.pause_open && exists {
        for e in &root {
            commands.entity(e).despawn();
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn handle(
    mut commands: Commands,
    q: Clicked<PauseButton>,
    mut ui: ResMut<GameUi>,
    mut open: ResMut<SettingsOpen>,
    mut client: Option<ResMut<NetClient>>,
    mut server: Option<ResMut<GameServer>>,
    mut next: ResMut<NextState<AppState>>,
    mut exit: MessageWriter<AppExit>,
) {
    for b in clicked(&q) {
        match *b {
            PauseButton::Resume => ui.pause_open = false,
            PauseButton::Settings => open.0 = true,
            PauseButton::EndMatch => {
                if let Some(c) = client.as_mut() {
                    c.send_cmd(Cmd::EndMatch);
                }
                ui.pause_open = false;
            }
            PauseButton::Leave => {
                leave_lobby(&mut commands, client.as_deref(), server.as_deref_mut());
                next.set(AppState::MainMenu);
            }
            PauseButton::Quit => {
                leave_lobby(&mut commands, client.as_deref(), server.as_deref_mut());
                exit.write(AppExit::Success);
            }
        }
    }
}
