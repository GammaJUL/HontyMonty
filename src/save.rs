//! Tiny JSON persistence helpers. Files live in `./saves/`.

use std::fs;
use std::path::PathBuf;

use serde::{de::DeserializeOwned, Serialize};

fn path(name: &str) -> PathBuf {
    PathBuf::from("saves").join(name)
}

pub fn load<T: DeserializeOwned + Default>(name: &str) -> T {
    match fs::read_to_string(path(name)) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_else(|e| {
            bevy::log::warn!("could not parse {name}: {e}, using defaults");
            T::default()
        }),
        Err(_) => T::default(),
    }
}

pub fn store<T: Serialize>(name: &str, value: &T) {
    let p = path(name);
    if let Some(dir) = p.parent() {
        let _ = fs::create_dir_all(dir);
    }
    match serde_json::to_string_pretty(value) {
        Ok(text) => {
            if let Err(e) = fs::write(&p, text) {
                bevy::log::warn!("could not save {name}: {e}");
            }
        }
        Err(e) => bevy::log::warn!("could not serialize {name}: {e}"),
    }
}
