//! Lobby room: team selection, host controls, and the jump into the match.

use bevy::prelude::*;

use super::widgets::*;
use crate::game_mode::{Phase, Team};
use crate::net::protocol::{Cmd, PlayerId};
use crate::net::{leave_lobby, ConnStatus, GameServer, NetClient};
use crate::states::{AppState, MenuMessage};

#[derive(Component, Clone, Copy)]
enum LobbyButton {
    Team(Team),
    ModePrev,
    ModeNext,
    Start,
    FriendlyFire,
    AddBot,
    RemoveBot,
    MaxPlayers(i32),
    BotDifficulty,
    Kick(PlayerId),
    Ban(PlayerId),
    Leave,
}

#[derive(Component)]
struct Roster;

#[derive(Component)]
struct HeaderText;

#[derive(Component)]
struct HostControls;

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(AppState::Lobby), spawn)
        .add_systems(
            Update,
            (handle, refresh, enter_match).run_if(in_state(AppState::Lobby)),
        )
        .add_systems(
            Update,
            watch_connection.run_if(|s: Res<State<AppState>>| matches!(s.get(), AppState::Lobby | AppState::InGame)),
        );
}

fn spawn(mut commands: Commands) {
    commands
        .spawn((
            DespawnOnExit(AppState::Lobby),
            Node {
                padding: UiRect::all(px(40)),
                ..screen_root()
            },
        ))
        .with_children(|root| {
            root.spawn(scroll_panel(px(720))).with_children(|p| {
                p.spawn((HeaderText, Text::new("Connecting..."), font(26.0), TextColor(ACCENT)));
                p.spawn((
                    Roster,
                    Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: px(16),
                        min_height: px(300),
                        margin: UiRect::vertical(px(10)),
                        ..default()
                    },
                ));
                p.spawn(row()).with_children(|r| {
                    button(r, "Join Alpha", LobbyButton::Team(Team::Alpha), px(150));
                    button(r, "Join Bravo", LobbyButton::Team(Team::Bravo), px(150));
                });
                p.spawn((
                    HostControls,
                    Node {
                        margin: UiRect::top(px(8)),
                        ..row()
                    },
                ))
                .with_children(|r| {
                    label(r, "Host:", 16.0, TEXT_DIM);
                    button_sized(r, "< Mode", LobbyButton::ModePrev, px(90), 38.0, 16.0);
                    button_sized(r, "Mode >", LobbyButton::ModeNext, px(90), 38.0, 16.0);
                    button_sized(r, "Friendly Fire", LobbyButton::FriendlyFire, px(140), 38.0, 16.0);
                });
                p.spawn((
                    HostControls,
                    Node {
                        margin: UiRect::top(px(4)),
                        ..row()
                    },
                ))
                .with_children(|r| {
                    label(r, "Bots:", 16.0, TEXT_DIM);
                    button_sized(r, "+ Add Bot", LobbyButton::AddBot, px(120), 38.0, 16.0);
                    button_sized(r, "- Remove Bot", LobbyButton::RemoveBot, px(140), 38.0, 16.0);
                    button_sized(r, "Difficulty", LobbyButton::BotDifficulty, px(110), 38.0, 16.0);
                    label(r, "Max players:", 16.0, TEXT_DIM);
                    button_sized(r, "-", LobbyButton::MaxPlayers(-1), px(36), 38.0, 16.0);
                    button_sized(r, "+", LobbyButton::MaxPlayers(1), px(36), 38.0, 16.0);
                    button_sized(r, "START MATCH", LobbyButton::Start, px(200), 44.0, 20.0);
                });
                p.spawn(Node {
                    margin: UiRect::top(px(8)),
                    ..row()
                })
                .with_children(|r| {
                    button(r, "Leave Lobby", LobbyButton::Leave, px(160));
                });
            });
        });
}

fn handle(
    mut commands: Commands,
    q: Clicked<LobbyButton>,
    mut client: Option<ResMut<NetClient>>,
    mut server: Option<ResMut<GameServer>>,
    mut next: ResMut<NextState<AppState>>,
) {
    for action in clicked(&q) {
        let Some(c) = client.as_mut() else { continue };
        let mode = c.snapshot.as_ref().map(|s| s.lobby.mode);
        let ff = c.snapshot.as_ref().is_some_and(|s| s.lobby.friendly_fire);
        match *action {
            LobbyButton::Team(t) => c.send_cmd(Cmd::SetTeam(t)),
            LobbyButton::ModePrev => {
                if let Some(m) = mode {
                    c.send_cmd(Cmd::SetMode(m.prev()));
                }
            }
            LobbyButton::ModeNext => {
                if let Some(m) = mode {
                    c.send_cmd(Cmd::SetMode(m.next()));
                }
            }
            LobbyButton::Start => c.send_cmd(Cmd::StartMatch),
            LobbyButton::FriendlyFire => c.send_cmd(Cmd::SetFriendlyFire(!ff)),
            LobbyButton::BotDifficulty => {
                let current = c.snapshot.as_ref().map(|s| s.lobby.bot_difficulty).unwrap_or_default();
                c.send_cmd(Cmd::SetBotDifficulty(current.next()));
            }
            LobbyButton::AddBot => c.send_cmd(Cmd::AddBot),
            LobbyButton::RemoveBot => c.send_cmd(Cmd::RemoveBot),
            LobbyButton::MaxPlayers(d) => {
                let current = c.snapshot.as_ref().map(|s| s.lobby.max_players).unwrap_or(10) as i32;
                c.send_cmd(Cmd::SetMaxPlayers((current + d).clamp(2, 10) as usize));
            }
            LobbyButton::Kick(id) => c.send_cmd(Cmd::Kick(id)),
            LobbyButton::Ban(id) => c.send_cmd(Cmd::Ban(id)),
            LobbyButton::Leave => {
                leave_lobby(&mut commands, Some(c), server.as_deref_mut());
                next.set(AppState::Browser);
                return;
            }
        }
    }
}

