//! Rebindable controls. Bindings are stored by name so the settings file stays readable.

use std::collections::BTreeMap;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::settings::Settings;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Action {
    MoveForward,
    MoveBack,
    MoveLeft,
    MoveRight,
    Jump,
    Crouch,
    Walk,
    Fire,
    Scope,
    Reload,
    Primary,
    Secondary,
    Knife,
    Grenades,
    BuyMenu,
    Scoreboard,
    Chat,
    TeamChat,
    Inspect,
    Drop,
}

impl Action {
    pub const ALL: [Action; 20] = [
        Action::MoveForward,
        Action::MoveBack,
        Action::MoveLeft,
        Action::MoveRight,
        Action::Jump,
        Action::Crouch,
        Action::Walk,
        Action::Fire,
        Action::Scope,
        Action::Reload,
        Action::Primary,
        Action::Secondary,
        Action::Knife,
        Action::Grenades,
        Action::BuyMenu,
        Action::Scoreboard,
        Action::Chat,
        Action::TeamChat,
        Action::Inspect,
        Action::Drop,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Action::MoveForward => "Move Forward",
            Action::MoveBack => "Move Back",
            Action::MoveLeft => "Move Left",
            Action::MoveRight => "Move Right",
            Action::Jump => "Jump",
            Action::Crouch => "Crouch",
            Action::Walk => "Walk",
            Action::Fire => "Fire",
            Action::Scope => "Scope / Alt Fire",
            Action::Reload => "Reload",
            Action::Primary => "Primary Weapon",
            Action::Secondary => "Secondary Weapon",
            Action::Knife => "Knife",
            Action::Grenades => "Grenades (cycle)",
            Action::BuyMenu => "Buy Menu",
            Action::Scoreboard => "Scoreboard",
            Action::Chat => "Chat (all)",
            Action::TeamChat => "Chat (team)",
            Action::Inspect => "Inspect Weapon",
            Action::Drop => "Drop Weapon",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Binding {
    Key(KeyCode),
    Mouse(MouseButton),
}

const KEY_NAMES: &[(KeyCode, &str)] = &[
    (KeyCode::KeyA, "A"),
    (KeyCode::KeyB, "B"),
    (KeyCode::KeyC, "C"),
    (KeyCode::KeyD, "D"),
    (KeyCode::KeyE, "E"),
    (KeyCode::KeyF, "F"),
    (KeyCode::KeyG, "G"),
    (KeyCode::KeyH, "H"),
    (KeyCode::KeyI, "I"),
    (KeyCode::KeyJ, "J"),
    (KeyCode::KeyK, "K"),
    (KeyCode::KeyL, "L"),
    (KeyCode::KeyM, "M"),
    (KeyCode::KeyN, "N"),
    (KeyCode::KeyO, "O"),
    (KeyCode::KeyP, "P"),
    (KeyCode::KeyQ, "Q"),
    (KeyCode::KeyR, "R"),
    (KeyCode::KeyS, "S"),
    (KeyCode::KeyT, "T"),
    (KeyCode::KeyU, "U"),
    (KeyCode::KeyV, "V"),
    (KeyCode::KeyW, "W"),
    (KeyCode::KeyX, "X"),
    (KeyCode::KeyY, "Y"),
    (KeyCode::KeyZ, "Z"),
    (KeyCode::Digit0, "0"),
    (KeyCode::Digit1, "1"),
    (KeyCode::Digit2, "2"),
    (KeyCode::Digit3, "3"),
    (KeyCode::Digit4, "4"),
    (KeyCode::Digit5, "5"),
    (KeyCode::Digit6, "6"),
    (KeyCode::Digit7, "7"),
    (KeyCode::Digit8, "8"),
    (KeyCode::Digit9, "9"),
    (KeyCode::Space, "Space"),
    (KeyCode::ShiftLeft, "Left Shift"),
    (KeyCode::ShiftRight, "Right Shift"),
    (KeyCode::ControlLeft, "Left Ctrl"),
    (KeyCode::ControlRight, "Right Ctrl"),
    (KeyCode::AltLeft, "Left Alt"),
    (KeyCode::AltRight, "Right Alt"),
    (KeyCode::Tab, "Tab"),
    (KeyCode::CapsLock, "Caps Lock"),
    (KeyCode::Enter, "Enter"),
    (KeyCode::Backspace, "Backspace"),
    (KeyCode::Backquote, "`"),
    (KeyCode::Minus, "-"),
    (KeyCode::Equal, "="),
    (KeyCode::BracketLeft, "["),
    (KeyCode::BracketRight, "]"),
    (KeyCode::Semicolon, ";"),
    (KeyCode::Quote, "'"),
    (KeyCode::Comma, ","),
    (KeyCode::Period, "."),
    (KeyCode::Slash, "/"),
    (KeyCode::Backslash, "\\"),
    (KeyCode::ArrowUp, "Up"),
    (KeyCode::ArrowDown, "Down"),
    (KeyCode::ArrowLeft, "Left"),
    (KeyCode::ArrowRight, "Right"),
    (KeyCode::F1, "F1"),
    (KeyCode::F2, "F2"),
    (KeyCode::F3, "F3"),
    (KeyCode::F4, "F4"),
    (KeyCode::F5, "F5"),
    (KeyCode::F6, "F6"),
    (KeyCode::Insert, "Insert"),
    (KeyCode::Delete, "Delete"),
    (KeyCode::Home, "Home"),
    (KeyCode::End, "End"),
    (KeyCode::PageUp, "Page Up"),
    (KeyCode::PageDown, "Page Down"),
    (KeyCode::Numpad0, "Num 0"),
    (KeyCode::Numpad1, "Num 1"),
    (KeyCode::Numpad2, "Num 2"),
    (KeyCode::Numpad3, "Num 3"),
    (KeyCode::Numpad4, "Num 4"),
    (KeyCode::Numpad5, "Num 5"),
    (KeyCode::Numpad6, "Num 6"),
    (KeyCode::Numpad7, "Num 7"),
    (KeyCode::Numpad8, "Num 8"),
    (KeyCode::Numpad9, "Num 9"),
];

const MOUSE_NAMES: &[(MouseButton, &str)] = &[
    (MouseButton::Left, "Mouse 1"),
    (MouseButton::Right, "Mouse 2"),
    (MouseButton::Middle, "Mouse 3"),
    (MouseButton::Back, "Mouse 4"),
    (MouseButton::Forward, "Mouse 5"),
];

impl Binding {
    pub fn name(self) -> String {
        match self {
            Binding::Key(k) => KEY_NAMES
                .iter()
                .find(|(c, _)| *c == k)
                .map(|(_, n)| n.to_string())
                .unwrap_or_else(|| format!("{k:?}")),
            Binding::Mouse(m) => MOUSE_NAMES
                .iter()
                .find(|(c, _)| *c == m)
                .map(|(_, n)| n.to_string())
                .unwrap_or_else(|| format!("{m:?}")),
        }
    }

    pub fn from_name(name: &str) -> Option<Binding> {
        KEY_NAMES
            .iter()
            .find(|(_, n)| *n == name)
            .map(|(k, _)| Binding::Key(*k))
            .or_else(|| {
                MOUSE_NAMES
                    .iter()
                    .find(|(_, n)| *n == name)
                    .map(|(m, _)| Binding::Mouse(*m))
            })
    }

    pub fn is_bindable_key(k: KeyCode) -> bool {
        KEY_NAMES.iter().any(|(c, _)| *c == k)
    }
}

impl Serialize for Binding {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.name())
    }
}

