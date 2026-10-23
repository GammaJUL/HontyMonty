//! In-match text chat: Y for everyone, U for your team. Enter sends, Esc cancels.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::ButtonState;
use bevy::prelude::*;

use super::{GameSet, GameUi, InGameEntity};
use crate::keybinds::{Action, Controls};
use crate::net::protocol::Cmd;
use crate::net::NetClient;
use crate::states::AppState;
use crate::ui::settings_menu::SettingsOpen;
use crate::ui::widgets::*;

const MAX_LEN: usize = 120;

#[derive(Resource, Default)]
struct Draft {
    text: String,
    team: bool,
}

#[derive(Component)]
struct ChatLine;

pub fn plugin(app: &mut App) {
    app.init_resource::<Draft>()
        .add_systems(OnEnter(AppState::InGame), spawn_line)
        .add_systems(Update, (type_chat, show_line).chain().in_set(GameSet::Input));
}

fn spawn_line(mut commands: Commands) {
    commands.spawn((
        InGameEntity,
        ChatLine,
        Text::new(""),
        font(18.0),
        TextColor(TEXT),
        TextShadow::default(),
        Visibility::Hidden,
        Node {
            position_type: PositionType::Absolute,
            left: px(16),
            bottom: px(112),
            padding: UiRect::axes(px(8), px(4)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
    ));
}

fn type_chat(
    controls: Controls,
    mut keys: MessageReader<KeyboardInput>,
    mut ui: ResMut<GameUi>,
    settings_open: Res<SettingsOpen>,
    mut draft: ResMut<Draft>,
    client: Option<ResMut<NetClient>>,
) {
    if !ui.chat_open {
        // The key that opens chat must not end up in the message.
        keys.clear();
        let all = controls.just_pressed(Action::Chat);
        let team = controls.just_pressed(Action::TeamChat);
        if (all || team) && !ui.any_open(&settings_open) {
            ui.chat_open = true;
            draft.text.clear();
            draft.team = team && !all;
        }
        return;
    }
    for ev in keys.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }
        match &ev.logical_key {
            Key::Enter => {
                let text = draft.text.trim().to_string();
                if !text.is_empty()
                    && let Some(mut c) = client
                {
                    c.send_cmd(if draft.team { Cmd::TeamChat(text) } else { Cmd::Chat(text) });
                }
                ui.chat_open = false;
                return;
            }
            Key::Backspace => {
                draft.text.pop();
            }
            Key::Space => {
                if draft.text.chars().count() < MAX_LEN {
                    draft.text.push(' ');
                }
            }
            Key::Character(s) => {
                for ch in s.chars() {
                    if !ch.is_control() && draft.text.chars().count() < MAX_LEN {
                        draft.text.push(ch);
                    }
                }
            }
            _ => {}
        }
    }
}

fn show_line(
    time: Res<Time>,
    ui: Res<GameUi>,
    draft: Res<Draft>,
    mut q: Query<(&mut Text, &mut Visibility), With<ChatLine>>,
) {
    let Ok((mut text, mut vis)) = q.single_mut() else { return };
    let want = if ui.chat_open { Visibility::Inherited } else { Visibility::Hidden };
    if *vis != want {
        *vis = want;
    }
    if ui.chat_open {
        let caret = if (time.elapsed_secs() * 2.0) as u32 % 2 == 0 { "|" } else { " " };
        let prefix = if draft.team { "(Team) Say" } else { "Say" };
        let s = format!("{prefix}: {}{caret}", draft.text);
        if text.0 != s {
            text.0 = s;
        }
    }
}