fn refresh(
    mut commands: Commands,
    client: Option<Res<NetClient>>,
    mut header: Query<&mut Text, With<HeaderText>>,
    roster: Query<Entity, With<Roster>>,
    mut host_controls: Query<&mut Node, With<HostControls>>,
    mut shown: Local<String>,
) {
    let Some(c) = client else { return };
    let Some(snap) = &c.snapshot else {
        if let Ok(mut t) = header.single_mut() {
            t.0 = match &c.status {
                ConnStatus::Connecting => "Connecting...".into(),
                ConnStatus::Failed(r) => r.clone(),
                ConnStatus::Connected => "Waiting for lobby data...".into(),
            };
        }
        return;
    };
    let me = c.my_id;
    let i_am_host = c.is_host();
    let mode = snap.lobby.mode;
    let head = format!(
        "{}  -  {}{}\nPlayers {}/{}  |  Friendly fire: {}  |  Bots: {}",
        snap.lobby.name,
        mode.name(),
        if snap.lobby.has_password { "  (password protected)" } else { "" },
        snap.lobby.players,
        snap.lobby.max_players,
        if snap.lobby.friendly_fire { "ON" } else { "off" },
        snap.lobby.bot_difficulty.name()
    );
    if let Ok(mut t) = header.single_mut()
        && t.0 != head
    {
        t.0 = head;
    }
    let d = if c.is_host() { Display::Flex } else { Display::None };
    for mut n in &mut host_controls {
        if n.display != d {
            n.display = d;
        }
    }
    let sig = format!(
        "{:?}",
        snap.players
            .iter()
            .map(|p| (p.id, &p.name, p.team, p.is_host))
            .collect::<Vec<_>>()
    ) + mode.name();
    if *shown == sig {
        return;
    }
    *shown = sig;
    let Ok(roster) = roster.single() else { return };
    commands.entity(roster).despawn_children();
    let columns: Vec<(String, Vec<Team>, Color)> = if mode.is_ffa() {
        vec![("Players".into(), vec![Team::None, Team::Alpha, Team::Bravo], TEXT)]
    } else {
        vec![
            ("Team Alpha".into(), vec![Team::Alpha], rgb(Team::Alpha.color())),
            ("Team Bravo".into(), vec![Team::Bravo], rgb(Team::Bravo.color())),
        ]
    };
    let cap = if mode.is_ffa() { mode.max_players() } else { mode.team_size() };
    commands.entity(roster).with_children(|r| {
        for (name, teams, color) in columns {
            r.spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: px(6),
                width: percent(50),
                ..default()
            })
            .with_children(|col| {
                let members: Vec<_> = snap.players.iter().filter(|p| teams.contains(&p.team)).collect();
                label(col, format!("{name}  ({}/{cap})", members.len()), 20.0, color);
                for p in members {
                    let mut text = p.name.clone();
                    if p.is_host {
                        text.push_str("  [host]");
                    }
                    if Some(p.id) == me {
                        text.push_str("  (you)");
                    }
                    col.spawn((
                        Node {
                            padding: UiRect::axes(px(10), px(6)),
                            border_radius: BorderRadius::all(px(4)),
                            justify_content: JustifyContent::SpaceBetween,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.05)),
                    ))
                    .with_children(|b| {
                        label(b, text, 18.0, TEXT);
                        if i_am_host && Some(p.id) != me {
                            b.spawn(row()).with_children(|r| {
                                button_sized(r, "Kick", LobbyButton::Kick(p.id), px(56), 26.0, 14.0);
                                button_sized(r, "Ban", LobbyButton::Ban(p.id), px(52), 26.0, 14.0);
                            });
                        }
                    });
                }
            });
        }
    });
}

fn enter_match(client: Option<Res<NetClient>>, mut next: ResMut<NextState<AppState>>) {
    if let Some(snap) = client.as_ref().and_then(|c| c.snapshot.as_ref())
        && snap.phase != Phase::Lobby
    {
        next.set(AppState::InGame);
    }
}

fn watch_connection(
    mut commands: Commands,
    client: Option<Res<NetClient>>,
    mut server: Option<ResMut<GameServer>>,
    state: Res<State<AppState>>,
    mut next: ResMut<NextState<AppState>>,
    mut msg: ResMut<MenuMessage>,
) {
    let failed = match client.as_deref() {
        None => Some("Disconnected".to_string()),
        Some(c) => match &c.status {
            ConnStatus::Failed(r) => Some(r.clone()),
            _ => None,
        },
    };
    if let Some(reason) = failed {
        leave_lobby(&mut commands, client.as_deref(), server.as_deref_mut());
        msg.0 = Some(reason);
        next.set(AppState::Browser);
    } else if *state.get() == AppState::InGame
        && client
            .as_ref()
            .and_then(|c| c.snapshot.as_ref())
            .is_some_and(|s| s.phase == Phase::Lobby)
    {
        next.set(AppState::Lobby);
    }
}
