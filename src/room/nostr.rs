//! Nostr relay transport: kind 24747 ephemeral events on room topic (lolPing-compatible).

use crate::room::crypto::RoomKeys;
use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use k256::schnorr::SigningKey;
use rand::rngs::OsRng;
use rand::RngCore;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tungstenite::{connect, Message};

pub const ROOM_EVENT_KIND: u64 = 24747;

pub const RELAYS: &[&str] = &[
    "wss://nos.lol",
    "wss://nostr.mom",
    "wss://relay.snort.social",
    "wss://relay.agentry.com",
    "wss://relay.routstr.com",
    "wss://nostr.rblb.it",
];

#[derive(Clone)]
struct KeyPair {
    signing: Arc<SigningKey>,
    pubkey_hex: String,
}

fn make_keypair() -> Result<KeyPair> {
    let signing = SigningKey::random(&mut OsRng);
    let pubkey_hex = hex::encode(signing.verifying_key().to_bytes());
    Ok(KeyPair {
        signing: Arc::new(signing),
        pubkey_hex,
    })
}

fn serialize_event(
    pubkey: &str,
    created_at: u64,
    kind: u64,
    tags: &[Vec<String>],
    content: &str,
) -> String {
    // NIP-01: [0,pubkey,created_at,kind,tags,content]
    json!([0, pubkey, created_at, kind, tags, content]).to_string()
}

pub fn sign_event(
    keys: &KeyPair,
    kind: u64,
    tags: Vec<Vec<String>>,
    content: String,
) -> Result<Value> {
    let created_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let ser = serialize_event(&keys.pubkey_hex, created_at, kind, &tags, &content);
    let id = Sha256::digest(ser.as_bytes());
    let id_hex = hex::encode(id);
    let mut id32 = [0u8; 32];
    id32.copy_from_slice(&id);
    let mut aux = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut aux);
    let sig = keys
        .signing
        .sign_prehash_with_aux_rand(&id32, &aux)
        .map_err(|e| anyhow!("sign: {e}"))?;
    let sig_hex = hex::encode(sig.to_bytes());
    Ok(json!({
        "id": id_hex,
        "pubkey": keys.pubkey_hex,
        "created_at": created_at,
        "kind": kind,
        "tags": tags,
        "content": content,
        "sig": sig_hex,
    }))
}

/// Build REQ subscription JSON.
pub fn req_message(sub_id: &str, topic: &str, since: u64) -> String {
    json!(["REQ", sub_id, {"kinds": [ROOM_EVENT_KIND], "#x": [topic], "since": since, "limit": 0}])
        .to_string()
}

pub fn event_message(ev: &Value) -> String {
    json!(["EVENT", ev]).to_string()
}

/// Parse relay frame; return event content (base64 ciphertext) if kind matches.
pub fn parse_relay_event_content(text: &str) -> Option<String> {
    let v: Value = serde_json::from_str(text).ok()?;
    let arr = v.as_array()?;
    if arr.first()?.as_str()? != "EVENT" {
        return None;
    }
    let ev = arr.get(2)?;
    if ev.get("kind")?.as_u64()? != ROOM_EVENT_KIND {
        return None;
    }
    ev.get("content")?.as_str().map(|s| s.to_string())
}

pub struct NostrTransport {
    running: Arc<AtomicBool>,
    publish_tx: Option<Sender<Vec<u8>>>,
    pub topic: String,
}

impl NostrTransport {
    /// Connect to first working relay (try list). Incoming sealed packets → `on_packet`.
    pub fn start<F>(keys: &RoomKeys, on_packet: F) -> Result<Self>
    where
        F: Fn(Vec<u8>) + Send + 'static,
    {
        let topic = keys.relay_topic.clone();
        let pair = make_keypair()?;
        let running = Arc::new(AtomicBool::new(true));
        let (pub_tx, pub_rx) = mpsc::channel::<Vec<u8>>();
        let run = running.clone();
        let topic_t = topic.clone();

        thread::Builder::new()
            .name("lolping-nostr".into())
            .spawn(move || {
                if let Err(e) = relay_loop(run, topic_t, pair, pub_rx, on_packet) {
                    log::warn!("nostr relay loop ended: {e:#}");
                }
            })?;

        Ok(Self {
            running,
            publish_tx: Some(pub_tx),
            topic,
        })
    }

