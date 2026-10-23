//! Listen server run by the lobby host inside their own game process.
//! The server is authoritative for lobby membership, teams, rounds, money,
//! inventories, damage and grenades. Clients own their movement and aim.

use std::collections::VecDeque;
use std::net::{SocketAddr, UdpSocket};

use bevy::math::Vec3;
use bevy::prelude::*;

use super::protocol::*;
use crate::game_mode::*;
use crate::map::{self, line_of_sight, ray_player, raycast_map};
use crate::profile::{Appearance, EquippedSkin};
use crate::weapons::*;

const TIMEOUT: f32 = 6.0;
const SNAPSHOT_INTERVAL: f32 = 1.0 / 30.0;
const EVENT_LIFETIME: f32 = 1.5;
const GRENADE_STEP: f32 = 1.0 / 120.0;

struct ServerPlayer {
    id: PlayerId,
    addr: SocketAddr,
    name: String,
    appearance: Appearance,
    skins: Vec<EquippedSkin>,
    team: Team,
    last_seen: f32,
    last_cmd: u32,
    state: ClientState,
    alive: bool,
    health: i32,
    armor: i32,
    helmet: bool,
    money: u32,
    kills: u32,
    deaths: u32,
    headshots: u32,
    damage: u32,
    primary: Option<WeaponId>,
    secondary: Option<WeaponId>,
    grenades: Vec<GrenadeKind>,
    spawn_seq: u32,
    spawn_pos: Vec3,
    spawn_yaw: f32,
    respawn_at: Option<f32>,
}

impl ServerPlayer {
    fn feet(&self) -> Vec3 {
        Vec3::from(self.state.pos)
    }

    fn owns(&self, w: WeaponId) -> bool {
        w == WeaponId::Knife || self.primary == Some(w) || self.secondary == Some(w)
    }

    fn reset_loadout(&mut self) {
        self.primary = None;
        self.secondary = Some(default_pistol(self.team));
        self.grenades.clear();
        self.armor = 0;
        self.helmet = false;
    }
}

fn default_pistol(team: Team) -> WeaponId {
    match team {
        Team::Bravo => WeaponId::Usp,
        _ => WeaponId::Glock,
    }
}

struct ServerGrenade {
    id: u32,
    kind: GrenadeKind,
    owner: PlayerId,
    pos: Vec3,
    vel: Vec3,
    age: f32,
    acc: f32,
}

struct ServerFire {
    id: u32,
    owner: PlayerId,
    pos: Vec3,
    ends: f32,
    next_tick: f32,
}

#[derive(Resource)]
pub struct GameServer {
    socket: UdpSocket,
    pub port: u16,
    name: String,
    password: String,
    mode: GameMode,
    host_id: Option<PlayerId>,
    players: Vec<ServerPlayer>,
    phase: Phase,
    phase_timer: f32,
    round_elapsed: f32,
    round: u32,
    score: [u32; 2],
    loss_streak: [u32; 2],
    match_over_after_round: bool,
    events: VecDeque<(u32, f32, GameEvent)>,
    next_event: u32,
    grenades: Vec<ServerGrenade>,
    fires: Vec<ServerFire>,
    smokes: Vec<(Vec3, f32)>,
    time: f32,
    snapshot_timer: f32,
    snapshot_count: u64,
    next_player_id: PlayerId,
    next_object_id: u32,
}

impl GameServer {
    /// Binds the first free port at or after `port`.
    pub fn host(name: String, password: String, mode: GameMode, port: u16) -> std::io::Result<Self> {
        let mut last_err = None;
        for p in port..port + 5 {
            match UdpSocket::bind(("0.0.0.0", p)) {
                Ok(socket) => {
                    socket.set_nonblocking(true)?;
                    info!("hosting lobby '{name}' on UDP port {p}");
                    return Ok(Self {
                        socket,
                        port: p,
                        name,
                        password,
                        mode,
                        host_id: None,
                        players: Vec::new(),
                        phase: Phase::Lobby,
                        phase_timer: 0.0,
                        round_elapsed: 0.0,
                        round: 0,
                        score: [0, 0],
                        loss_streak: [0, 0],
                        match_over_after_round: false,
                        events: VecDeque::new(),
                        next_event: 1,
                        grenades: Vec::new(),
                        fires: Vec::new(),
                        smokes: Vec::new(),
                        time: 0.0,
                        snapshot_timer: 0.0,
                        snapshot_count: 0,
                        next_player_id: 1,
                        next_object_id: 1,
                    });
                }
                Err(e) => last_err = Some(e),
            }
        }
        Err(last_err.unwrap_or_else(|| std::io::Error::other("no free port")))
    }

