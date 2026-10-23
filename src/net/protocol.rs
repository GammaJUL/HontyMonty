//! Wire format. JSON over UDP keeps this easy to debug; see TODO.md for a binary codec.
//!
//! Reliability model:
//! * Client -> server `Cmd`s carry a sequence number and are resent until the
//!   server acknowledges them in a snapshot (`ack_cmd`).
//! * Server -> client `GameEvent`s are repeated in every snapshot for a short
//!   window; clients drop the ids they have already seen.
//! * Everything else (positions, health, round state) is plain state that the
//!   next packet overwrites.

use serde::{Deserialize, Serialize};

use crate::game_mode::{GameMode, Phase, Team};
use crate::profile::{Appearance, EquippedSkin};
use crate::weapons::{DamageSource, GrenadeKind, ShopItem, WeaponId};

pub const PROTOCOL_VERSION: u32 = 1;
pub const DEFAULT_PORT: u16 = 27015;
/// Ports scanned by the lobby browser.
pub const PORT_RANGE: std::ops::Range<u16> = 27015..27020;
pub const MAX_PACKET: usize = 64 * 1024;

pub type PlayerId = u8;
pub type V3 = [f32; 3];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LobbyInfo {
    pub name: String,
    pub has_password: bool,
    pub mode: GameMode,
    pub players: usize,
    pub max_players: usize,
    pub in_game: bool,
    pub host: String,
}

/// Reliable commands from a client.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Cmd {
    SetTeam(Team),
    /// Host only.
    SetMode(GameMode),
    /// Host only.
    StartMatch,
    /// Host only: back to the lobby screen.
    EndMatch,
    Buy(ShopItem),
    Fire {
        weapon: WeaponId,
        origin: V3,
        dirs: Vec<V3>,
    },
    Throw {
        kind: GrenadeKind,
        origin: V3,
        vel: V3,
    },
    Chat(String),
}

/// Unreliable per-tick client state.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ClientState {
    pub pos: V3,
    pub yaw: f32,
    pub pitch: f32,
    pub crouch: bool,
    pub weapon: Option<WeaponId>,
    pub grenade: Option<GrenadeKind>,
    pub moving: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum C2S {
    Discover,
    Join {
        version: u32,
        name: String,
        password: String,
        appearance: Appearance,
        skins: Vec<EquippedSkin>,
    },
    Update {
        state: ClientState,
        cmds: Vec<(u32, Cmd)>,
    },
    Leave,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum S2C {
    LobbyInfo(LobbyInfo),
    Accepted { id: PlayerId },
    Rejected { reason: String },
    Snapshot(Box<Snapshot>),
    Kicked { reason: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum GameEvent {
    Shot {
        shooter: PlayerId,
        weapon: WeaponId,
        from: V3,
        to: V3,
    },
    Hit {
        attacker: PlayerId,
        victim: PlayerId,
        damage: i32,
        headshot: bool,
    },
    Kill {
        killer: PlayerId,
        victim: PlayerId,
        source: DamageSource,
        headshot: bool,
    },
    GrenadeThrown {
        id: u32,
        kind: GrenadeKind,
        owner: PlayerId,
        pos: V3,
        vel: V3,
    },
    GrenadeDetonate {
        id: u32,
        kind: GrenadeKind,
        pos: V3,
    },
    FireOut {
        id: u32,
    },
    Bought {
        player: PlayerId,
        item: ShopItem,
    },
    BuyDenied {
        player: PlayerId,
        reason: String,
    },
    RoundStart {
        round: u32,
    },
    RoundEnd {
        winner: Option<Team>,
        reason: String,
    },
    MatchEnd {
        winner: String,
    },
    Chat {
        from: String,
        text: String,
    },
    Info(String),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NetPlayer {
    pub id: PlayerId,
    pub name: String,
    pub team: Team,
    pub pos: V3,
    pub yaw: f32,
    pub pitch: f32,
    pub crouch: bool,
    pub moving: bool,
    pub weapon: Option<WeaponId>,
    pub grenade: Option<GrenadeKind>,
    pub alive: bool,
    pub health: i32,
    pub armor: i32,
    pub helmet: bool,
    pub money: u32,
    pub kills: u32,
    pub deaths: u32,
    pub headshots: u32,
    pub damage: u32,
    pub primary: Option<WeaponId>,
    pub secondary: Option<WeaponId>,
    pub grenades: Vec<GrenadeKind>,
    /// Incremented each time the server (re)spawns this player.
    pub spawn_seq: u32,
    pub spawn_pos: V3,
    pub spawn_yaw: f32,
    pub is_host: bool,
}

/// Cosmetics, sent less often than the rest.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NetProfile {
    pub id: PlayerId,
    pub appearance: Appearance,
    pub skins: Vec<EquippedSkin>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub your_id: PlayerId,
    pub ack_cmd: u32,
    pub lobby: LobbyInfo,
    pub phase: Phase,
    pub phase_time_left: f32,
    pub buy_time_left: f32,
    pub round: u32,
    pub score: [u32; 2],
    pub players: Vec<NetPlayer>,
    pub profiles: Option<Vec<NetProfile>>,
    pub events: Vec<(u32, GameEvent)>,
}

pub fn encode<T: Serialize>(msg: &T) -> Vec<u8> {
    serde_json::to_vec(msg).unwrap_or_default()
}

pub fn decode<'a, T: Deserialize<'a>>(bytes: &'a [u8]) -> Option<T> {
    serde_json::from_slice(bytes).ok()
}
