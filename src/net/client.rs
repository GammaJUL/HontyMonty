//! Client side of the protocol plus LAN lobby discovery.

use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};

use bevy::prelude::*;

use super::protocol::*;

const SEND_INTERVAL: f32 = 1.0 / 60.0;
const CONNECT_TIMEOUT: f32 = 5.0;
const LOST_TIMEOUT: f32 = 6.0;

#[derive(Clone, Debug, PartialEq)]
pub enum ConnStatus {
    Connecting,
    Connected,
    Failed(String),
}

/// A server event delivered to game systems exactly once.
#[derive(Message, Clone, Debug)]
pub struct NetEvent(pub GameEvent);

#[derive(Resource)]
pub struct NetClient {
    socket: UdpSocket,
    pub server: SocketAddr,
    pub status: ConnStatus,
    pub my_id: Option<PlayerId>,
    join: C2S,
    join_timer: f32,
    age: f32,
    next_seq: u32,
    pending: Vec<(u32, Cmd)>,
    pub snapshot: Option<Snapshot>,
    pub profiles: HashMap<PlayerId, NetProfile>,
    last_event: u32,
    since_recv: f32,
    send_timer: f32,
    /// Written by gameplay every frame, sent at 60 Hz.
    pub local: ClientState,
}

impl NetClient {
    pub fn connect(server: SocketAddr, join: C2S) -> std::io::Result<Self> {
        let socket = UdpSocket::bind(("0.0.0.0", 0))?;
        socket.set_nonblocking(true)?;
        Ok(Self {
            socket,
            server,
            status: ConnStatus::Connecting,
            my_id: None,
            join,
            join_timer: 0.0,
            age: 0.0,
            next_seq: 1,
            pending: Vec::new(),
            snapshot: None,
            profiles: HashMap::new(),
            last_event: 0,
            since_recv: 0.0,
            send_timer: 0.0,
            local: ClientState::default(),
        })
    }

    pub fn send_cmd(&mut self, cmd: Cmd) {
        self.pending.push((self.next_seq, cmd));
        self.next_seq += 1;
    }

    pub fn leave(&self) {
        let bytes = encode(&C2S::Leave);
        for _ in 0..3 {
            let _ = self.socket.send_to(&bytes, self.server);
        }
    }

    pub fn me(&self) -> Option<&NetPlayer> {
        let id = self.my_id?;
        self.snapshot.as_ref()?.players.iter().find(|p| p.id == id)
    }

    pub fn player(&self, id: PlayerId) -> Option<&NetPlayer> {
        self.snapshot.as_ref()?.players.iter().find(|p| p.id == id)
    }

    pub fn is_host(&self) -> bool {
        self.me().is_some_and(|p| p.is_host)
    }

    fn send(&self, msg: &C2S) {
        let _ = self.socket.send_to(&encode(msg), self.server);
    }
}

pub fn client_receive(client: Option<ResMut<NetClient>>, time: Res<Time<Real>>, mut events: MessageWriter<NetEvent>) {
    let Some(mut c) = client else { return };
    let dt = time.delta_secs();
    c.age += dt;
    c.since_recv += dt;
    let mut buf = vec![0u8; MAX_PACKET];
    loop {
        let (len, from) = match c.socket.recv_from(&mut buf) {
            Ok(v) => v,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(_) => continue,
        };
        if from != c.server {
            continue;
        }
        let Some(msg) = decode::<S2C>(&buf[..len]) else { continue };
        c.since_recv = 0.0;
        match msg {
            S2C::Accepted { id } => {
                c.my_id = Some(id);
                if c.status == ConnStatus::Connecting {
                    c.status = ConnStatus::Connected;
                }
            }
            S2C::Rejected { reason } => c.status = ConnStatus::Failed(reason),
            S2C::Kicked { reason } => c.status = ConnStatus::Failed(reason),
            S2C::LobbyInfo(_) => {}
            S2C::Snapshot(snap) => {
                if c.my_id.is_none() {
                    c.my_id = Some(snap.your_id);
                    c.status = ConnStatus::Connected;
                }
                let ack = snap.ack_cmd;
                c.pending.retain(|(seq, _)| *seq > ack);
                let mut newest = c.last_event;
                for (id, e) in &snap.events {
                    if *id > c.last_event {
                        events.write(NetEvent(e.clone()));
                        newest = newest.max(*id);
                    }
                }
                c.last_event = newest;
                let mut snap = *snap;
                if let Some(profiles) = snap.profiles.take() {
                    c.profiles = profiles.into_iter().map(|p| (p.id, p)).collect();
                }
                c.snapshot = Some(snap);
            }
        }
    }
    match c.status {
        ConnStatus::Connecting if c.age > CONNECT_TIMEOUT => {
            c.status = ConnStatus::Failed("No response from the lobby".into());
        }
        ConnStatus::Connected if c.since_recv > LOST_TIMEOUT => {
            c.status = ConnStatus::Failed("Lost connection to the lobby".into());
        }
        _ => {}
    }
}