    pub fn shutdown(&mut self) {
        let bytes = encode(&S2C::Kicked {
            reason: "The host closed the lobby".into(),
        });
        for p in &self.players {
            let _ = self.socket.send_to(&bytes, p.addr);
        }
        self.players.clear();
    }

    fn lobby_info(&self) -> LobbyInfo {
        LobbyInfo {
            name: self.name.clone(),
            has_password: !self.password.is_empty(),
            mode: self.mode,
            players: self.players.len(),
            max_players: self.mode.max_players(),
            in_game: self.phase != Phase::Lobby,
            host: self
                .players
                .iter()
                .find(|p| Some(p.id) == self.host_id)
                .map(|p| p.name.clone())
                .unwrap_or_default(),
        }
    }

    fn send(&self, addr: SocketAddr, msg: &S2C) {
        let _ = self.socket.send_to(&encode(msg), addr);
    }

    fn event(&mut self, e: GameEvent) {
        self.events.push_back((self.next_event, self.time, e));
        self.next_event += 1;
    }

    fn idx(&self, id: PlayerId) -> Option<usize> {
        self.players.iter().position(|p| p.id == id)
    }

    pub fn tick(&mut self, dt: f32) {
        self.time += dt;
        self.receive();
        self.drop_timeouts();
        self.update_game(dt);
        while self
            .events
            .front()
            .is_some_and(|(_, t, _)| self.time - t > EVENT_LIFETIME)
        {
            self.events.pop_front();
        }
        self.snapshot_timer -= dt;
        if self.snapshot_timer <= 0.0 {
            self.snapshot_timer = SNAPSHOT_INTERVAL;
            self.broadcast_snapshot();
        }
    }

    // ------------------------------------------------------------ packets

    fn receive(&mut self) {
        let mut buf = vec![0u8; MAX_PACKET];
        loop {
            let (len, addr) = match self.socket.recv_from(&mut buf) {
                Ok(v) => v,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                // Windows reports ICMP "port unreachable" as ConnectionReset; ignore it.
                Err(_) => continue,
            };
            let Some(msg) = decode::<C2S>(&buf[..len]) else {
                continue;
            };
            self.handle(addr, msg);
        }
    }

    fn handle(&mut self, addr: SocketAddr, msg: C2S) {
        match msg {
            C2S::Discover => self.send(addr, &S2C::LobbyInfo(self.lobby_info())),
            C2S::Join {
                version,
                name,
                password,
                appearance,
                skins,
            } => self.handle_join(addr, version, name, password, appearance, skins),
            C2S::Update { state, cmds } => {
                let Some(i) = self.players.iter().position(|p| p.addr == addr) else {
                    return;
                };
                let now = self.time;
                let p = &mut self.players[i];
                p.last_seen = now;
                if p.alive {
                    p.state = state;
                } else {
                    p.state.yaw = state.yaw;
                    p.state.pitch = state.pitch;
                }
                let id = p.id;
                let last = p.last_cmd;
                let mut newest = last;
                for (seq, cmd) in cmds {
                    if seq > newest {
                        newest = seq;
                        self.handle_cmd(id, cmd);
                    }
                }
                if let Some(i) = self.idx(id) {
                    self.players[i].last_cmd = newest;
                }
            }
            C2S::Leave => {
                if let Some(i) = self.players.iter().position(|p| p.addr == addr) {
                    let p = self.players.remove(i);
                    self.event(GameEvent::Info(format!("{} left the lobby", p.name)));
                    self.check_round_end();
                }
            }
        }
    }

