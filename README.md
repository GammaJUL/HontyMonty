# Honty Monty

A Counter-Strike–style multiplayer shooter written in Rust with Bevy 0.20.

## Running

```sh
cargo run            # dev build
cargo run --release  # optimized build
cargo test           # unit + network integration tests
```

Settings and your profile (character, skins) are saved in `./saves/`.

## Playing together

1. One player clicks **PLAY -> Create Lobby**. The password is optional; leave it empty for an open lobby.
2. Everyone else clicks **PLAY**. LAN lobbies show up automatically; select one, type the password if it is locked, and click **Join Lobby**.
   You can also type the host's address directly (`192.168.1.20` or `192.168.1.20:27015`).
3. Players pick a team, and the host picks the mode and clicks **START MATCH**.

The host runs the server inside their game on UDP port **27015** (or the next free port up to 27019, which the lobby browser also scans).
Allow the game through the firewall. Playing over the internet needs port forwarding on the host's router.

You can test locally by starting the game twice on one PC.

## Features

- **Modes:** 1v1, 2v2, 3v3, 4v4, 5v5 (round based, first to 9/11/13), and All vs All (deathmatch, first to 25 kills).
- **CS2 economy:** $800 start, kill rewards per weapon class, $3250 round win, loss bonus from $1400 up to $3400, and a $16000 cap.
- **Weapons:** Glock-18, USP-S, Desert Eagle, MP9, P90, Nova, AK-47, M4A4, AUG, SG 553, SSG 08, AWP, and the knife.
- **Utility:** HE grenade, flashbang, smoke, molotov, Kevlar, and Kevlar + Helmet.
- **Shooting:** spread from movement, jumping and spraying, plus recoil. Right-click scopes on AUG/SG 553 (one zoom level) and SSG 08/AWP (two levels).
- **Damage:** headshots ×4, stomach ×1.25, legs ×0.75, range falloff, and armor penetration, all matching CS2.
- **Case:** one case with unlimited free openings and CS2 odds: Mil-Spec 79.92%, Restricted 15.98%, Classified 3.2%, Covert 0.64%, and Rare Special (knives) 0.26%. Every item also rolls a wear float, StatTrak (10%) and a pattern seed.
- **Character:** man/woman body, skin tone, 8 hairstyles, hair/eye/shirt/pants colors and height, shown in the main menu.
- **Settings:**
  - Video: resolution, display mode, VSync, FPS limit, FPS counter and FOV.
  - Mouse: sensitivity (CS scale), zoom sensitivity and invert Y.
  - Audio: master, effects and UI volume, plus the hit sound.
  - Crosshair: editor.
  - Keybinds: every key can be rebound.

## Default controls

| Action | Key |
|---|---|
| Move | W A S D |
| Jump / Crouch / Walk | Space / Left Ctrl / Left Shift |
| Fire / Scope (grenade: throw / lob) | Mouse 1 / Mouse 2 |
| Reload | R |
| Primary / Secondary / Knife / Grenades | 1 / 2 / 3 / 4 (or mouse wheel) |
| Buy menu | B |
| Scoreboard | Tab |
| Chat (all / team) | Y / U |
| Knife: slash / heavy stab | Mouse 1 / Mouse 2 |
| Spectate next / previous (when dead) | Mouse 1 / Mouse 2 |
| Inspect weapon | F |
| Drop weapon (walk over guns to pick them up) | G |
| Pause menu | Esc |

## Code layout

| Path | What |
|---|---|
| `src/main.rs` | App setup |
| `src/weapons.rs` | Weapon/grenade data, CS2 damage model |
| `src/cases.rs` | Rarities, wear, skin catalog, case rolls |
| `src/game_mode.rs` | Modes, teams, round timings, economy |
| `src/map.rs` | Arena geometry, collision, raycasts, hitboxes |
| `src/net/` | UDP protocol, host server, client, LAN discovery, tests |
| `src/game/` | In-match: movement, combat, remote players, effects, HUD, buy and pause menus |
| `src/ui/` | Menu screens: main menu, customize, inventory/cases, lobby browser, lobby, settings |
| `src/audio.rs` | Procedurally synthesized sound effects |
| `src/character.rs`, `src/weapon_model.rs`, `src/skin_textures.rs` | Procedural models and skin textures |
| `src/dev.rs` | Dev shortcuts (see below) |

## Dev shortcuts

Environment variables, handy for quick iteration.

PowerShell (Windows):

```powershell
$env:HONTY_DEV_STATE="ingame"; cargo run                       # host + start a 1v1 immediately
$env:HONTY_DEV_STATE="ingame"; $env:HONTY_DEV_MODE="ffa"; cargo run
$env:HONTY_DEV_STATE="ingame"; $env:HONTY_DEV_SMOKE="0"; cargo run   # smoke on top of you
$env:HONTY_DEV_STATE="host"; cargo run                         # window 1: host, starts when someone joins
$env:HONTY_DEV_STATE="join"; cargo run                         # window 2: joins 127.0.0.1:27015
$env:HONTY_DEV_STATE="inventory"; $env:HONTY_DEV_OPEN="1"; cargo run

# Variables stick for the whole terminal session; clear them to get the normal menu back:
Remove-Item Env:HONTY_DEV_*
```

Bash (Linux/macOS/Git Bash):

```sh
HONTY_DEV_STATE=ingame cargo run                  # host + start a 1v1 immediately
HONTY_DEV_STATE=ingame HONTY_DEV_MODE=ffa cargo run
HONTY_DEV_STATE=host cargo run                    # instance 1: host, starts when someone joins
HONTY_DEV_STATE=join cargo run                    # instance 2: joins 127.0.0.1:27015
HONTY_DEV_STATE=inventory HONTY_DEV_OPEN=1 cargo run
HONTY_DEV_SCREENSHOT=shot.png ...                 # save a screenshot after ~7s and quit
```

See [TODO.md](TODO.md) for planned features.
