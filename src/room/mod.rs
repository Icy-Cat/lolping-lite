//! Room sync: codes, AES-GCM crypto, LAN UDP, Nostr stub.

mod code;
mod crypto;
mod lan;
mod nostr;
mod protocol;

pub use code::{format_room_code, make_room_code, parse_room_code, ParsedRoomCode};
pub use crypto::{derive_room_keys, open_packet, seal_packet, RoomKeys, MAX_PACKET};
pub use lan::{LanTransport, LAN_PORT};
pub use nostr::NostrTransport;
pub use protocol::{
    decode_message, encode_message, EmoteMsg, MemberStatus, PingMsg, PresenceMsg, RoomMessage,
    PROTOCOL_VERSION,
};

use crate::settings::Settings;
use anyhow::Result;
use rand::RngCore;
use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct RoomMember {
    pub peer: String,
    pub name: String,
    pub color: u8,
    pub status: MemberStatus,
    pub self_: bool,
}

#[derive(Debug, Clone)]
pub enum RoomEvent {
    MemberUpsert(RoomMember),
    MemberGone(String),
    RemotePing {
        peer: String,
        name: String,
        ping: String,
        d: u32,
        x: f64,
        y: f64,
    },
    RemoteEmote {
        peer: String,
        name: String,
        e: String,
        d: u32,
        x: f64,
        y: f64,
    },
    Status(String),
}

pub struct RoomHub {
    pub peer_id: String,
    keys: Option<RoomKeys>,
    code: Option<String>,
    seq: u64,
    tx: Sender<RoomEvent>,
    rx: Receiver<RoomEvent>,
    members: Arc<Mutex<HashMap<String, RoomMember>>>,
    lan: Option<LanTransport>,
    name: String,
    color: u8,
}

impl RoomHub {
    pub fn new(settings: &Settings) -> Self {
        let (tx, rx) = mpsc::channel();
        let mut peer = [0u8; 8];
        rand::thread_rng().fill_bytes(&mut peer);
        Self {
            peer_id: hex::encode(peer),
            keys: None,
            code: None,
            seq: 0,
            tx,
            rx,
            members: Arc::new(Mutex::new(HashMap::new())),
            lan: None,
            name: settings.display_name.clone(),
            color: settings.tag_color,
        }
    }

    pub fn code_display(&self) -> Option<String> {
        self.code.as_ref().map(|c| format_room_code(c))
    }

    pub fn poll(&mut self) -> Vec<RoomEvent> {
        let mut out = Vec::new();
        while let Ok(ev) = self.rx.try_recv() {
            if let RoomEvent::MemberUpsert(m) = &ev {
                self.members
                    .lock()
                    .unwrap()
                    .insert(m.peer.clone(), m.clone());
            }
            if let RoomEvent::MemberGone(id) = &ev {
                self.members.lock().unwrap().remove(id);
            }
            out.push(ev);
        }
        out
    }

    pub fn members_snapshot(&self) -> Vec<RoomMember> {
        self.members.lock().unwrap().values().cloned().collect()
    }

    pub fn create(&mut self) -> Result<String> {
        let mut nine = [0u8; 9];
        rand::thread_rng().fill_bytes(&mut nine);
        let code = make_room_code(&nine);
        self.join_canonical(&code)?;
        Ok(format_room_code(&code))
    }

    pub fn join(&mut self, input: &str) -> Result<String> {
        match parse_room_code(input) {
            ParsedRoomCode::Ok { code } => {
                self.join_canonical(&code)?;
                Ok(format_room_code(&code))
            }
            ParsedRoomCode::Err { error } => anyhow::bail!("room code {error}"),
        }
    }

    fn join_canonical(&mut self, code: &str) -> Result<()> {
        self.leave();
        let keys = derive_room_keys(code)?;
        self.keys = Some(keys.clone());
        self.code = Some(code.to_string());

        let mut me = HashMap::new();
        me.insert(
            self.peer_id.clone(),
            RoomMember {
                peer: self.peer_id.clone(),
                name: self.name.clone(),
                color: self.color,
                status: MemberStatus::On,
                self_: true,
            },
        );
        *self.members.lock().unwrap() = me;

        let tx = self.tx.clone();
        let keys_cb = keys.clone();
        let peer_self = self.peer_id.clone();
        let members = self.members.clone();
        let lan = LanTransport::start(LAN_PORT, move |packet, from| {
            if let Some(plain) = open_packet(&keys_cb.msg_key, &packet) {
                if let Some(msg) = decode_message(&plain) {
                    handle_incoming(msg, &peer_self, &members, &tx, from);
                }
            }
        })?;
        self.lan = Some(lan);
        self.broadcast_presence(MemberStatus::On);
        let _ = self.tx.send(RoomEvent::Status(format!(
            "joined {} (LAN :{LAN_PORT})",
            format_room_code(code)
        )));
        Ok(())
    }

