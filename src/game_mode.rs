//! Game modes, teams and match rules.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameMode {
    OneVOne,
    TwoVTwo,
    ThreeVThree,
    FourVFour,
    FiveVFive,
    FreeForAll,
}

impl GameMode {
    pub const ALL: [GameMode; 6] = [
        GameMode::OneVOne,
        GameMode::TwoVTwo,
        GameMode::ThreeVThree,
        GameMode::FourVFour,
        GameMode::FiveVFive,
        GameMode::FreeForAll,
    ];

    pub fn name(self) -> &'static str {
        match self {
            GameMode::OneVOne => "1v1",
            GameMode::TwoVTwo => "2v2",
            GameMode::ThreeVThree => "3v3",
            GameMode::FourVFour => "4v4",
            GameMode::FiveVFive => "5v5",
            GameMode::FreeForAll => "All vs All",
        }
    }

    pub fn is_ffa(self) -> bool {
        self == GameMode::FreeForAll
    }

    pub fn team_size(self) -> usize {
        match self {
            GameMode::OneVOne => 1,
            GameMode::TwoVTwo => 2,
            GameMode::ThreeVThree => 3,
            GameMode::FourVFour => 4,
            GameMode::FiveVFive => 5,
            GameMode::FreeForAll => 10,
        }
    }

    pub fn max_players(self) -> usize {
        if self.is_ffa() { 10 } else { self.team_size() * 2 }
    }

    /// Rounds needed to win the match (team modes).
    pub fn rounds_to_win(self) -> u32 {
        match self {
            GameMode::OneVOne | GameMode::TwoVTwo => 9,
            GameMode::ThreeVThree | GameMode::FourVFour => 11,
            GameMode::FiveVFive => 13,
            GameMode::FreeForAll => 0,
        }
    }

    pub fn next(self) -> GameMode {
        let i = Self::ALL.iter().position(|m| *m == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }

    pub fn prev(self) -> GameMode {
        let i = Self::ALL.iter().position(|m| *m == self).unwrap_or(0);
        Self::ALL[(i + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}

pub const FFA_KILL_LIMIT: u32 = 25;
pub const FFA_TIME_LIMIT: f32 = 600.0;
pub const FFA_RESPAWN_TIME: f32 = 3.0;
pub const FREEZE_TIME: f32 = 10.0;
pub const ROUND_TIME: f32 = 115.0;
pub const BUY_TIME: f32 = 20.0;
pub const ROUND_END_TIME: f32 = 5.0;
pub const MATCH_END_TIME: f32 = 10.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Team {
    Alpha,
    Bravo,
    /// Used in free-for-all.
    None,
}

impl Team {
    pub fn name(self) -> &'static str {
        match self {
            Team::Alpha => "Alpha",
            Team::Bravo => "Bravo",
            Team::None => "Solo",
        }
    }

    pub fn color(self) -> [f32; 3] {
        match self {
            Team::Alpha => [0.95, 0.55, 0.15],
            Team::Bravo => [0.25, 0.55, 0.95],
            Team::None => [0.7, 0.7, 0.7],
        }
    }

    pub fn index(self) -> usize {
        match self {
            Team::Alpha => 0,
            _ => 1,
        }
    }

    pub fn other(self) -> Team {
        match self {
            Team::Alpha => Team::Bravo,
            Team::Bravo => Team::Alpha,
            Team::None => Team::None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    Lobby,
    Freeze,
    Live,
    RoundEnd,
    MatchEnd,
}

pub mod economy {
    //! CS2 competitive economy values.
    pub const START_MONEY: u32 = 800;
    pub const MAX_MONEY: u32 = 16000;
    pub const ROUND_WIN: u32 = 3250;
    pub const LOSS_BONUS_BASE: u32 = 1400;
    pub const LOSS_BONUS_STEP: u32 = 500;
    pub const LOSS_BONUS_MAX: u32 = 3400;
    pub const FFA_START_MONEY: u32 = 4000;

    /// `streak` = consecutive rounds lost, starting at 1.
    pub fn loss_bonus(streak: u32) -> u32 {
        (LOSS_BONUS_BASE + LOSS_BONUS_STEP * streak.saturating_sub(1)).min(LOSS_BONUS_MAX)
    }
}
