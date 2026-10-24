//! End-to-end tests of the listen server over real UDP sockets.

use std::net::{SocketAddr, UdpSocket};

use super::protocol::*;
use super::server::GameServer;
use crate::game_mode::{GameMode, Phase, Team, FREEZE_TIME};
use crate::profile::Appearance;
use crate::weapons::{ShopItem, WeaponId};

struct TestClient {
    socket: UdpSocket,
    server: SocketAddr,
    seq: u32,
    pending: Vec<(u32, Cmd)>,
    state: ClientState,
    snapshot: Option<Snapshot>,
    rejected: Option<String>,
}

impl TestClient {
    fn new(port: u16) -> Self {
        let socket = UdpSocket::bind(("127.0.0.1", 0)).unwrap();
        socket.set_nonblocking(true).unwrap();
        Self {
            socket,
            server: SocketAddr::from(([127, 0, 0, 1], port)),
            seq: 1,
            pending: Vec::new(),
            state: ClientState::default(),
            snapshot: None,
            rejected: None,
        }
    }

    fn join(&self, name: &str, password: &str) {
        let msg = C2S::Join {
            version: PROTOCOL_VERSION,
            name: name.into(),
            password: password.into(),
            appearance: Appearance::default(),
            skins: vec![],
        };
        self.socket.send_to(&encode(&msg), self.server).unwrap();
    }

    fn cmd(&mut self, cmd: Cmd) {
        self.pending.push((self.seq, cmd));
        self.seq += 1;
    }

    fn send(&self) {
        let msg = C2S::Update {
            state: self.state.clone(),
            cmds: self.pending.clone(),
        };
        self.socket.send_to(&encode(&msg), self.server).unwrap();
    }

    fn recv(&mut self) {
        let mut buf = vec![0u8; MAX_PACKET];
        while let Ok((n, _)) = self.socket.recv_from(&mut buf) {
            match decode::<S2C>(&buf[..n]) {
                Some(S2C::Snapshot(s)) => {
                    let ack = s.ack_cmd;
                    self.pending.retain(|(q, _)| *q > ack);
                    self.snapshot = Some(*s);
                }
                Some(S2C::Rejected { reason }) => self.rejected = Some(reason),
                _ => {}
            }
        }
    }

    fn me(&self) -> &NetPlayer {
        let s = self.snapshot.as_ref().unwrap();
        s.players.iter().find(|p| p.id == s.your_id).unwrap()
    }
}

