//! Headless `--simulate` harness for gesture / wheel / room crypto (no GUI).

use crate::gesture::{GestureKind, GestureOutcome, GestureSession};
use crate::room::{
    derive_room_keys, format_room_code, make_room_code, open_packet, parse_room_code, seal_packet,
    ParsedRoomCode,
};
use crate::wheel::{hover_slot, PINGS, WHEEL_RADIUS};
use eframe::egui::Pos2;

pub fn run_simulate() -> i32 {
    let mut failed = 0u32;

    let mut g = GestureSession::default();
    let center = Pos2::new(200.0, 200.0);
    let o = g.tick(true, false, center, false);
    assert_none(&o, "rise should not place", &mut failed);
    if !g.wheel_active() {
        eprintln!("FAIL wheel should open");
        failed += 1;
    }
    let up = Pos2::new(200.0, 200.0 - WHEEL_RADIUS * 0.7);
    let _ = g.tick(true, false, up, false);
    let hover = g.wheel.as_ref().and_then(|w| w.hover);
    expect(hover == Some(0), "hover top slot 0", &mut failed);
    match g.tick(false, false, up, false) {
        GestureOutcome::Placed {
            kind: GestureKind::Ping,
            slot: 0,
            ..
        } => {}
        other => {
            eprintln!("FAIL place ping: {other:?}");
            failed += 1;
        }
    }

    let mut g2 = GestureSession::default();
    let _ = g2.tick(true, false, center, false);
    match g2.tick(false, false, center, false) {
        GestureOutcome::Cancelled => {}
        other => {
            eprintln!("FAIL center cancel: {other:?}");
            failed += 1;
        }
    }

    let mut g3 = GestureSession::default();
    let _ = g3.tick(false, true, center, false);
    match g3.tick(false, true, up, true) {
        GestureOutcome::Cancelled => {}
        other => {
            eprintln!("FAIL esc cancel: {other:?}");
            failed += 1;
        }
    }

    expect(hover_slot(center, center).is_none(), "deadzone", &mut failed);
    expect(PINGS.len() == 8, "8 pings", &mut failed);

    let code = make_room_code(&[10, 11, 12, 13, 14, 15, 16, 17, 18]);
    let disp = format_room_code(&code);
    match parse_room_code(&disp) {
        ParsedRoomCode::Ok { code: c } if c == code => {}
        other => {
            eprintln!("FAIL parse code: {other:?}");
            failed += 1;
        }
    }
    match derive_room_keys(&code) {
        Ok(keys) => {
            let pkt = seal_packet(&keys.msg_key, b"hello-room").unwrap();
            match open_packet(&keys.msg_key, &pkt) {
                Some(p) if p == b"hello-room" => {}
                _ => {
                    eprintln!("FAIL seal/open");
                    failed += 1;
                }
            }
        }
        Err(e) => {
            eprintln!("FAIL derive: {e:#}");
            failed += 1;
        }
    }

    // Two hubs crypto-compatible
    if failed == 0 {
        println!("simulate: all checks passed");
        0
    } else {
        eprintln!("simulate: {failed} failure(s)");
        1
    }
}

fn assert_none(o: &GestureOutcome, msg: &str, failed: &mut u32) {
    if !matches!(o, GestureOutcome::None) {
        eprintln!("FAIL {msg}: {o:?}");
        *failed += 1;
    }
}

fn expect(cond: bool, msg: &str, failed: &mut u32) {
    if !cond {
        eprintln!("FAIL {msg}");
        *failed += 1;
    }
}
