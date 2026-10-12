//! JSON room messages — compatible with lolPing `roomProtocol.ts`.

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MemberStatus {
    On,
    Paused,
    Muted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresenceMsg {
    pub peer: String,
    pub seq: u64,
    pub ts: u64,
    pub proto: u32,
    pub app: String,
    pub name: String,
    pub color: u32,
    pub status: MemberStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PingMsg {
    pub peer: String,
    pub seq: u64,
    pub ts: u64,
    pub ping: String,
    pub d: u32,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmoteMsg {
    pub peer: String,
    pub seq: u64,
    pub ts: u64,
    pub e: String,
    pub d: u32,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone)]
pub enum RoomMessage {
    Presence(PresenceMsg),
    Ping(PingMsg),
    Emote(EmoteMsg),
    Bye { peer: String, seq: u64, ts: u64 },
}

pub fn encode_message(m: &RoomMessage) -> Vec<u8> {
    let v = match m {
        RoomMessage::Presence(p) => {
            let mut o = serde_json::to_value(p).unwrap();
            o.as_object_mut().unwrap().insert("t".into(), Value::String("presence".into()));
            o
        }
        RoomMessage::Ping(p) => {
            let mut o = serde_json::to_value(p).unwrap();
            if let Some(obj) = o.as_object_mut() {
                obj.insert("t".into(), Value::String("ping".into()));
                if let Some(x) = obj.get_mut("x") {
                    *x = Value::from(round4(p.x));
                }
                if let Some(y) = obj.get_mut("y") {
                    *y = Value::from(round4(p.y));
                }
            }
            o
        }
        RoomMessage::Emote(e) => {
            let mut o = serde_json::to_value(e).unwrap();
            if let Some(obj) = o.as_object_mut() {
                obj.insert("t".into(), Value::String("emote".into()));
                if let Some(x) = obj.get_mut("x") {
                    *x = Value::from(round4(e.x));
                }
                if let Some(y) = obj.get_mut("y") {
                    *y = Value::from(round4(e.y));
                }
            }
            o
        }
        RoomMessage::Bye { peer, seq, ts } => serde_json::json!({
            "t": "bye", "peer": peer, "seq": seq, "ts": ts
        }),
    };
    serde_json::to_vec(&v).unwrap_or_default()
}

fn round4(v: f64) -> f64 {
    (v * 10_000.0).round() / 10_000.0
}

pub fn decode_message(bytes: &[u8]) -> Option<RoomMessage> {
    let v: Value = serde_json::from_slice(bytes).ok()?;
    let t = v.get("t")?.as_str()?;
    let peer = v.get("peer")?.as_str()?.to_string();
    if peer.len() != 16 || !peer.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let seq = v.get("seq")?.as_u64()?;
    let ts = v.get("ts")?.as_u64()?;
    match t {
        "presence" => {
            let mut p: PresenceMsg = serde_json::from_value(v).ok()?;
            p.peer = peer;
            p.seq = seq;
            p.ts = ts;
            Some(RoomMessage::Presence(p))
        }
        "ping" => {
            let mut p: PingMsg = serde_json::from_value(v).ok()?;
            p.peer = peer;
            Some(RoomMessage::Ping(p))
        }
        "emote" => {
            let mut e: EmoteMsg = serde_json::from_value(v).ok()?;
            e.peer = peer;
            Some(RoomMessage::Emote(e))
        }
        "bye" => Some(RoomMessage::Bye { peer, seq, ts }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ping_roundtrip() {
        let m = RoomMessage::Ping(PingMsg {
            peer: "0123456789abcdef".into(),
            seq: 2,
            ts: 100,
            ping: "omw".into(),
            d: 1,
            x: 0.5,
            y: 0.25,
        });
        let bytes = encode_message(&m);
        let back = decode_message(&bytes).unwrap();
        match back {
            RoomMessage::Ping(p) => {
                assert_eq!(p.ping, "omw");
                assert!((p.x - 0.5).abs() < 1e-6);
            }
            _ => panic!("not ping"),
        }
    }
}