impl<'de> Deserialize<'de> for Binding {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let name = String::deserialize(d)?;
        Binding::from_name(&name).ok_or_else(|| serde::de::Error::custom(format!("unknown key {name}")))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Keybinds(pub BTreeMap<Action, Binding>);

impl Default for Keybinds {
    fn default() -> Self {
        use Binding::*;
        let map = [
            (Action::MoveForward, Key(KeyCode::KeyW)),
            (Action::MoveBack, Key(KeyCode::KeyS)),
            (Action::MoveLeft, Key(KeyCode::KeyA)),
            (Action::MoveRight, Key(KeyCode::KeyD)),
            (Action::Jump, Key(KeyCode::Space)),
            (Action::Crouch, Key(KeyCode::ControlLeft)),
            (Action::Walk, Key(KeyCode::ShiftLeft)),
            (Action::Fire, Mouse(MouseButton::Left)),
            (Action::Scope, Mouse(MouseButton::Right)),
            (Action::Reload, Key(KeyCode::KeyR)),
            (Action::Primary, Key(KeyCode::Digit1)),
            (Action::Secondary, Key(KeyCode::Digit2)),
            (Action::Knife, Key(KeyCode::Digit3)),
            (Action::Grenades, Key(KeyCode::Digit4)),
            (Action::BuyMenu, Key(KeyCode::KeyB)),
            (Action::Scoreboard, Key(KeyCode::Tab)),
            (Action::Chat, Key(KeyCode::KeyY)),
            (Action::TeamChat, Key(KeyCode::KeyU)),
            (Action::Inspect, Key(KeyCode::KeyF)),
            (Action::Drop, Key(KeyCode::KeyG)),
        ];
        Keybinds(map.into_iter().collect())
    }
}

impl Keybinds {
    pub fn get(&self, action: Action) -> Binding {
        self.0
            .get(&action)
            .copied()
            .unwrap_or_else(|| Keybinds::default().0[&action])
    }
}

/// Convenience system param to query actions through the user's keybinds.
#[derive(SystemParam)]
pub struct Controls<'w> {
    keys: Res<'w, ButtonInput<KeyCode>>,
    mouse: Res<'w, ButtonInput<MouseButton>>,
    settings: Res<'w, Settings>,
}

impl Controls<'_> {
    pub fn binding_name(&self, action: Action) -> String {
        self.settings.keybinds.get(action).name()
    }

    pub fn pressed(&self, action: Action) -> bool {
        match self.settings.keybinds.get(action) {
            Binding::Key(k) => self.keys.pressed(k),
            Binding::Mouse(m) => self.mouse.pressed(m),
        }
    }

    pub fn just_pressed(&self, action: Action) -> bool {
        match self.settings.keybinds.get(action) {
            Binding::Key(k) => self.keys.just_pressed(k),
            Binding::Mouse(m) => self.mouse.just_pressed(m),
        }
    }
}
