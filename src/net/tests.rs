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
