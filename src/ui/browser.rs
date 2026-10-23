//! Create a lobby (optional password) or browse/join LAN lobbies.

use std::net::{SocketAddr, ToSocketAddrs};

use bevy::prelude::*;

use super::widgets::*;
use crate::game_mode::GameMode;
use crate::net::protocol::{C2S, DEFAULT_PORT, PROTOCOL_VERSION};
use crate::net::{GameServer, LobbyBrowser, NetClient};
use crate::profile::Profile;
use crate::settings::Settings;
use crate::states::{AppState, MenuMessage};

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Input {
    PlayerName,
    LobbyName,
    Password,
    JoinPassword,
    Address,
}

#[derive(Component, Clone, Copy)]
enum BrowserButton {
    ModePrev,
    ModeNext,
    Create,
    Select(SocketAddr),
    Join,
    Refresh,
    Back,
}

#[derive(Component)]
struct LobbyList;

#[derive(Component)]
struct ModeText;

#[derive(Component)]
struct StatusText;

#[derive(Resource)]
struct BrowserUi {
    mode: GameMode,
    selected: Option<SocketAddr>,
    status: String,
    shown: Vec<String>,
}

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(AppState::Browser), spawn)
        .add_systems(OnExit(AppState::Browser), |mut c: Commands| {
            c.remove_resource::<LobbyBrowser>();
            c.remove_resource::<BrowserUi>();
        })
        .add_systems(
            Update,
            (discover, rebuild_list, handle, refresh_texts)
                .chain()
                .run_if(in_state(AppState::Browser)),
        );
}

pub fn join_message(settings: &Settings, profile: &Profile, password: String) -> C2S {
    C2S::Join {
        version: PROTOCOL_VERSION,
        name: settings.player_name.clone(),
        password,
        appearance: profile.appearance.clone(),
        skins: profile.loadout(),
    }
}

fn spawn(mut commands: Commands, settings: Res<Settings>, mut msg: ResMut<MenuMessage>) {
    let status = msg.0.take().unwrap_or_default();
    commands.insert_resource(LobbyBrowser::default());
    commands.insert_resource(BrowserUi {
        mode: GameMode::FiveVFive,
        selected: None,
        status,
        shown: Vec::new(),
    });
    let default_lobby = format!("{}'s lobby", settings.player_name);
    commands
        .spawn((
            DespawnOnExit(AppState::Browser),
            Node {
                padding: UiRect::all(px(40)),
                column_gap: px(24),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::FlexStart,
                ..screen_root()
            },
        ))
        .with_children(|root| {
            // Left: profile + create.
            root.spawn(panel(px(420))).with_children(|p| {
                title(p, "Play");
                label(p, "Your name", 16.0, TEXT_DIM);
                text_input(p, &settings.player_name, "Player name", false, 20, Input::PlayerName);
                p.spawn(Node {
                    height: px(14),
                    ..default()
                });
                label(p, "CREATE LOBBY", 20.0, ACCENT);
                label(p, "Lobby name", 16.0, TEXT_DIM);
                text_input(p, &default_lobby, "Lobby name", false, 28, Input::LobbyName);
                label(p, "Password (optional)", 16.0, TEXT_DIM);
                text_input(p, "", "Leave empty for an open lobby", true, 24, Input::Password);
                label(p, "Game mode", 16.0, TEXT_DIM);
                p.spawn(row()).with_children(|r| {
                    button_sized(r, "<", BrowserButton::ModePrev, px(40), 38.0, 18.0);
                    r.spawn((
                        ModeText,
                        Text::new(""),
                        font(20.0),
                        TextColor(TEXT),
                        Node {
                            width: px(200),
                            justify_content: JustifyContent::Center,
                            ..default()
                        },
                        TextLayout::justify(Justify::Center),
                    ));
                    button_sized(r, ">", BrowserButton::ModeNext, px(40), 38.0, 18.0);
                });
                button_sized(p, "Create Lobby", BrowserButton::Create, percent(100), 46.0, 20.0);
                p.spawn(Node {
                    height: px(10),
                    ..default()
                });
                button(p, "Back", BrowserButton::Back, px(120));
            });
            // Right: browser + join.
            root.spawn(panel(px(560))).with_children(|p| {
                p.spawn(Node {
                    justify_content: JustifyContent::SpaceBetween,
                    ..row()
                })
                .with_children(|r| {
                    label(r, "LAN LOBBIES", 20.0, ACCENT);
                    button_sized(r, "Refresh", BrowserButton::Refresh, px(100), 32.0, 16.0);
                });
                p.spawn((
                    LobbyList,
                    Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: px(6),
                        min_height: px(240),
                        ..default()
                    },
                ));
                label(p, "Lobby password (if locked)", 16.0, TEXT_DIM);
                text_input(p, "", "Password", true, 24, Input::JoinPassword);
                label(p, "Or connect directly (ip or ip:port)", 16.0, TEXT_DIM);
                text_input(p, "", "e.g. 192.168.1.20:27015", false, 40, Input::Address);
                button_sized(p, "Join Lobby", BrowserButton::Join, percent(100), 46.0, 20.0);
                p.spawn((
                    StatusText,
                    Text::new(""),
                    font(16.0),
                    TextColor(Color::srgb(1.0, 0.55, 0.45)),
                ));
            });
        });
}

