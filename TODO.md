# Honty Monty — TODO

Ideas and known gaps, roughly ordered by impact. Tick items off as they land.

## Gameplay
- [ ] Bots (fill empty slots, practice offline)
- [x] Spectator camera when dead (cycle through alive teammates)
- [ ] Bomb plant/defuse objective mode (Alpha attacks, Bravo defends) + side swap at half-time
- [x] Drop / pick up weapons (G key), dropped guns on death, buying drops your old gun
- [ ] Wallbangs (penetration through thin walls / crates with damage falloff)
- [x] Real spray patterns per weapon (AK, M4/AUG/SG, SMGs)
- [x] Knife right-click heavy stab and backstab damage
- [x] Player-vs-player collision
- [x] Fall damage
- [ ] Defuse kit, Zeus, decoy and incendiary (team variants of molotov)
- [x] Friendly-fire toggle in lobby settings (team kills cost $300)
- [x] Overtime for tied team matches (first to 4 more)
- [x] Smoke blocks flashbangs and hides name tags (client side)
- [x] Gray screen fog while standing inside a smoke, fading towards the edge
- [x] Footstep sounds for other players (volume by distance; walking/crouching is silent)
- [x] Left/right panning for footsteps, shots and grenades

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
- [x] Kick button for the host
- [x] Ban button for the host (by IP, for the lobby's lifetime)
- [ ] Lobby max-players setting, unban

## Cases & inventory
- [ ] More cases, each with its own collection
- [ ] Gloves as rare special items
- [ ] Trade-up contract (10 items -> 1 of the next rarity)
- [ ] Inspect view (3D weapon preview rotating in the inventory)
- [x] StatTrak kill counters that count up in matches
- [x] Pattern seeds affecting the texture (8 variants per skin)
- [x] Filter inventory by rarity, delete items
- [x] Wear visibly affects the in-game material (roughness, scratches)

- [x] Weapon inspect (F)
- [x] Money popups (+$300 / -$2700) and a damage report when you die

## Character
- [x] Facial hair, glasses/sunglasses, caps, beanies, headbands
- [ ] More hairstyles, body types
- [ ] Animated characters (walk cycle, crouch, reload)
- [ ] Agent skins (team-specific outfits)

## Settings / UX
- [x] Rebindable keys for chat and drop
- [x] Crosshair codes (copy / paste via clipboard)
- [ ] Separate music volume once there is music
- [ ] Gamepad support
- [x] Brightness, shadows on/off, MSAA settings
- [x] Replace the deprecated `Interaction` (own hit-testing, works with the low-res render target)
- [ ] Loading screen + tooltips
- [ ] Localisation

## Audio & visuals
- [ ] Real sound files (currently procedurally synthesized)
- [x] Spatial audio (left/right panning)
- [x] Muzzle flash
- [x] Shell casings, blood puffs
- [ ] Blood decals on walls
- [ ] Better weapon and knife models
- [ ] Skybox

## Tech
- [ ] CI running `cargo test` and `cargo clippy`
- [x] Tests for overtime, drop/pickup and bans
- [ ] More tests for FFA respawns and grenades