/// Advance the server while both clients keep talking to it.
fn run(server: &mut GameServer, clients: &mut [&mut TestClient], seconds: f32) {
    let steps = (seconds / 0.02).ceil() as usize;
    for _ in 0..steps {
        for c in clients.iter() {
            c.send();
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
        server.tick(0.02);
        std::thread::sleep(std::time::Duration::from_millis(1));
        for c in clients.iter_mut() {
            c.recv();
        }
    }
}

fn setup(port: u16, password: &str) -> (GameServer, TestClient, TestClient) {
    let mut server = GameServer::host("test".into(), password.into(), GameMode::OneVOne, port).unwrap();
    let port = server.port;
    let mut a = TestClient::new(port);
    let mut b = TestClient::new(port);
    a.join("Alice", password);
    std::thread::sleep(std::time::Duration::from_millis(5));
    server.tick(0.01);
    b.join("Bob", password);
    std::thread::sleep(std::time::Duration::from_millis(5));
    run(&mut server, &mut [&mut a, &mut b], 0.2);
    (server, a, b)
}

#[test]
fn wrong_password_is_rejected() {
    let mut server = GameServer::host("locked".into(), "hunter2".into(), GameMode::OneVOne, 27200).unwrap();
    let mut c = TestClient::new(server.port);
    c.join("Eve", "nope");
    std::thread::sleep(std::time::Duration::from_millis(5));
    server.tick(0.01);
    std::thread::sleep(std::time::Duration::from_millis(5));
    c.recv();
    assert_eq!(c.rejected.as_deref(), Some("Wrong password"));
}

#[test]
fn headshot_kills_and_pays_out() {
    let (mut server, mut a, mut b) = setup(27210, "pw");
    assert_eq!(a.me().team, Team::Alpha);
    assert_eq!(b.me().team, Team::Bravo);
    assert!(a.me().is_host);

    a.cmd(Cmd::StartMatch);
    run(&mut server, &mut [&mut a, &mut b], 0.2);
    assert_eq!(a.snapshot.as_ref().unwrap().phase, Phase::Freeze);

    // Re-buying the default pistol must be refused.
    a.cmd(Cmd::Buy(ShopItem::Weapon(WeaponId::Glock)));
    run(&mut server, &mut [&mut a, &mut b], 0.2);
    assert_eq!(a.me().money, 800, "already owned pistol must not be charged");

    run(&mut server, &mut [&mut a, &mut b], FREEZE_TIME + 0.2);
    assert_eq!(a.snapshot.as_ref().unwrap().phase, Phase::Live);

    // Stand 10 m apart on an open lane and shoot Bob in the head.
    a.state.pos = [-5.0, 0.0, -8.0];
    b.state.pos = [5.0, 0.0, -8.0];
    run(&mut server, &mut [&mut a, &mut b], 0.1);
    a.cmd(Cmd::Fire {
        weapon: WeaponId::Glock,
        origin: [-5.0, 1.65, -8.0],
        dirs: vec![[1.0, 0.0, 0.0]],
        heavy: false,
    });
    run(&mut server, &mut [&mut a, &mut b], 0.2);

    let bob = b.me().clone();
    assert!(!bob.alive, "a Glock headshot at 10 m should kill an unarmored player");
    assert_eq!(a.me().kills, 1);
    assert_eq!(a.me().headshots, 1);
    let snap = a.snapshot.as_ref().unwrap();
    assert_eq!(snap.phase, Phase::RoundEnd);
    assert_eq!(snap.score, [1, 0]);
    // 800 start + 300 kill reward + 3250 round win.
    assert_eq!(a.me().money, 800 + 300 + 3250);
    // Loser gets the first loss bonus.
    assert_eq!(b.me().money, 800 + 1400);
}

#[test]
fn shots_do_not_go_through_walls() {
    let (mut server, mut a, mut b) = setup(27220, "");
    a.cmd(Cmd::StartMatch);
    run(&mut server, &mut [&mut a, &mut b], FREEZE_TIME + 0.4);
    // The mid building spans x,z in [-3, 3].
    a.state.pos = [-6.0, 0.0, 0.0];
    b.state.pos = [6.0, 0.0, 0.0];
    run(&mut server, &mut [&mut a, &mut b], 0.1);
    a.cmd(Cmd::Fire {
        weapon: WeaponId::Glock,
        origin: [-6.0, 1.65, 0.0],
        dirs: vec![[1.0, 0.0, 0.0]],
        heavy: false,
    });
    run(&mut server, &mut [&mut a, &mut b], 0.2);
    assert!(b.me().alive);
    assert_eq!(b.me().health, 100);
}

#[test]
fn knife_range_and_backstab() {
    let (mut server, mut a, mut b) = setup(27230, "");
    a.cmd(Cmd::StartMatch);
    run(&mut server, &mut [&mut a, &mut b], FREEZE_TIME + 0.4);
    // Bob faces -Z (yaw 0).
    b.state.pos = [10.0, 0.0, -9.0];
    b.state.yaw = 0.0;
    let stab = |a: &mut TestClient, z: f32, dir: f32| {
        a.state.pos = [10.0, 0.0, z];
        a.cmd(Cmd::Fire {
            weapon: WeaponId::Knife,
            origin: [10.0, 1.65, z],
            dirs: vec![[0.0, 0.0, dir]],
            heavy: false,
        });
    };

    // Too far away: no damage.
    stab(&mut a, -6.0, -1.0);
    run(&mut server, &mut [&mut a, &mut b], 0.2);
    assert_eq!(b.me().health, 100);

    // From behind: backstab for 90.
    stab(&mut a, -8.0, -1.0);
    run(&mut server, &mut [&mut a, &mut b], 0.2);
    assert_eq!(b.me().health, 10);

    // From the front: a normal 40 slash finishes him.
    stab(&mut a, -10.0, 1.0);
    run(&mut server, &mut [&mut a, &mut b], 0.2);
    assert!(!b.me().alive);
    assert_eq!(a.me().money, 800 + 1500 + 3250, "knife kill reward + round win");
}

#[test]
fn team_chat_is_tagged_with_team() {
    let (mut server, mut a, mut b) = setup(27240, "");
    a.cmd(Cmd::TeamChat("rush B".into()));
    run(&mut server, &mut [&mut a, &mut b], 0.2);
    let events = &b.snapshot.as_ref().unwrap().events;
    assert!(events.iter().any(|(_, e)| matches!(
        e,
        GameEvent::Chat { text, team: Some(Team::Alpha), .. } if text == "rush B"
    )));
}

#[test]
fn friendly_fire_toggle_kick_and_fall_damage() {
    let mut server = GameServer::host("ff".into(), String::new(), GameMode::TwoVTwo, 27250).unwrap();
    let port = server.port;
    let (mut a, mut b, mut c) = (TestClient::new(port), TestClient::new(port), TestClient::new(port));
    for (cl, name) in [(&a, "Host"), (&b, "Mate"), (&c, "Enemy")] {
        cl.join(name, "");
        std::thread::sleep(std::time::Duration::from_millis(5));
        server.tick(0.01);
    }
    run(&mut server, &mut [&mut a, &mut b, &mut c], 0.2);
    // Put the host and "Mate" on the same team.
    let host_team = a.me().team;
    // Auto-assign filled the host's team (2/2), so swap the enemy out first.
    c.cmd(Cmd::SetTeam(host_team.other()));
    run(&mut server, &mut [&mut a, &mut b, &mut c], 0.2);
    b.cmd(Cmd::SetTeam(host_team));
    a.cmd(Cmd::SetFriendlyFire(true));
    run(&mut server, &mut [&mut a, &mut b, &mut c], 0.2);
    assert!(a.snapshot.as_ref().unwrap().lobby.friendly_fire);
    assert_eq!(b.me().team, host_team);

    // Only the host may kick, and kicked players are gone.
    let enemy = c.me().id;
    b.cmd(Cmd::Kick(enemy));
    run(&mut server, &mut [&mut a, &mut b, &mut c], 0.2);
    assert_eq!(a.snapshot.as_ref().unwrap().players.len(), 3);
    a.cmd(Cmd::Kick(enemy));
    run(&mut server, &mut [&mut a, &mut b], 0.2);
    assert_eq!(a.snapshot.as_ref().unwrap().players.len(), 2);

    a.cmd(Cmd::StartMatch);
    run(&mut server, &mut [&mut a, &mut b], FREEZE_TIME + 0.4);
    a.state.pos = [-5.0, 0.0, -8.0];
    b.state.pos = [5.0, 0.0, -8.0];
    run(&mut server, &mut [&mut a, &mut b], 0.1);
    // Teammate takes damage with friendly fire on (body shot, not lethal).
    a.cmd(Cmd::Fire {
        weapon: a.me().secondary.unwrap(),
        origin: [-5.0, 1.2, -8.0],
        dirs: vec![[1.0, 0.0, 0.0]],
        heavy: false,
    });
    run(&mut server, &mut [&mut a, &mut b], 0.2);
    assert!(b.me().health < 100, "friendly fire should hurt teammates");

    // Fall damage is applied to yourself and clamped.
    a.cmd(Cmd::FallDamage(30));
    run(&mut server, &mut [&mut a, &mut b], 0.2);
    assert_eq!(a.me().health, 70);
}

/// Like `run` but with big steps, for simulating whole matches quickly.
fn run_fast(server: &mut GameServer, clients: &mut [&mut TestClient], seconds: f32) {
    let steps = (seconds / 0.25).ceil() as usize;
    for _ in 0..steps {
        for c in clients.iter() {
            c.send();
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
        server.tick(0.25);
        std::thread::sleep(std::time::Duration::from_millis(1));
        for c in clients.iter_mut() {
            c.recv();
        }
    }
}

#[test]
fn tied_match_goes_to_overtime() {
    let (mut server, mut a, mut b) = setup(27260, "");
    a.cmd(Cmd::StartMatch);
    run_fast(&mut server, &mut [&mut a, &mut b], 0.5);
    // Alternate round wins until 8-8 (first to 9 in 1v1).
    for round in 0..16 {
        run_fast(&mut server, &mut [&mut a, &mut b], FREEZE_TIME + 0.5);
        let loser = if round % 2 == 0 { &mut b } else { &mut a };
        loser.cmd(Cmd::FallDamage(200));
        run_fast(&mut server, &mut [&mut a, &mut b], 0.5);
        run_fast(&mut server, &mut [&mut a, &mut b], crate::game_mode::ROUND_END_TIME);
    }
    let snap = a.snapshot.as_ref().unwrap();
    assert_eq!(snap.score, [8, 8]);
    assert_eq!(snap.win_target, 12, "overtime should need 4 more rounds");
    assert_ne!(snap.phase, Phase::MatchEnd);
}

#[test]
fn dropped_guns_can_be_picked_up() {
    let (mut server, mut a, mut b) = setup(27270, "");
    a.cmd(Cmd::StartMatch);
    run(&mut server, &mut [&mut a, &mut b], FREEZE_TIME + 0.4);
    a.state.pos = [-5.0, 0.0, -8.0];
    a.state.yaw = 0.0;
    run(&mut server, &mut [&mut a, &mut b], 0.1);
    a.cmd(Cmd::Drop(WeaponId::Glock));
    run(&mut server, &mut [&mut a, &mut b], 0.2);
    assert_eq!(a.me().secondary, None);
    let dropped = a.snapshot.as_ref().unwrap().dropped.clone();
    assert_eq!(dropped.len(), 1);
    // It lands about a meter in front (yaw 0 faces -Z).
    let pos = dropped[0].pos;
    assert!((pos[2] - -9.0).abs() < 0.2, "dropped at {pos:?}");

    // Walk onto it; the dropper can grab it again after a short delay.
    a.state.pos = [pos[0], 0.0, pos[2]];
    run(&mut server, &mut [&mut a, &mut b], 1.2);
    assert_eq!(a.me().secondary, Some(WeaponId::Glock));
    assert!(a.snapshot.as_ref().unwrap().dropped.is_empty());
}

#[test]
fn banned_players_cannot_rejoin() {
    let (mut server, mut a, mut b) = setup(27280, "");
    let bob = b.me().id;
    a.cmd(Cmd::Ban(bob));
    run(&mut server, &mut [&mut a], 0.2);
    let mut again = TestClient::new(server.port);
    again.join("Bob again", "");
    std::thread::sleep(std::time::Duration::from_millis(5));
    server.tick(0.01);
    std::thread::sleep(std::time::Duration::from_millis(5));
    again.recv();
    assert_eq!(again.rejected.as_deref(), Some("You are banned from this lobby"));
    b.recv();
}

#[test]
fn bots_find_and_shoot_players() {
    let (mut server, mut a, mut b) = setup(27290, "");
    // Swap the second human for a bot: kick Bob, add a bot (it takes Bravo).
    let bob = b.me().id;
    a.cmd(Cmd::Kick(bob));
    a.cmd(Cmd::AddBot);
    run(&mut server, &mut [&mut a], 0.2);
    let snap = a.snapshot.as_ref().unwrap();
    assert!(snap.players.iter().any(|p| p.name.starts_with("BOT")));

    a.cmd(Cmd::StartMatch);
    run_fast(&mut server, &mut [&mut a], FREEZE_TIME + 0.5);
    // Stand in plain view of the Bravo spawn.
    a.state.pos = [20.0, 0.0, -8.0];
    run_fast(&mut server, &mut [&mut a], 15.0);
    assert!(a.me().deaths > 0 || a.me().health < 100, "the bot should have shot us");
}

#[test]
fn wallbangs_depend_on_weapon_power() {
    let mut server = GameServer::host("wb".into(), String::new(), GameMode::FreeForAll, 27300).unwrap();
    let port = server.port;
    let (mut a, mut b) = (TestClient::new(port), TestClient::new(port));
    a.join("A", "");
    std::thread::sleep(std::time::Duration::from_millis(5));
    server.tick(0.01);
    b.join("B", "");
    run(&mut server, &mut [&mut a, &mut b], 0.2);
    a.cmd(Cmd::StartMatch);
    run(&mut server, &mut [&mut a, &mut b], 0.3);
    a.cmd(Cmd::Buy(ShopItem::Weapon(WeaponId::Ak47)));
    // A 1.2 m crate sits between x = -23 and -21.8 at this depth.
    a.state.pos = [-26.0, 0.0, -6.4];
    b.state.pos = [-19.0, 0.0, -6.4];
    run(&mut server, &mut [&mut a, &mut b], 0.3);
    let shot = |weapon| Cmd::Fire {
        weapon,
        origin: [-26.0, 0.95, -6.4],
        dirs: vec![[1.0, 0.0, 0.0]],
        heavy: false,
    };
    a.cmd(shot(WeaponId::Glock));
    run(&mut server, &mut [&mut a, &mut b], 0.2);
    assert_eq!(b.me().health, 100, "a pistol can't shoot through the crate");
    a.cmd(shot(WeaponId::Ak47));
    run(&mut server, &mut [&mut a, &mut b], 0.2);
    let hp = b.me().health;
    assert!((60..100).contains(&hp), "an AK goes through with reduced damage, health {hp}");
}

#[test]
fn ffa_players_respawn() {
    let mut server = GameServer::host("ffa".into(), String::new(), GameMode::FreeForAll, 27310).unwrap();
    let mut a = TestClient::new(server.port);
    a.join("A", "");
    run(&mut server, &mut [&mut a], 0.2);
    a.cmd(Cmd::StartMatch);
    run(&mut server, &mut [&mut a], 0.3);
    let seq = a.me().spawn_seq;
    a.cmd(Cmd::FallDamage(200));
    run(&mut server, &mut [&mut a], 0.3);
    assert!(!a.me().alive);
    run_fast(&mut server, &mut [&mut a], crate::game_mode::FFA_RESPAWN_TIME + 0.5);
    assert!(a.me().alive);
    assert_eq!(a.me().health, 100);
    assert!(a.me().spawn_seq > seq);
}

#[test]
fn he_grenade_hurts_nearby_enemies() {
    let (mut server, mut a, mut b) = setup(27320, "");
    a.cmd(Cmd::StartMatch);
    run(&mut server, &mut [&mut a, &mut b], 0.3);
    a.cmd(Cmd::Buy(ShopItem::Grenade(crate::weapons::GrenadeKind::He)));
    run_fast(&mut server, &mut [&mut a, &mut b], FREEZE_TIME + 0.5);
    a.state.pos = [-5.0, 0.0, -8.0];
    b.state.pos = [0.0, 0.0, -8.0];
    run(&mut server, &mut [&mut a, &mut b], 0.1);
    // Drop it right at Bob's feet.
    a.cmd(Cmd::Throw {
        kind: crate::weapons::GrenadeKind::He,
        origin: [-0.5, 0.3, -8.0],
        vel: [0.0, 0.0, 0.0],
    });
    run(&mut server, &mut [&mut a, &mut b], 2.5);
    assert!(b.me().health < 60, "HE should hit hard up close, health {}", b.me().health);
}

/// Seconds a single bot needs to kill a player standing still in plain view (capped at `seconds`).
fn bot_time_to_kill(port: u16, difficulty: crate::game_mode::BotDifficulty, seconds: f32) -> f32 {
    let mut server = GameServer::host("bd".into(), String::new(), GameMode::OneVOne, port).unwrap();
    let mut a = TestClient::new(server.port);
    a.join("Target", "");
    run(&mut server, &mut [&mut a], 0.2);
    a.cmd(Cmd::SetBotDifficulty(difficulty));
    a.cmd(Cmd::AddBot);
    a.cmd(Cmd::StartMatch);
    run_fast(&mut server, &mut [&mut a], FREEZE_TIME + 0.5);
    a.state.pos = [16.0, 0.0, -8.0];
    let mut t = 0.0;
    while t < seconds && a.me().deaths == 0 && a.me().alive {
        run_fast(&mut server, &mut [&mut a], 0.25);
        t += 0.25;
    }
    t
}

#[test]
fn bot_difficulty_changes_how_dangerous_bots_are() {
    use crate::game_mode::BotDifficulty;
    // Average a few runs; bots have some randomness.
    let avg = |d: BotDifficulty, port: u16| (0..5).map(|k| bot_time_to_kill(port + k, d, 30.0)).sum::<f32>() / 5.0;
    let easy = avg(BotDifficulty::Easy, 27330);
    let normal = avg(BotDifficulty::Normal, 27350);
    let hard = avg(BotDifficulty::Hard, 27360);
    let expert = avg(BotDifficulty::Expert, 27340);
    eprintln!("time to kill a standing player: easy {easy:.2}s, normal {normal:.2}s, hard {hard:.2}s, expert {expert:.2}s");
    assert!(easy > expert * 1.5, "easy bots ({easy:.2}s) should be much slower than expert ({expert:.2}s)");
}