    fn handle_join(
        &mut self,
        addr: SocketAddr,
        version: u32,
        name: String,
        password: String,
        appearance: Appearance,
        skins: Vec<EquippedSkin>,
    ) {
        if let Some(p) = self.players.iter().find(|p| p.addr == addr) {
            self.send(addr, &S2C::Accepted { id: p.id });
            return;
        }
        let reject = |s: &Self, reason: &str| s.send(addr, &S2C::Rejected { reason: reason.into() });
        if version != PROTOCOL_VERSION {
            return reject(self, "Game version mismatch");
        }
        if !self.password.is_empty() && password != self.password {
            return reject(self, "Wrong password");
        }
        if self.players.len() >= self.mode.max_players() {
            return reject(self, "Lobby is full");
        }
        let mut name: String = name.trim().chars().take(20).collect();
        if name.is_empty() {
            name = "Player".into();
        }
        let base = name.clone();
        let mut n = 2;
        while self.players.iter().any(|p| p.name == name) {
            name = format!("{base} ({n})");
            n += 1;
        }
        let id = self.next_player_id;
        self.next_player_id = self.next_player_id.wrapping_add(1).max(1);
        if self.host_id.is_none() {
            self.host_id = Some(id);
        }
        let team = self.auto_team();
        let money = if self.mode.is_ffa() { economy::FFA_START_MONEY } else { economy::START_MONEY };
        let mut p = ServerPlayer {
            id,
            addr,
            name: name.clone(),
            appearance,
            skins,
            team,
            last_seen: self.time,
            last_cmd: 0,
            state: ClientState::default(),
            alive: false,
            health: 100,
            armor: 0,
            helmet: false,
            money,
            kills: 0,
            deaths: 0,
            headshots: 0,
            damage: 0,
            primary: None,
            secondary: None,
            grenades: Vec::new(),
            spawn_seq: 0,
            spawn_pos: Vec3::ZERO,
            spawn_yaw: 0.0,
            respawn_at: None,
        };
        p.reset_loadout();
        if self.mode.is_ffa() && self.phase == Phase::Live {
            p.respawn_at = Some(self.time + 1.0);
        }
        self.players.push(p);
        self.send(addr, &S2C::Accepted { id });
        self.event(GameEvent::Info(format!("{name} joined the lobby")));
    }

    fn auto_team(&self) -> Team {
        if self.mode.is_ffa() {
            return Team::None;
        }
        let a = self.players.iter().filter(|p| p.team == Team::Alpha).count();
        let b = self.players.iter().filter(|p| p.team == Team::Bravo).count();
        if a <= b { Team::Alpha } else { Team::Bravo }
    }

    fn drop_timeouts(&mut self) {
        let now = self.time;
        let mut left = Vec::new();
        self.players.retain(|p| {
            let keep = now - p.last_seen < TIMEOUT;
            if !keep {
                left.push(p.name.clone());
            }
            keep
        });
        for name in left {
            self.event(GameEvent::Info(format!("{name} timed out")));
            self.check_round_end();
        }
    }

    fn broadcast_snapshot(&mut self) {
        self.snapshot_count += 1;
        let every = if self.phase == Phase::Lobby { 5 } else { 15 };
        let profiles = (self.snapshot_count % every == 1).then(|| {
            self.players
                .iter()
                .map(|p| NetProfile {
                    id: p.id,
                    appearance: p.appearance.clone(),
                    skins: p.skins.clone(),
                })
                .collect()
        });
        let buy_time_left = match self.phase {
            Phase::Freeze => self.phase_timer + BUY_TIME,
            Phase::Live if self.mode.is_ffa() => f32::INFINITY,
            Phase::Live => (BUY_TIME - self.round_elapsed).max(0.0),
            _ => 0.0,
        };
        let players: Vec<NetPlayer> = self
            .players
            .iter()
            .map(|p| NetPlayer {
                id: p.id,
                name: p.name.clone(),
                team: p.team,
                pos: p.state.pos,
                yaw: p.state.yaw,
                pitch: p.state.pitch,
                crouch: p.state.crouch,
                moving: p.state.moving,
                weapon: p.state.weapon,
                grenade: p.state.grenade,
                alive: p.alive,
                health: p.health.max(0),
                armor: p.armor,
                helmet: p.helmet,
                money: p.money,
                kills: p.kills,
                deaths: p.deaths,
                headshots: p.headshots,
                damage: p.damage,
                primary: p.primary,
                secondary: p.secondary,
                grenades: p.grenades.clone(),
                spawn_seq: p.spawn_seq,
                spawn_pos: p.spawn_pos.to_array(),
                spawn_yaw: p.spawn_yaw,
                is_host: Some(p.id) == self.host_id,
            })
            .collect();
        let mut snap = Snapshot {
            your_id: 0,
            ack_cmd: 0,
            lobby: self.lobby_info(),
            phase: self.phase,
            phase_time_left: self.phase_timer.max(0.0),
            buy_time_left: if buy_time_left.is_finite() { buy_time_left } else { 9999.0 },
            round: self.round,
            score: self.score,
            players,
            profiles,
            events: self.events.iter().map(|(id, _, e)| (*id, e.clone())).collect(),
        };
        for p in &self.players {
            snap.your_id = p.id;
            snap.ack_cmd = p.last_cmd;
            let bytes = encode(&S2C::Snapshot(Box::new(snap.clone())));
            let _ = self.socket.send_to(&bytes, p.addr);
        }
    }

