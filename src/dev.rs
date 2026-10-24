//! Developer shortcuts driven by environment variables:
//!
//! * `HONTY_DEV_STATE=customize|inventory|browser` jumps straight to a screen.
//! * `HONTY_DEV_STATE=ingame` hosts a lobby and starts the match right away.
//! * `HONTY_DEV_STATE=host` hosts a lobby and starts once a second player joined;
//!   `HONTY_DEV_STATE=join` (in a second instance) joins 127.0.0.1:27015.
//! * `HONTY_DEV_BOTS=<n>` adds that many bots before the dev match starts.
//! * `HONTY_DEV_MODE=1v1|2v2|5v5|ffa` picks the mode when hosting.
//! * `HONTY_DEV_SETTINGS=1` opens the settings overlay.
//! * `HONTY_DEV_SMOKE=<meters>` pops a smoke that far in front of the spawn (0 = on top of you).
//! * `HONTY_DEV_LOOK=<facial hair>,<accessory>` (indices) previews character options without saving.
//! * `HONTY_DEV_OPEN=1` opens one case (inventory) or the buy menu (in game).
//! * `HONTY_DEV_CRASH=1` crashes on purpose to test the crash reporter.
//! * `HONTY_DEV_SCREENSHOT=path.png` saves a screenshot after ~7 s and exits.

use std::net::SocketAddr;

use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};

use crate::game::remote::RemotePlayer;
use crate::game::player::LocalPlayer;
use crate::game::GameUi;
use crate::game_mode::GameMode;
use crate::net::protocol::{Cmd, GameEvent, DEFAULT_PORT};
use crate::net::{GameServer, NetClient, NetEvent};
use crate::weapons::GrenadeKind;
use crate::profile::Profile;
use crate::settings::Settings;
use crate::states::AppState;
use crate::ui::inventory::DevAutoOpen;

pub struct DevPlugin;

impl Plugin for DevPlugin {
    fn build(&self, app: &mut App) {
        // Lets you check the crash reporter (error box + crash.log).
        if std::env::var("HONTY_DEV_CRASH").is_ok() {
            panic!("HONTY_DEV_CRASH is set: this is a test crash");
        }
        if std::env::var("HONTY_DEV_STATE").is_ok() || std::env::var("HONTY_DEV_SCREENSHOT").is_ok() {
            app.add_systems(Update, drive);
        }
    }
}

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_default()
}

#[allow(clippy::too_many_arguments)]
fn drive(
    mut commands: Commands,
    time: Res<Time<Real>>,
    settings: Res<Settings>,
    mut profile: ResMut<Profile>,
    client: Option<ResMut<NetClient>>,
    state: Res<State<AppState>>,
    remotes: Query<&RemotePlayer>,
    mut ui: ResMut<GameUi>,
    mut next: ResMut<NextState<AppState>>,
    mut exit: MessageWriter<AppExit>,
    mut step: Local<u32>,
    mut started: Local<bool>,
    mut net_events: MessageWriter<NetEvent>,
    me: Query<&Transform, With<LocalPlayer>>,
) {
    let t = time.elapsed_secs();
    let target = env("HONTY_DEV_STATE");
    if *step == 0 && t > 1.0 {
        *step = 1;
        if let Some((f, a)) = env("HONTY_DEV_LOOK").split_once(',') {
            use crate::profile::{Accessory, FacialHair};
            let look = &mut profile.appearance;
            look.facial_hair = FacialHair::ALL[f.parse::<usize>().unwrap_or(0) % FacialHair::ALL.len()];
            look.accessory = Accessory::ALL[a.parse::<usize>().unwrap_or(0) % Accessory::ALL.len()];
        }
        match target.as_str() {
            "customize" => next.set(AppState::Customize),
            "inventory" => {
                next.set(AppState::Inventory);
                if !env("HONTY_DEV_OPEN").is_empty() {
                    commands.insert_resource(DevAutoOpen);
                }
            }
            "browser" => next.set(AppState::Browser),
            "ingame" | "host" => {
                let mode = match env("HONTY_DEV_MODE").as_str() {
                    "ffa" => GameMode::FreeForAll,
                    "2v2" => GameMode::TwoVTwo,
                    "5v5" => GameMode::FiveVFive,
                    _ => GameMode::OneVOne,
                };
                if let Ok(server) = GameServer::host("Dev lobby".into(), env("HONTY_DEV_PASSWORD"), mode, DEFAULT_PORT) {
                    let addr = SocketAddr::from(([127, 0, 0, 1], server.port));
                    let join = crate::ui::browser::join_message(&settings, &profile, env("HONTY_DEV_PASSWORD"));
                    if let Ok(c) = NetClient::connect(addr, join) {
                        commands.insert_resource(server);
                        commands.insert_resource(c);
                        next.set(AppState::Lobby);
                    }
                }
            }
            "join" => {
                let addr = SocketAddr::from(([127, 0, 0, 1], DEFAULT_PORT));
                let join = crate::ui::browser::join_message(&settings, &profile, env("HONTY_DEV_PASSWORD"));
                if let Ok(c) = NetClient::connect(addr, join) {
                    commands.insert_resource(c);
                    next.set(AppState::Lobby);
                }
            }
            _ => {}
        }
    }
    if let Some(mut c) = client
        && !*started
    {
        let players = c.snapshot.as_ref().map(|s| s.players.len()).unwrap_or(0);
        let go = match target.as_str() {
            "ingame" => t > 2.0,
            "host" => players >= 2,
            _ => false,
        };
        if go {
            *started = true;
            for _ in 0..env("HONTY_DEV_BOTS").parse::<usize>().unwrap_or(0) {
                c.send_cmd(Cmd::AddBot);
            }
            c.send_cmd(Cmd::StartMatch);
        }
    }
    if *step == 1 && t > 4.0 {
        *step = 2;
        if let Ok(offset) = env("HONTY_DEV_SMOKE").parse::<f32>()
            && let Ok(t) = me.single()
        {
            let pos = t.translation + t.rotation * Vec3::NEG_Z * offset;
            net_events.write(NetEvent(GameEvent::GrenadeDetonate {
                id: 9999,
                kind: GrenadeKind::Smoke,
                pos: pos.to_array(),
            }));
        }
        if !env("HONTY_DEV_SETTINGS").is_empty() {
            commands.insert_resource(crate::ui::settings_menu::SettingsOpen(true));
        }
        if !env("HONTY_DEV_OPEN").is_empty() && *state.get() == AppState::InGame {
            ui.buy_open = true;
        }
    }
    let shot = env("HONTY_DEV_SCREENSHOT");
    let at: f32 = env("HONTY_DEV_SCREENSHOT_AT").parse().unwrap_or(7.0);
    if *step == 2 && t > at {
        *step = 3;
        info!(
            "dev: state={:?} remote_players={} status={:?}",
            state.get(),
            remotes.iter().count(),
            "ok"
        );
        if !shot.is_empty() {
            commands.spawn(Screenshot::primary_window()).observe(save_to_disk(shot.clone()));
        }
    }
    if *step == 3 && t > at + 1.5 && !shot.is_empty() {
        exit.write(AppExit::Success);
    }
}