    pub fn publish(&self, sealed: &[u8]) {
        if let Some(tx) = &self.publish_tx {
            let _ = tx.send(sealed.to_vec());
        }
    }

    pub fn stop(self) {
        self.running.store(false, Ordering::SeqCst);
    }
}

fn relay_loop<F>(
    running: Arc<AtomicBool>,
    topic: String,
    pair: KeyPair,
    pub_rx: Receiver<Vec<u8>>,
    on_packet: F,
) -> Result<()>
where
    F: Fn(Vec<u8>),
{
    let mut last_err = anyhow!("no relays");
    for url in RELAYS {
        if !running.load(Ordering::SeqCst) {
            break;
        }
        match session(url, &topic, &pair, &running, &pub_rx, &on_packet) {
            Ok(()) => return Ok(()),
            Err(e) => {
                log::info!("nostr: {url} failed: {e:#}");
                last_err = e;
                thread::sleep(Duration::from_millis(400));
            }
        }
    }
    Err(last_err)
}

fn session<F>(
    url: &str,
    topic: &str,
    pair: &KeyPair,
    running: &AtomicBool,
    pub_rx: &Receiver<Vec<u8>>,
    on_packet: &F,
) -> Result<()>
where
    F: Fn(Vec<u8>),
{
    let (mut socket, _resp) = connect(url).with_context(|| format!("connect {url}"))?;
    let since = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs().saturating_sub(600))
        .unwrap_or(0);
    socket
        .send(Message::Text(req_message("lolping", topic, since)))
        .context("REQ")?;
    log::info!("nostr: subscribed on {url} topic={}…", &topic[..16.min(topic.len())]);

    // Non-blocking-ish reads via short timeouts when underlying stream supports it.
    match socket.get_mut() {
        tungstenite::stream::MaybeTlsStream::NativeTls(s) => {
            let _ = s.get_mut().set_read_timeout(Some(Duration::from_millis(200)));
        }
        tungstenite::stream::MaybeTlsStream::Plain(s) => {
            let _ = s.set_read_timeout(Some(Duration::from_millis(200)));
        }
        _ => {}
    }

    while running.load(Ordering::SeqCst) {
        while let Ok(pkt) = pub_rx.try_recv() {
            let content = B64.encode(&pkt);
            let tags = vec![vec!["x".into(), topic.to_string()]];
            let ev = sign_event(pair, ROOM_EVENT_KIND, tags, content)?;
            socket.send(Message::Text(event_message(&ev)))?;
        }
        match socket.read() {
            Ok(Message::Text(t)) => {
                if let Some(content) = parse_relay_event_content(&t) {
                    if let Ok(bytes) = B64.decode(content.as_bytes()) {
                        on_packet(bytes);
                    }
                }
            }
            Ok(Message::Ping(p)) => {
                let _ = socket.send(Message::Pong(p));
            }
            Ok(Message::Close(_)) => break,
            Err(tungstenite::Error::Io(e))
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut => {}
            Err(e) => return Err(e.into()),
            _ => {}
        }
    }
    Ok(())
}

/// Verify NIP-01 id matches content (for tests).
pub fn event_id_hex(pubkey: &str, created_at: u64, kind: u64, tags: &[Vec<String>], content: &str) -> String {
    let ser = serialize_event(pubkey, created_at, kind, tags, content);
    hex::encode(Sha256::digest(ser.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_and_id_stable() {
        let keys = make_keypair().unwrap();
        let tags = vec![vec!["x".into(), "abc".into()]];
        let ev = sign_event(&keys, ROOM_EVENT_KIND, tags.clone(), "hi".into()).unwrap();
        let id = ev.get("id").unwrap().as_str().unwrap();
        let created = ev.get("created_at").unwrap().as_u64().unwrap();
        let expect = event_id_hex(&keys.pubkey_hex, created, ROOM_EVENT_KIND, &tags, "hi");
        assert_eq!(id, expect);
        assert_eq!(keys.pubkey_hex.len(), 64);
    }

    #[test]
    fn parse_event_frame() {
        let frame = r#"["EVENT","sub",{"id":"aa","pubkey":"bb","created_at":1,"kind":24747,"tags":[],"content":"YQ==","sig":"cc"}]"#;
        // kind ok but we only need content extraction
        let c = parse_relay_event_content(frame).unwrap();
        assert_eq!(c, "YQ==");
    }
}
