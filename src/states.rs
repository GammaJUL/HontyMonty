use bevy::prelude::*;

#[derive(States, Default, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum AppState {
    /// Main menu with the character showcase.
    #[default]
    MainMenu,
    /// Character customization.
    Customize,
    /// Case opening and skin inventory.
    Inventory,
    /// Create / browse / join lobbies.
    Browser,
    /// Inside a lobby, waiting for the host to start.
    Lobby,
    /// Playing a match.
    InGame,
}

impl AppState {
    pub fn is_menu(self) -> bool {
        self != AppState::InGame
    }
}

/// A message shown on the next menu screen (e.g. "Wrong password").
#[derive(Resource, Default)]
pub struct MenuMessage(pub Option<String>);