    pub fn leave(&mut self) {
        if self.keys.is_some() {
            self.broadcast_bye();
        }
        if let Some(lan) = self.lan.take() {
            lan.stop();
        }
        self.keys = None;
        self.code = None;
        self.members.lock().unwrap().clear();
    }

    fn next_seq(&mut self) -> u64 {
        self.seq += 1;
        self.seq
    }

    fn now_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    fn seal_send(&self, msg: &RoomMessage) {
        let Some(keys) = &self.keys else { return };
        let Some(lan) = &self.lan else { return };
        let plain = encode_message(msg);
        if let Ok(packet) = seal_packet(&keys.msg_key, &plain) {
            lan.broadcast(&packet);
        }
    }

    pub fn broadcast_presence(&mut self, status: MemberStatus) {
        let msg = RoomMessage::Presence(PresenceMsg {
            peer: self.peer_id.clone(),
            seq: self.next_seq(),
            ts: Self::now_ms(),
            proto: PROTOCOL_VERSION,
            app: env!("CARGO_PKG_VERSION").into(),
            name: self.name.clone(),
            color: self.color as u32,
            status,
        });
        self.seal_send(&msg);
    }

    pub fn broadcast_ping(&mut self, ping: &str, d: u32, x: f64, y: f64) {
        let msg = RoomMessage::Ping(PingMsg {
            peer: self.peer_id.clone(),
            seq: self.next_seq(),
            ts: Self::now_ms(),
            ping: ping.into(),
            d,
            x,
            y,
        });
        self.seal_send(&msg);
    }

    pub fn broadcast_emote(&mut self, e: &str, d: u32, x: f64, y: f64) {
        let msg = RoomMessage::Emote(EmoteMsg {
            peer: self.peer_id.clone(),
            seq: self.next_seq(),
            ts: Self::now_ms(),
            e: e.into(),
            d,
            x,
            y,
        });
        self.seal_send(&msg);
    }

    fn broadcast_bye(&mut self) {
        let msg = RoomMessage::Bye {
            peer: self.peer_id.clone(),
            seq: self.next_seq(),
            ts: Self::now_ms(),
        };
        self.seal_send(&msg);
    }
}

fn handle_incoming(
    msg: RoomMessage,
    self_peer: &str,
    members: &Arc<Mutex<HashMap<String, RoomMember>>>,
    tx: &Sender<RoomEvent>,
    _from: String,
) {
    match msg {
        RoomMessage::Presence(p) => {
            if p.peer == self_peer {
                return;
            }
            let m = RoomMember {
                peer: p.peer.clone(),
                name: p.name,
                color: p.color as u8,
                status: p.status,
                self_: false,
            };
            members.lock().unwrap().insert(p.peer, m.clone());
            let _ = tx.send(RoomEvent::MemberUpsert(m));
        }
        RoomMessage::Ping(p) => {
            if p.peer == self_peer {
                return;
            }
            let name = members
                .lock()
                .unwrap()
                .get(&p.peer)
                .map(|m| m.name.clone())
                .unwrap_or_else(|| p.peer.clone());
            let _ = tx.send(RoomEvent::RemotePing {
                peer: p.peer,
                name,
                ping: p.ping,
                d: p.d,
                x: p.x,
                y: p.y,
            });
        }
        RoomMessage::Emote(e) => {
            if e.peer == self_peer {
                return;
            }
            let name = members
                .lock()
                .unwrap()
                .get(&e.peer)
                .map(|m| m.name.clone())
                .unwrap_or_else(|| e.peer.clone());
            let _ = tx.send(RoomEvent::RemoteEmote {
                peer: e.peer,
                name,
                e: e.e,
                d: e.d,
                x: e.x,
                y: e.y,
            });
        }
        RoomMessage::Bye { peer, .. } => {
            members.lock().unwrap().remove(&peer);
            let _ = tx.send(RoomEvent::MemberGone(peer));
        }
    }
}