    // ------------------------------------------------------------ commands

    fn handle_cmd(&mut self, id: PlayerId, cmd: Cmd) {
        let Some(i) = self.idx(id) else { return };
        let is_host = Some(id) == self.host_id;
        match cmd {
            Cmd::SetTeam(team) => {
                if self.phase != Phase::Lobby || self.mode.is_ffa() || team == Team::None {
                    return;
                }
                let count = self.players.iter().filter(|p| p.team == team).count();
                if count < self.mode.team_size() {
                    self.players[i].team = team;
                }
            }
            Cmd::SetMode(mode) => {
                if !is_host || self.phase != Phase::Lobby {
                    return;
                }
                if self.players.len() > mode.max_players() {
                    self.event(GameEvent::Info(format!("Too many players for {}", mode.name())));
                    return;
                }
                self.mode = mode;
                for p in &mut self.players {
                    p.team = Team::None;
                }
                for k in 0..self.players.len() {
                    let t = self.auto_team();
                    self.players[k].team = t;
                }
            }
            Cmd::StartMatch => {
                if is_host && self.phase == Phase::Lobby {
                    self.start_match();
                }
            }
            Cmd::EndMatch => {
                if is_host && self.phase != Phase::Lobby {
                    self.to_lobby();
                }
            }
            Cmd::Buy(item) => self.buy(i, item),
            Cmd::Fire {
                weapon,
                origin,
                dirs,
            } => self.fire(i, weapon, Vec3::from(origin), dirs),
            Cmd::Throw { kind, origin, vel } => {
                let p = &mut self.players[i];
                if !p.alive || !matches!(self.phase, Phase::Live | Phase::RoundEnd) {
                    return;
                }
                let Some(gi) = p.grenades.iter().position(|g| *g == kind) else {
                    return;
                };
                p.grenades.remove(gi);
                let gid = self.next_object_id;
                self.next_object_id += 1;
                self.grenades.push(ServerGrenade {
                    id: gid,
                    kind,
                    owner: id,
                    pos: Vec3::from(origin),
                    vel: Vec3::from(vel).clamp_length_max(30.0),
                    age: 0.0,
                    acc: 0.0,
                });
                self.event(GameEvent::GrenadeThrown {
                    id: gid,
                    kind,
                    owner: id,
                    pos: origin,
                    vel,
                });
            }
            Cmd::Chat(text) => {
                let text: String = text.chars().take(120).collect();
                let from = self.players[i].name.clone();
                self.event(GameEvent::Chat { from, text });
            }
        }
    }

    fn can_buy(&self, i: usize) -> bool {
        let p = &self.players[i];
        p.alive
            && match self.phase {
                Phase::Freeze => true,
                Phase::Live => self.mode.is_ffa() || self.round_elapsed < BUY_TIME,
                _ => false,
            }
    }

    fn buy(&mut self, i: usize, item: ShopItem) {
        let id = self.players[i].id;
        let deny = |s: &mut Self, reason: &str| {
            s.event(GameEvent::BuyDenied {
                player: id,
                reason: reason.into(),
            })
        };
        if !self.can_buy(i) {
            return deny(self, "Buy time is over");
        }
        let p = &self.players[i];
        let price = match item {
            ShopItem::Weapon(w) => {
                if p.owns(w) {
                    return deny(self, "You already own this weapon");
                }
                w.def().price
            }
            ShopItem::Grenade(g) => {
                if p.grenades.len() >= MAX_GRENADES {
                    return deny(self, "You cannot carry any more grenades");
                }
                if p.grenades.iter().filter(|x| **x == g).count() >= g.max_carry() {
                    return deny(self, "You cannot carry any more of these");
                }
                g.price()
            }
            ShopItem::Kevlar => {
                if p.armor >= 100 {
                    return deny(self, "You already have full armor");
                }
                650
            }
            ShopItem::KevlarHelmet => {
                if p.armor >= 100 && p.helmet {
                    return deny(self, "You already have armor and a helmet");
                }
                if p.armor >= 100 { 350 } else { 1000 }
            }
        };
        if p.money < price {
            return deny(self, "Not enough money");
        }
        let p = &mut self.players[i];
        p.money -= price;
        match item {
            ShopItem::Weapon(w) => match w.def().slot {
                Slot::Primary => p.primary = Some(w),
                Slot::Secondary => p.secondary = Some(w),
                Slot::Melee => {}
            },
            ShopItem::Grenade(g) => p.grenades.push(g),
            ShopItem::Kevlar => p.armor = 100,
            ShopItem::KevlarHelmet => {
                p.armor = 100;
                p.helmet = true;
            }
        }
        self.event(GameEvent::Bought { player: id, item });
    }

