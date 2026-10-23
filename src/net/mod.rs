//! UDP multiplayer: a listen server hosted by the lobby owner and a client
//! used by everyone (the host connects to their own server over loopback).

pub mod client;
pub mod protocol;
pub mod server;

#[cfg(test)]
mod tests;

use bevy::prelude::*;

pub use client::{ConnStatus, LobbyBrowser, NetClient, NetEvent};
pub use server::GameServer;

pub struct NetPlugin;

impl Plugin for NetPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<NetEvent>()
            .add_plugins(server::ServerPlugin)
            .add_systems(PreUpdate, client::client_receive.after(server::ServerSet))
            .add_systems(PostUpdate, client::client_send);
    }
}

/// Disconnects from the current lobby and shuts the server down if we host it.
pub fn leave_lobby(commands: &mut Commands, client: Option<&NetClient>, server: Option<&mut GameServer>) {
    if let Some(c) = client {
        c.leave();
    }
    if let Some(s) = server {
        s.shutdown();
    }
    commands.remove_resource::<NetClient>();
    commands.remove_resource::<GameServer>();
}
