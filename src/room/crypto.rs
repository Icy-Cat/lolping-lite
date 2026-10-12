//! scrypt + HKDF + AES-256-GCM — envelope compatible with lolPing `roomCrypto.ts`.

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use anyhow::{anyhow, Result};
use hkdf::Hkdf;
use rand::RngCore;
use scrypt::{scrypt, Params};
use sha2::Sha256;

pub const MAX_PACKET: usize = 1200;
const FORMAT: u8 = 0x01;
const NONCE_LEN: usize = 12;
const TAG_LEN: usize = 16;
const OVERHEAD: usize = 1 + NONCE_LEN + TAG_LEN;

#[derive(Clone)]
pub struct RoomKeys {
    pub msg_key: [u8; 32],
    pub relay_topic: String,
}

pub fn derive_room_keys(canonical: &str) -> Result<RoomKeys> {
    let params = Params::new(15, 8, 1, 32).map_err(|e| anyhow!("scrypt params: {e}"))?;
    let mut master = [0u8; 32];
    scrypt(
        canonical.as_bytes(),
        b"lolping/room/v1",
        &params,
        &mut master,
    )
    .map_err(|e| anyhow!("scrypt: {e}"))?;

    let msg_key = hkdf_sub(&master, b"msg")?;
    let topic_bytes = hkdf_sub(&master, b"sig-room")?;
    Ok(RoomKeys {
        msg_key,
        relay_topic: hex::encode(topic_bytes),
    })
}

fn hkdf_sub(master: &[u8], info: &[u8]) -> Result<[u8; 32]> {
    let hk = Hkdf::<Sha256>::new(Some(&[]), master);
    let mut out = [0u8; 32];
    hk.expand(info, &mut out)
        .map_err(|_| anyhow!("hkdf expand"))?;
    Ok(out)
}

pub fn seal_packet(key: &[u8; 32], plain: &[u8]) -> Result<Vec<u8>> {
    if plain.len() + OVERHEAD > MAX_PACKET {
        anyhow::bail!("room message too large");
    }
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| anyhow!("{e}"))?;
    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let aad = [FORMAT];
    let ct = cipher
        .encrypt(
            nonce,
            Payload {
                msg: plain,
                aad: &aad,
            },
        )
        .map_err(|_| anyhow!("encrypt"))?;
    // ct includes ciphertext || tag
    let mut out = Vec::with_capacity(OVERHEAD + plain.len());
    out.push(FORMAT);
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ct);
    Ok(out)
}

pub fn open_packet(key: &[u8; 32], packet: &[u8]) -> Option<Vec<u8>> {
    if packet.len() < OVERHEAD || packet.len() > MAX_PACKET || packet[0] != FORMAT {
        return None;
    }
    let cipher = Aes256Gcm::new_from_slice(key).ok()?;
    let nonce = Nonce::from_slice(&packet[1..1 + NONCE_LEN]);
    let aad = [FORMAT];
    cipher
        .decrypt(
            nonce,
            Payload {
                msg: &packet[1 + NONCE_LEN..],
                aad: &aad,
            },
        )
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seal_open_roundtrip() {
        let keys = derive_room_keys("012345678A").expect("derive");
        let plain = b"{\"t\":\"bye\",\"peer\":\"0123456789abcdef\",\"seq\":1,\"ts\":1}";
        let pkt = seal_packet(&keys.msg_key, plain).unwrap();
        let out = open_packet(&keys.msg_key, &pkt).expect("open");
        assert_eq!(out, plain);
        assert_eq!(keys.relay_topic.len(), 64);
    }
}
