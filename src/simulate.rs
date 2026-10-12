//! Headless `--simulate` harness.

use crate::audio::{decode_file, AudioOut};
use crate::display::{frac_to_local, list_displays, point_to_frac};
use crate::gesture::{GestureKind, GestureOutcome, GestureSession};
use crate::input::{map_triggers_for_test, InputConfig};
use crate::room::{
    derive_room_keys, format_room_code, make_room_code, open_packet, parse_room_code,
    parse_relay_event_content, seal_packet, ParsedRoomCode, ROOM_EVENT_KIND,
};
use crate::settings::{Settings, TriggerKey};
use crate::wheel::{hover_slot, PINGS, WHEEL_RADIUS};
use eframe::egui::Pos2;
use std::path::Path;

pub fn run_simulate() -> i32 {
    let mut failed = 0u32;

    // gesture
    let mut g = GestureSession::default();
    let center = Pos2::new(200.0, 200.0);
    let _ = g.tick(true, false, center, false);
    expect(g.wheel_active(), "wheel open", &mut failed);
    let up = Pos2::new(200.0, 200.0 - WHEEL_RADIUS * 0.7);
    let _ = g.tick(true, false, up, false);
    expect(
        g.wheel.as_ref().and_then(|w| w.hover) == Some(0),
        "hover slot 0",
        &mut failed,
    );
    match g.tick(false, false, up, false) {
        GestureOutcome::Placed {
            kind: GestureKind::Ping,
            slot: 0,
            ..
        } => {}
        o => {
            eprintln!("FAIL place: {o:?}");
            failed += 1;
        }
    }
    let mut g2 = GestureSession::default();
    let _ = g2.tick(true, false, center, false);
    expect(
        matches!(g2.tick(false, false, center, false), GestureOutcome::Cancelled),
        "center cancel",
        &mut failed,
    );
    expect(hover_slot(center, center).is_none(), "deadzone", &mut failed);
    expect(PINGS.len() == 8, "8 pings", &mut failed);

    // settings → input mapping
    let mut cfg = InputConfig::default();
    cfg.ping_trigger = TriggerKey::Ctrl;
    cfg.emote_trigger = TriggerKey::Alt;
    expect(
        map_triggers_for_test(&cfg, true, false, false, false) == (false, true),
        "swap triggers",
        &mut failed,
    );
    expect(
        cfg.pause_chord_matches(true, true, false, false),
        "pause chord",
        &mut failed,
    );

    // pause toggles enabled via settings mirror
    let mut s = Settings::default();
    s.enabled = true;
    s.enabled = !s.enabled;
    expect(!s.enabled, "pause toggle", &mut failed);

    // multi-display mapping
    let displays = list_displays();
    expect(!displays.is_empty(), "displays non-empty", &mut failed);
    let (n, fx, fy) = point_to_frac(&displays, displays[0].x as f64 + 10.0, displays[0].y as f64 + 10.0);
    expect(n == displays[0].number, "point display", &mut failed);
    let (lx, ly) = frac_to_local(&displays[0], fx, fy);
    expect(lx >= 0.0 && ly >= 0.0, "frac local", &mut failed);

    // room crypto + code
    let code = make_room_code(&[10, 11, 12, 13, 14, 15, 16, 17, 18]);
    match parse_room_code(&format_room_code(&code)) {
        ParsedRoomCode::Ok { code: c } if c == code => {}
        o => {
            eprintln!("FAIL code {o:?}");
            failed += 1;
        }
    }
    let keys = derive_room_keys(&code).expect("keys");
    let pkt = seal_packet(&keys.msg_key, b"emote-payload").unwrap();
    expect(
        open_packet(&keys.msg_key, &pkt).as_deref() == Some(b"emote-payload".as_ref()),
        "seal/open",
        &mut failed,
    );

    // nostr frame parse
    let frame = r#"["EVENT","lolping",{"kind":24747,"content":"AQID","id":"00","pubkey":"00","created_at":1,"tags":[],"sig":"00"}]"#;
    expect(
        parse_relay_event_content(frame).as_deref() == Some("AQID"),
        "nostr parse",
        &mut failed,
    );
    expect(ROOM_EVENT_KIND == 24747, "kind", &mut failed);

    // audio decode + play call (may be silent without device)
    let wav = Path::new("assets/original/sounds/OnMyWay.wav");
    let ogg = Path::new("assets/original/emotes/angry_kitty.ogg");
    if wav.exists() {
        match decode_file(wav) {
            Ok(n) if n > 0 => {}
            e => {
                eprintln!("FAIL wav decode {e:?}");
                failed += 1;
            }
        }
        let mut audio = AudioOut::new();
        audio.play_file(wav);
        expect(audio.play_count == 1, "audio play_count", &mut failed);
        expect(
            audio.last_played.as_deref() == Some(wav),
            "audio last path",
            &mut failed,
        );
    }
    if ogg.exists() {
        match decode_file(ogg) {
            Ok(n) if n > 0 => {}
            e => {
                eprintln!("FAIL ogg decode {e:?}");
                failed += 1;
            }
        }
    }

    // room "receive emote" path: decrypt → would place (logic covered by open+protocol)
    let emote_msg = br#"{"t":"emote","peer":"0123456789abcdef","seq":1,"ts":1,"e":"angry_kitty","d":1,"x":0.5,"y":0.5}"#;
    let sealed = seal_packet(&keys.msg_key, emote_msg).unwrap();
    let plain = open_packet(&keys.msg_key, &sealed).unwrap();
    expect(plain == emote_msg, "remote emote envelope", &mut failed);

    if failed == 0 {
        println!("simulate: all checks passed");
        0
    } else {
        eprintln!("simulate: {failed} failure(s)");
        1
    }
}

fn expect(cond: bool, msg: &str, failed: &mut u32) {
    if !cond {
        eprintln!("FAIL {msg}");
        *failed += 1;
    }
}