    fn enemies(&self, a: usize, b: usize) -> bool {
        a != b && (self.mode.is_ffa() || self.players[a].team != self.players[b].team)
    }

    fn fire(&mut self, i: usize, weapon: WeaponId, origin: Vec3, dirs: Vec<V3>) {
        let p = &self.players[i];
        if !p.alive || !p.owns(weapon) || !matches!(self.phase, Phase::Live | Phase::RoundEnd) {
            return;
        }
        let def = weapon.def();
        // Sanity check: the shot must start near the shooter.
        if origin.distance(p.feet()) > 3.0 {
            return;
        }
        let shooter = p.id;
        let mut hits: Vec<(usize, i32, i32, bool)> = Vec::new();
        for (n, d) in dirs.into_iter().take(def.pellets.max(1) as usize).enumerate() {
            let dir = Vec3::from(d).normalize_or_zero();
            if dir == Vec3::ZERO {
                continue;
            }
            let wall = raycast_map(origin, dir, def.range).unwrap_or(def.range);
            let mut best: Option<(f32, usize, HitGroup)> = None;
            for (j, v) in self.players.iter().enumerate() {
                if !v.alive || !self.enemies(i, j) {
                    continue;
                }
                if let Some((t, g)) = ray_player(origin, dir, v.feet(), v.state.crouch)
                    && t < wall
                    && best.is_none_or(|b| t < b.0)
                {
                    best = Some((t, j, g));
                }
            }
            let end = best.map(|b| b.0).unwrap_or(wall);
            if n == 0 && weapon != WeaponId::Knife {
                self.event(GameEvent::Shot {
                    shooter,
                    weapon,
                    from: origin.to_array(),
                    to: (origin + dir * end).to_array(),
                });
            }
            if let Some((t, j, g)) = best {
                let v = &self.players[j];
                let (hp, ar) = compute_damage(def, t, g, v.armor, v.helmet);
                hits.push((j, hp, ar, g == HitGroup::Head));
            }
        }
        // Merge pellets so shotguns produce one hit event per victim.
        let mut merged: Vec<(usize, i32, i32, bool)> = Vec::new();
        for (j, hp, ar, hs) in hits {
            if let Some(m) = merged.iter_mut().find(|m| m.0 == j) {
                m.1 += hp;
                m.2 += ar;
                m.3 |= hs;
            } else {
                merged.push((j, hp, ar, hs));
            }
        }
        for (j, hp, ar, hs) in merged {
            let victim = self.players[j].id;
            self.damage_player(victim, shooter, hp, ar, hs, DamageSource::Weapon(weapon));
        }
    }

    fn damage_player(
        &mut self,
        victim: PlayerId,
        attacker: PlayerId,
        hp: i32,
        armor_dmg: i32,
        headshot: bool,
        source: DamageSource,
    ) {
        let Some(vi) = self.idx(victim) else { return };
        if !self.players[vi].alive || hp <= 0 {
            return;
        }
        let v = &mut self.players[vi];
        let dealt = hp.min(v.health);
        v.health -= hp;
        v.armor = (v.armor - armor_dmg).max(0);
        if v.armor == 0 {
            v.helmet = false;
        }
        let died = v.health <= 0;
        let victim_team = v.team;
        if let Some(ai) = self.idx(attacker)
            && ai != vi
        {
            self.players[ai].damage += dealt as u32;
        }
        self.event(GameEvent::Hit {
            attacker,
            victim,
            damage: hp,
            headshot,
        });
        if !died {
            return;
        }
        let v = &mut self.players[vi];
        v.alive = false;
        v.health = 0;
        v.deaths += 1;
        if self.mode.is_ffa() {
            v.respawn_at = Some(self.time + FFA_RESPAWN_TIME);
        }
        if let Some(ai) = self.idx(attacker)
            && ai != vi
        {
            let a = &mut self.players[ai];
            a.kills += 1;
            if headshot {
                a.headshots += 1;
            }
            if self.mode.is_ffa() || a.team != victim_team {
                let reward = match source {
                    DamageSource::Weapon(w) => w.def().kill_reward,
                    DamageSource::Grenade(_) => 300,
                };
                a.money = (a.money + reward).min(economy::MAX_MONEY);
            }
        }
        self.event(GameEvent::Kill {
            killer: attacker,
            victim,
            source,
            headshot,
        });
        if self.mode.is_ffa() {
            if self.players.iter().any(|p| p.kills >= FFA_KILL_LIMIT) {
                self.end_ffa();
            }
        } else {
            self.check_round_end();
        }
    }

