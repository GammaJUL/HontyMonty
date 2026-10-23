# Honty Monty — TODO

Ideas and known gaps, roughly ordered by impact. Tick items off as they land.

## Gameplay
- [ ] Bots (fill empty slots, practice offline)
- [x] Spectator camera when dead (cycle through alive teammates)
- [ ] Bomb plant/defuse objective mode (Alpha attacks, Bravo defends) + side swap at half-time
- [ ] Drop / pick up weapons (G key), dropped guns on death
- [ ] Wallbangs (penetration through thin walls / crates with damage falloff)
- [ ] Real spray patterns per weapon instead of the generic climb + sway
- [x] Knife right-click heavy stab and backstab damage
- [ ] Player-vs-player collision
- [ ] Fall damage
- [ ] Defuse kit, Zeus, decoy and incendiary (team variants of molotov)
- [ ] Friendly-fire toggle in lobby settings
- [ ] Overtime rules for tied team matches
- [x] Smoke blocks flashbangs and hides name tags (client side)
- [x] Gray screen fog while standing inside a smoke, fading towards the edge
- [x] Footstep sounds for other players (volume by distance; walking/crouching is silent)
- [ ] Left/right panning for footsteps and shots

## Maps
- [ ] More maps + map vote in the lobby
- [ ] Ramps / slopes (movement is AABB only right now)
- [ ] Load maps from a data file instead of code

## Networking
- [ ] Binary codec (bincode/postcard) and delta-compressed snapshots instead of JSON
- [ ] Lag compensation (rewind hitboxes to the shooter's view time)
- [ ] Server-side movement validation / anti-cheat (speed, fire-rate, ammo checks)
- [ ] Internet play: NAT punch-through or a relay / dedicated server binary
- [x] Ping display on the scoreboard
- [x] Text chat (Y = all, U = team)
- [ ] Host migration when the host leaves
- [ ] Kick / ban for the host, lobby max-players setting

## Cases & inventory
- [ ] More cases, each with its own collection
- [ ] Gloves as rare special items
- [ ] Trade-up contract (10 items -> 1 of the next rarity)
- [ ] Inspect view (3D weapon preview rotating in the inventory)
- [ ] StatTrak kill counters that count up in matches
- [ ] Pattern seeds affecting the texture (e.g. Fade %, Doppler phases)
- [ ] Sort / filter inventory, delete items
- [ ] Wear visibly affects the in-game material (roughness, scratches)

## Character
- [ ] More hairstyles, facial hair, glasses/hats, body types
- [ ] Animated characters (walk cycle, crouch, reload)
- [ ] Agent skins (team-specific outfits)

## Settings / UX
- [ ] Rebindable keys for chat and drop
- [ ] Crosshair codes (import/export)
- [ ] Separate music volume once there is music
- [ ] Gamepad support
- [ ] Brightness / gamma, shadow quality, MSAA settings
- [ ] Migrate from the deprecated `Interaction` to `bevy::ui_widgets` (Bevy 0.20 deprecation)
- [ ] Loading screen + tooltips
- [ ] Localisation

## Audio & visuals
- [ ] Real sound files (currently procedurally synthesized)
- [ ] Spatial audio (left/right panning)
- [ ] Muzzle flash, shell casings, blood decals
- [ ] Better weapon and knife models
- [ ] Skybox

## Tech
- [ ] CI running `cargo test` and `cargo clippy`
- [ ] More tests for round flow, FFA respawns and grenades