pub fn client_send(client: Option<ResMut<NetClient>>, time: Res<Time<Real>>) {
    let Some(mut c) = client else { return };
    let dt = time.delta_secs();
    match c.status {
        ConnStatus::Connecting => {
            c.join_timer -= dt;
            if c.join_timer <= 0.0 {
                c.join_timer = 0.4;
                c.send(&c.join);
            }
        }
        ConnStatus::Connected => {
            c.send_timer -= dt;
            if c.send_timer <= 0.0 {
                c.send_timer = SEND_INTERVAL;
                let msg = C2S::Update {
                    state: c.local.clone(),
                    cmds: c.pending.iter().take(64).cloned().collect(),
                };
                c.send(&msg);
            }
        }
        ConnStatus::Failed(_) => {}
    }
}

// ---------------------------------------------------------------- discovery

pub struct FoundLobby {
    pub addr: SocketAddr,
    pub info: LobbyInfo,
    pub seen: f32,
}

#[derive(Resource)]
pub struct LobbyBrowser {
    socket: Option<UdpSocket>,
    pub found: Vec<FoundLobby>,
    timer: f32,
    time: f32,
}

impl Default for LobbyBrowser {
    fn default() -> Self {
        let socket = UdpSocket::bind(("0.0.0.0", 0)).ok().and_then(|s| {
            s.set_nonblocking(true).ok()?;
            s.set_broadcast(true).ok()?;
            Some(s)
        });
        Self {
            socket,
            found: Vec::new(),
            timer: 0.0,
            time: 0.0,
        }
    }
}

impl LobbyBrowser {
    pub fn refresh(&mut self) {
        self.found.clear();
        self.timer = 0.0;
    }

    pub fn update(&mut self, dt: f32) {
        let Some(socket) = &self.socket else { return };
        self.time += dt;
        self.timer -= dt;
        if self.timer <= 0.0 {
            self.timer = 1.5;
            let bytes = encode(&C2S::Discover);
            for port in PORT_RANGE {
                let _ = socket.send_to(&bytes, (Ipv4Addr::BROADCAST, port));
                let _ = socket.send_to(&bytes, (Ipv4Addr::LOCALHOST, port));
            }
        }
        let mut buf = vec![0u8; MAX_PACKET];
        loop {
            let (len, from) = match socket.recv_from(&mut buf) {
                Ok(v) => v,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(_) => continue,
            };
            let Some(S2C::LobbyInfo(info)) = decode::<S2C>(&buf[..len]) else { continue };
            // The same lobby answers on loopback and on the LAN address; keep one entry per port+name.
            let same = self.found.iter_mut().find(|f| {
                f.addr == from || (f.addr.port() == from.port() && f.info.name == info.name)
            });
            match same {
                Some(f) => {
                    f.info = info;
                    f.seen = self.time;
                }
                None => self.found.push(FoundLobby {
                    addr: from,
                    info,
                    seen: self.time,
                }),
            }
        }
        let now = self.time;
        self.found.retain(|f| now - f.seen < 5.0);
    }
}