    // ------------------------------------------------------------ match flow

    fn start_match(&mut self) {
        let money = if self.mode.is_ffa() { economy::FFA_START_MONEY } else { economy::START_MONEY };
        for p in &mut self.players {
            p.money = money;
            p.kills = 0;
            p.deaths = 0;
            p.headshots = 0;
            p.damage = 0;
            p.alive = false;
            if self.mode.is_ffa() {
                p.team = Team::None;
            }
        }
        self.score = [0, 0];
        self.loss_streak = [0, 0];
        self.round = 0;
        self.match_over_after_round = false;
        self.event(GameEvent::Info(format!("Match started: {}", self.mode.name())));
        if self.mode.is_ffa() {
            self.phase = Phase::Live;
            self.phase_timer = FFA_TIME_LIMIT;
            self.round_elapsed = 0.0;
            self.round = 1;
            for k in 0..self.players.len() {
                self.spawn(k);
            }
            self.event(GameEvent::RoundStart { round: 1 });
        } else {
            self.start_round();
        }
    }

    fn start_round(&mut self) {
        self.round += 1;
        self.phase = Phase::Freeze;
        self.phase_timer = FREEZE_TIME;
        self.round_elapsed = 0.0;
        self.grenades.clear();
        self.fires.clear();
        self.smokes.clear();
        for k in 0..self.players.len() {
            self.spawn(k);
        }
        self.event(GameEvent::RoundStart { round: self.round });
    }

    fn spawn(&mut self, k: usize) {
        let m = map::map();
        let team = self.players[k].team;
        let (pos, yaw) = match team {
            Team::Alpha | Team::Bravo => {
                let spawns = if team == Team::Alpha { &m.spawns_alpha } else { &m.spawns_bravo };
                let slot = self.players[..k].iter().filter(|p| p.team == team).count();
                spawns[slot % spawns.len()]
            }
            Team::None => {
                // Pick the spawn furthest from living enemies.
                let others: Vec<Vec3> = self
                    .players
                    .iter()
                    .enumerate()
                    .filter(|(j, p)| *j != k && p.alive)
                    .map(|(_, p)| p.feet())
                    .collect();
                let mut best = m.spawns_ffa[fastrand::usize(..m.spawns_ffa.len())];
                if !others.is_empty() {
                    let score = |s: &Vec3| {
                        others.iter().map(|o| o.distance(*s)).fold(f32::MAX, f32::min)
                            + fastrand::f32() * 4.0
                    };
                    best = *m
                        .spawns_ffa
                        .iter()
                        .max_by(|a, b| score(&a.0).total_cmp(&score(&b.0)))
                        .unwrap();
                }
                best
            }
        };
        let p = &mut self.players[k];
        if !p.alive {
            p.reset_loadout();
        }
        p.alive = true;
        p.health = 100;
        p.respawn_at = None;
        p.spawn_seq += 1;
        p.spawn_pos = pos;
        p.spawn_yaw = yaw;
        p.state.pos = pos.to_array();
        p.state.yaw = yaw;
    }

    fn check_round_end(&mut self) {
        if self.mode.is_ffa() || self.phase != Phase::Live {
            return;
        }
        let count = |t: Team, alive_only: bool| {
            self.players
                .iter()
                .filter(|p| p.team == t && (!alive_only || p.alive))
                .count()
        };
        let (a_total, b_total) = (count(Team::Alpha, false), count(Team::Bravo, false));
        if a_total == 0 || b_total == 0 {
            return;
        }
        let (a, b) = (count(Team::Alpha, true), count(Team::Bravo, true));
        if a == 0 && b == 0 {
            self.end_round(None, "Both teams were eliminated".into());
        } else if a == 0 {
            self.end_round(Some(Team::Bravo), "Bravo eliminated the enemy team".into());
        } else if b == 0 {
            self.end_round(Some(Team::Alpha), "Alpha eliminated the enemy team".into());
        }
    }

