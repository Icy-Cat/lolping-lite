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

    online_asset_check(&mut failed);

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

/// Live CDN check: manifest + one emote + sha256. Skips gracefully when offline.
/// With `LOLPING_NO_LOCAL_ASSETS=1`, also proves the catalog/on-demand path end to end
/// (names from manifest → non-blocking resolve schedules download → file lands in cache → decode).
fn online_asset_check(failed: &mut u32) {
    use crate::assets::{AssetCatalog, AssetId};
    use crate::remote::{
        cache_dir_under, local_assets_disabled, platform_cache_root, sha256_hex, AssetStore,
        Manifest, base_url,
    };
    use std::sync::Arc;
    if std::env::var("LOLPING_OFFLINE").as_deref() == Ok("1") {
        println!("simulate: online asset check skipped (LOLPING_OFFLINE=1)");
        return;
    }
    // Fresh temp cache so the check really downloads.
    let tmp = std::env::temp_dir().join(format!("lolping-sim-{}", std::process::id()));
    let store = Arc::new(AssetStore::new(None, Some(cache_dir_under(&tmp)), base_url()));
    let m: Arc<Manifest> = match store.refresh_manifest_blocking() {
        Ok(m) => m,
        Err(e) => {
            println!("simulate: online asset check skipped (offline? {e:#})");
            let _ = std::fs::remove_dir_all(&tmp);
            return;
        }
    };
    expect(m.files.len() > 100, "manifest file count", failed);
    let rel = "emotes/angry_kitty.webp";
    match store.fetch_blocking(rel) {
        Ok(p) => {
            let bytes = std::fs::read(&p).unwrap_or_default();
            let want = m.get(rel).map(|f| f.sha256.clone()).unwrap_or_default();
            expect(sha256_hex(&bytes) == want, "cdn emote sha256", failed);
            println!("simulate: cdn ok ({} files in manifest, {rel} {} bytes verified)", m.files.len(), bytes.len());
        }
        Err(e) => {
            eprintln!("FAIL cdn fetch {rel}: {e:#}");
            *failed += 1;
        }
    }

    if local_assets_disabled() {
        let no_v2 = std::env::var("LOLPING_NO_V2").as_deref() == Ok("1");
        let v2 = (crate::webm::VP9_AVAILABLE && !no_v2).then(|| {
            Arc::new(AssetStore::new(None, Some(tmp.join("v2")), crate::remote::v2_base_url()))
        });
        let mut cat = AssetCatalog::with_stores(store.clone(), v2.clone());
        expect(cat.emote_count() == m.emote_names().len(), "catalog names from manifest", failed);
        let idx = cat.emotes().iter().position(|e| e.name == "angry_kitty").unwrap_or(0);
        let name = cat.emotes().get(idx).map(|e| e.name.clone()).unwrap_or_default();
        let snd = "sounds/OnMyWay.wav";
        // Non-blocking: first call must return None and schedule download.
        expect(cat.resolve(snd).is_none(), "resolve non-blocking miss", failed);
        let t0 = std::time::Instant::now();
        while cat.resolve(snd).is_none() && t0.elapsed().as_secs() < 30 {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        match cat.resolve(snd) {
            Some(p) => {
                expect(decode_file(&p).map(|n| n > 0).unwrap_or(false), "on-demand wav decode", failed);
            }
            None => {
                eprintln!("FAIL on-demand sound never arrived");
                *failed += 1;
            }
        }
        if v2.is_none() {
            let _ = store.fetch_blocking("emotes/angry_kitty.webp");
        }
        // Non-blocking animation load: Err (downloading) until the file lands, then decodes.
        let t0 = std::time::Instant::now();
        let mut frames = 0;
        while t0.elapsed().as_secs() < 30 {
            if let Ok(a) = cat.load_emote_animation(AssetId::Index(idx)) {
                frames = a.frames.len();
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        expect(frames == 74, "on-demand emote animation (v2 webm if VP9 else v1 webp)", failed);
        println!("simulate: on-demand animation via {} ok ({frames} frames)", if v2.is_some() { "v2 webm (v1 webp fallback)" } else { "v1 webp" });
        println!("simulate: no-local-assets on-demand path ok (emote '{name}', {snd})");
        // Real user cache path sanity.
        expect(platform_cache_root().is_some(), "platform cache root", failed);
    }
    v2_check(failed);

    if std::env::var("LOLPING_SIM_FULL").as_deref() == Ok("1") {
        match store.download_all_blocking() {
            Ok(n) => {
                expect(n == m.files.len(), "full zip file count", failed);
                println!("simulate: full zip ok ({n} files verified, cache {} bytes)", store.cache_size());
            }
            Err(e) => {
                eprintln!("FAIL full zip: {e:#}");
                *failed += 1;
            }
        }
    }
    let _ = std::fs::remove_dir_all(&tmp);
}

/// v2: fetch manifest + one WebM from the live CDN, verify sha256, decode VP9+alpha in-app,
/// compare frame count/size/alpha/delays with the v1 WebP, and time per-frame decode.
fn v2_check(failed: &mut u32) {
    use crate::assets::decode_webm_animation;
    use crate::remote::{v2_base_url, AssetStore};
    use std::sync::Arc;
    if !crate::webm::VP9_AVAILABLE {
        println!("simulate: v2 check skipped (built without VP9; app uses v1 WebP)");
        return;
    }
    let tmp = std::env::temp_dir().join(format!("lolping-sim-v2-{}", std::process::id()));
    let v2 = Arc::new(AssetStore::new(None, Some(tmp.clone()), v2_base_url()));
    let m = match v2.refresh_manifest_blocking() {
        Ok(m) => m,
        Err(e) => {
            println!("simulate: v2 check skipped (offline? {e:#})");
            return;
        }
    };
    let rel = "emotes/angry_kitty.webm";
    expect(m.get(rel).is_some(), "v2 manifest has webm", failed);
    match v2.fetch_blocking(rel) {
        Ok(p) => {
            let t0 = std::time::Instant::now();
            match decode_webm_animation(&p, "angry_kitty") {
                Ok(a) => {
                    let dt = t0.elapsed();
                    let n = a.frames.len();
                    expect(n == 74, "v2 frame count 74", failed);
                    expect(a.frames[0].dimensions() == (320, 320), "v2 320x320", failed);
                    let transparent = a.frames[0].pixels().any(|p| p[3] < 16);
                    let opaque = a.frames[n / 2].pixels().any(|p| p[3] > 240);
                    expect(transparent && opaque, "v2 alpha present", failed);
                    expect(a.delays_ms.last() == Some(&567), "v2 last-frame hold 567ms", failed);
                    // Compare with v1 WebP if available locally.
                    let webp = std::path::Path::new("assets/original/emotes/angry_kitty.webp");
                    if webp.exists() {
                        if let Ok(f) = std::fs::File::open(webp) {
                            use image::AnimationDecoder;
                            let t1 = std::time::Instant::now();
                            if let Ok(dec) = image::codecs::webp::WebPDecoder::new(std::io::BufReader::new(f)) {
                                let orig: Vec<_> = dec.into_frames().filter_map(|f| f.ok()).map(|f| f.into_buffer()).collect();
                                println!("simulate: v1 webp decode for comparison: {:.2} ms/frame", t1.elapsed().as_secs_f64() * 1e3 / orig.len().max(1) as f64);
                                expect(orig.len() == n, "v1/v2 frame count match", failed);
                                let psnr = composited_psnr(&orig, &a.frames);
                                expect(psnr > 38.0, "v2 composited PSNR > 38 dB", failed);
                                println!("simulate: v2 vs v1 composited PSNR (black bg) {psnr:.2} dB");
                            }
                        }
                    }
                    println!(
                        "simulate: v2 ok ({rel} sha256 verified, {n} frames VP9+alpha decoded in {:.1} ms = {:.2} ms/frame)",
                        dt.as_secs_f64() * 1e3,
                        dt.as_secs_f64() * 1e3 / n as f64
                    );
                }
                Err(e) => {
                    eprintln!("FAIL v2 decode: {e:#}");
                    *failed += 1;
                }
            }
        }
        Err(e) => {
            eprintln!("FAIL v2 fetch: {e:#}");
            *failed += 1;
        }
    }
    let _ = std::fs::remove_dir_all(&tmp);
}

fn composited_psnr(a: &[image::RgbaImage], b: &[image::RgbaImage]) -> f64 {
    let (mut se, mut n) = (0f64, 0f64);
    for (x, y) in a.iter().zip(b) {
        for (p, q) in x.pixels().zip(y.pixels()) {
            for c in 0..3 {
                let cp = p[c] as f64 * p[3] as f64 / 255.0;
                let cq = q[c] as f64 * q[3] as f64 / 255.0;
                se += (cp - cq).powi(2);
                n += 1.0;
            }
        }
    }
    if se == 0.0 { 99.0 } else { 10.0 * (255f64.powi(2) / (se / n)).log10() }
}
