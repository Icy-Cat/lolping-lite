//! Nostr relay transport — stub for internet rooms.
//!
//! lolPing uses Trystero/Nostr for signaling. Full relay pool + WebSocket client
//! is deferred; LAN works today. When `allow_internet` is on we log the derived
//! relay topic so a future implementation can subscribe.

use crate::room::crypto::RoomKeys;

#[derive(Debug, Default)]
pub struct NostrTransport {
    pub topic: Option<String>,
}

impl NostrTransport {
    pub fn bind_topic(keys: &RoomKeys) -> Self {
        log::info!(
            "nostr stub: relay topic {} (internet path not yet implemented)",
            &keys.relay_topic[..16.min(keys.relay_topic.len())]
        );
        Self {
            topic: Some(keys.relay_topic.clone()),
        }
    }
}