    fn end_round(&mut self, winner: Option<Team>, reason: String) {
        self.phase = Phase::RoundEnd;
        self.phase_timer = ROUND_END_TIME;
        match winner {
            Some(w) => {
                self.score[w.index()] += 1;
                let l = w.other();
                self.loss_streak[w.index()] = 0;
                self.loss_streak[l.index()] += 1;
                let bonus = economy::loss_bonus(self.loss_streak[l.index()]);
                for p in &mut self.players {
                    let earn = if p.team == w { economy::ROUND_WIN } else { bonus };
                    p.money = (p.money + earn).min(economy::MAX_MONEY);
                }
            }
            None => {
                for p in &mut self.players {
                    p.money = (p.money + economy::LOSS_BONUS_BASE).min(economy::MAX_MONEY);
                }
            }
        }
        self.event(GameEvent::RoundEnd { winner, reason });
        let need = self.mode.rounds_to_win();
        let max_rounds = need * 2 - 2;
        if self.score.iter().any(|s| *s >= need) || self.round >= max_rounds {
            self.match_over_after_round = true;
        }
    }

    fn end_ffa(&mut self) {
        let winner = self
            .players
            .iter()
            .max_by_key(|p| (p.kills, std::cmp::Reverse(p.deaths)))
            .map(|p| p.name.clone())
            .unwrap_or_default();
        self.phase = Phase::MatchEnd;
        self.phase_timer = MATCH_END_TIME;
        self.event(GameEvent::MatchEnd { winner: format!("{winner} wins the match!") });
    }

    fn end_team_match(&mut self) {
        self.phase = Phase::MatchEnd;
        self.phase_timer = MATCH_END_TIME;
        let text = match self.score[0].cmp(&self.score[1]) {
            std::cmp::Ordering::Greater => "Team Alpha wins the match!".to_string(),
            std::cmp::Ordering::Less => "Team Bravo wins the match!".to_string(),
            std::cmp::Ordering::Equal => "The match ended in a draw".to_string(),
        };
        self.event(GameEvent::MatchEnd { winner: text });
    }

    fn to_lobby(&mut self) {
        self.phase = Phase::Lobby;
        self.phase_timer = 0.0;
        self.grenades.clear();
        self.fires.clear();
        self.smokes.clear();
        for p in &mut self.players {
            p.alive = false;
        }
    }

    fn update_game(&mut self, dt: f32) {
        self.phase_timer -= dt;
        match self.phase {
            Phase::Lobby => {}
            Phase::Freeze => {
                if self.phase_timer <= 0.0 {
                    self.phase = Phase::Live;
                    self.phase_timer = ROUND_TIME;
                    self.round_elapsed = 0.0;
                }
            }
            Phase::Live => {
                self.round_elapsed += dt;
                if self.mode.is_ffa() {
                    let now = self.time;
                    for k in 0..self.players.len() {
                        if self.players[k].respawn_at.is_some_and(|t| now >= t) {
                            self.spawn(k);
                        }
                    }
                    if self.phase_timer <= 0.0 {
                        self.end_ffa();
                    }
                } else if self.phase_timer <= 0.0 {
                    self.round_timeout();
                }
            }
            Phase::RoundEnd => {
                if self.phase_timer <= 0.0 {
                    if self.match_over_after_round {
                        self.end_team_match();
                    } else {
                        self.start_round();
                    }
                }
            }
            Phase::MatchEnd => {
                if self.phase_timer <= 0.0 {
                    self.to_lobby();
                }
            }
        }
        if self.phase != Phase::Lobby {
            self.update_grenades(dt);
            self.update_fires();
        }
    }

    fn round_timeout(&mut self) {
        let alive = |t: Team| self.players.iter().filter(|p| p.team == t && p.alive).count();
        let hp = |t: Team| -> i32 {
            self.players
                .iter()
                .filter(|p| p.team == t && p.alive)
                .map(|p| p.health)
                .sum()
        };
        let (a, b) = (alive(Team::Alpha), alive(Team::Bravo));
        let winner = match a.cmp(&b) {
            std::cmp::Ordering::Greater => Some(Team::Alpha),
            std::cmp::Ordering::Less => Some(Team::Bravo),
            std::cmp::Ordering::Equal => match hp(Team::Alpha).cmp(&hp(Team::Bravo)) {
                std::cmp::Ordering::Greater => Some(Team::Alpha),
                std::cmp::Ordering::Less => Some(Team::Bravo),
                std::cmp::Ordering::Equal => None,
            },
        };
        let reason = match winner {
            Some(t) => format!("Time ran out - {} had the advantage", t.name()),
            None => "Time ran out - draw".to_string(),
        };
        self.end_round(winner, reason);
    }