fn discover(time: Res<Time<Real>>, browser: Option<ResMut<LobbyBrowser>>) {
    if let Some(mut b) = browser {
        b.update(time.delta_secs());
    }
}

fn rebuild_list(
    mut commands: Commands,
    browser: Option<Res<LobbyBrowser>>,
    ui: Option<ResMut<BrowserUi>>,
    list: Query<Entity, With<LobbyList>>,
) {
    let (Some(browser), Some(mut ui), Ok(list)) = (browser, ui, list.single()) else {
        return;
    };
    let rows: Vec<String> = browser
        .found
        .iter()
        .map(|f| {
            format!(
                "{}{}  |  {}  |  {}/{}{}",
                if f.info.has_password { "[locked] " } else { "" },
                f.info.name,
                f.info.mode.name(),
                f.info.players,
                f.info.max_players,
                if f.info.in_game { "  |  in match" } else { "" },
            )
        })
        .collect();
    let mut signature = rows.clone();
    signature.push(format!("{:?}", ui.selected));
    if signature == ui.shown {
        return;
    }
    ui.shown = signature;
    commands.entity(list).despawn_children();
    let selected = ui.selected;
    commands.entity(list).with_children(|l| {
        if browser.found.is_empty() {
            label(l, "Searching for lobbies on your network...", 16.0, TEXT_DIM);
        }
        for (f, text) in browser.found.iter().zip(rows) {
            let e = button_sized(l, text, BrowserButton::Select(f.addr), percent(100), 38.0, 16.0);
            if selected == Some(f.addr) {
                l.commands().entity(e).insert(Selected);
            }
        }
    });
}

fn parse_addr(s: &str) -> Option<SocketAddr> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let with_port = if s.contains(':') { s.to_string() } else { format!("{s}:{DEFAULT_PORT}") };
    with_port.to_socket_addrs().ok()?.find(|a| a.is_ipv4())
}

#[allow(clippy::too_many_arguments)]
fn handle(
    mut commands: Commands,
    q: Clicked<BrowserButton>,
    inputs: Query<(&Input, &TextInput)>,
    mut ui: Option<ResMut<BrowserUi>>,
    mut browser: Option<ResMut<LobbyBrowser>>,
    mut settings: ResMut<Settings>,
    profile: Res<Profile>,
    mut next: ResMut<NextState<AppState>>,
) {
    let Some(ui) = ui.as_mut() else { return };
    let value = |which: Input| {
        inputs
            .iter()
            .find(|(i, _)| **i == which)
            .map(|(_, t)| t.value.trim().to_string())
            .unwrap_or_default()
    };
    for action in clicked(&q) {
        let name = value(Input::PlayerName);
        if !name.is_empty() && name != settings.player_name {
            settings.player_name = name;
        }
        match *action {
            BrowserButton::ModePrev => ui.mode = ui.mode.prev(),
            BrowserButton::ModeNext => ui.mode = ui.mode.next(),
            BrowserButton::Refresh => {
                if let Some(b) = browser.as_mut() {
                    b.refresh();
                }
            }
            BrowserButton::Back => next.set(AppState::MainMenu),
            BrowserButton::Select(addr) => ui.selected = Some(addr),
            BrowserButton::Create => {
                let mut lobby = value(Input::LobbyName);
                if lobby.is_empty() {
                    lobby = format!("{}'s lobby", settings.player_name);
                }
                let password = value(Input::Password);
                match GameServer::host(lobby, password.clone(), ui.mode, DEFAULT_PORT) {
                    Ok(server) => {
                        let addr = SocketAddr::from(([127, 0, 0, 1], server.port));
                        match NetClient::connect(addr, join_message(&settings, &profile, password)) {
                            Ok(client) => {
                                commands.insert_resource(server);
                                commands.insert_resource(client);
                                next.set(AppState::Lobby);
                            }
                            Err(e) => ui.status = format!("Could not open a socket: {e}"),
                        }
                    }
                    Err(e) => ui.status = format!("Could not host: {e}"),
                }
            }
            BrowserButton::Join => {
                let target = parse_addr(&value(Input::Address)).or(ui.selected);
                let Some(addr) = target else {
                    ui.status = "Select a lobby or enter an address first".into();
                    continue;
                };
                let password = value(Input::JoinPassword);
                match NetClient::connect(addr, join_message(&settings, &profile, password)) {
                    Ok(client) => {
                        commands.insert_resource(client);
                        next.set(AppState::Lobby);
                    }
                    Err(e) => ui.status = format!("Could not connect: {e}"),
                }
            }
        }
    }
}

fn refresh_texts(
    ui: Option<Res<BrowserUi>>,
    mut mode: Query<&mut Text, (With<ModeText>, Without<StatusText>)>,
    mut status: Query<&mut Text, (With<StatusText>, Without<ModeText>)>,
) {
    let Some(ui) = ui else { return };
    for mut t in &mut mode {
        let s = ui.mode.name().to_string();
        if t.0 != s {
            t.0 = s;
        }
    }
    for mut t in &mut status {
        if t.0 != ui.status {
            t.0 = ui.status.clone();
        }
    }
}