    fn update_grenades(&mut self, dt: f32) {
        let mut detonate = Vec::new();
        for g in &mut self.grenades {
            g.acc += dt;
            while g.acc >= GRENADE_STEP {
                g.acc -= GRENADE_STEP;
                g.age += GRENADE_STEP;
                let ground = map::step_grenade(&mut g.pos, &mut g.vel, GRENADE_STEP);
                let burst = g.kind == GrenadeKind::Molotov && ground && g.age > 0.05;
                if g.age >= g.kind.fuse() || burst {
                    detonate.push(g.id);
                    break;
                }
            }
        }
        for id in detonate {
            let Some(i) = self.grenades.iter().position(|g| g.id == id) else { continue };
            let g = self.grenades.remove(i);
            self.detonate(g);
        }
    }

    fn detonate(&mut self, g: ServerGrenade) {
        self.event(GameEvent::GrenadeDetonate {
            id: g.id,
            kind: g.kind,
            pos: g.pos.to_array(),
        });
        match g.kind {
            GrenadeKind::He => {
                let owner_idx = self.idx(g.owner);
                let mut victims = Vec::new();
                for (j, v) in self.players.iter().enumerate() {
                    if !v.alive {
                        continue;
                    }
                    let friendly = owner_idx.is_some_and(|o| o != j && !self.enemies(o, j));
                    if friendly {
                        continue;
                    }
                    let center = v.feet() + Vec3::Y * 0.9;
                    let d = center.distance(g.pos);
                    if d < HE_RADIUS && line_of_sight(g.pos + Vec3::Y * 0.1, center) {
                        let raw = HE_DAMAGE * (1.0 - d / HE_RADIUS);
                        let (hp, ar) = apply_armor(raw, 0.57, v.armor, v.armor > 0);
                        victims.push((v.id, hp, ar));
                    }
                }
                for (vid, hp, ar) in victims {
                    self.damage_player(vid, g.owner, hp, ar, false, DamageSource::Grenade(GrenadeKind::He));
                }
            }
            GrenadeKind::Smoke => {
                self.smokes.push((g.pos, self.time + SMOKE_DURATION));
                let mut out = Vec::new();
                self.fires.retain(|f| {
                    let inside = f.pos.distance(g.pos) < SMOKE_RADIUS + FIRE_RADIUS * 0.5;
                    if inside {
                        out.push(f.id);
                    }
                    !inside
                });
                for id in out {
                    self.event(GameEvent::FireOut { id });
                }
            }
            GrenadeKind::Molotov => {
                let now = self.time;
                let smoked = self
                    .smokes
                    .iter()
                    .any(|(p, end)| *end > now && p.distance(g.pos) < SMOKE_RADIUS);
                if smoked {
                    self.event(GameEvent::FireOut { id: g.id });
                } else {
                    self.fires.push(ServerFire {
                        id: g.id,
                        owner: g.owner,
                        pos: g.pos,
                        ends: now + FIRE_DURATION,
                        next_tick: now,
                    });
                }
            }
            GrenadeKind::Flash => {}
        }
    }

    fn update_fires(&mut self) {
        let now = self.time;
        self.smokes.retain(|(_, end)| *end > now);
        let mut damage = Vec::new();
        let mut expired = Vec::new();
        let owner_idx: Vec<Option<usize>> = self.fires.iter().map(|f| self.idx(f.owner)).collect();
        for (fi, f) in self.fires.iter_mut().enumerate() {
            if now >= f.ends {
                expired.push(f.id);
                continue;
            }
            if now < f.next_tick {
                continue;
            }
            f.next_tick = now + 0.25;
            for (j, v) in self.players.iter().enumerate() {
                if !v.alive {
                    continue;
                }
                let friendly = owner_idx[fi].is_some_and(|o| {
                    o != j && !(self.mode.is_ffa() || self.players[o].team != v.team)
                });
                if friendly {
                    continue;
                }
                let feet = v.feet();
                let flat = Vec3::new(feet.x - f.pos.x, 0.0, feet.z - f.pos.z).length();
                if flat < FIRE_RADIUS && (feet.y - f.pos.y).abs() < 1.5 {
                    damage.push((v.id, f.owner));
                }
            }
        }
        self.fires.retain(|f| now < f.ends);
        for id in expired {
            self.event(GameEvent::FireOut { id });
        }
        for (vid, owner) in damage {
            let hp = (FIRE_DPS * 0.25).round() as i32;
            self.damage_player(vid, owner, hp, 0, false, DamageSource::Grenade(GrenadeKind::Molotov));
        }
    }
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ServerSet;

pub struct ServerPlugin;

impl Plugin for ServerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreUpdate, server_tick.in_set(ServerSet));
    }
}

fn server_tick(server: Option<ResMut<GameServer>>, time: Res<Time<Real>>) {
    if let Some(mut server) = server {
        server.tick(time.delta_secs().min(0.25));
    }
}
