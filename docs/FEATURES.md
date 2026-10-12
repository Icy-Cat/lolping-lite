# Feature checklist vs darrenprx/lolPing

| Feature | Upstream | lolping-lite | Notes |
|--------|----------|--------------|-------|
| Ping wheel 8 slices | ✓ | **Pass** | editable slots in settings |
| Emote wheel 8 slots | ✓ | **Pass** | editable; on-demand WebP |
| Hold Option/Alt ping | ✓ | **Pass** | settings-driven trigger map |
| Hold Control emote | ✓ | **Pass** | settings-driven |
| Center / Esc cancel | ✓ | **Pass** | |
| Transparent click-through | ✓ | **Pass** | per-display overlays |
| Sound effects | ✓ | **Pass** | rodio wav + ffmpeg Opus/ogg |
| Settings window | ✓ | **Pass** | |
| Trigger key select | ✓ | **Pass** | SharedConfig → hooks |
| Pause hotkey Ctrl+Alt+P | ✓ | **Pass** | hooks + in-window |
| Ping/emote size | ✓ | **Pass** | persisted |
| Volume / mute | ✓ | **Pass** | drives AudioOut |
| Wheel layout edit | ✓ | **Pass** | combo editors for 8+8 slots |
| Language en/zh | ✓ | **Pass** | |
| Open at login (macOS) | ✓ | **Pass** | LaunchAgent |
| Multi-monitor overlays | ✓ | **Pass** | one viewport per display |
| Room LAN UDP | ✓ | **Pass** | :47474 AES-GCM |
| Room Nostr/internet | ✓ | **Pass** | WSS relays + kind 24747 |
| Remote ping/emote place | ✓ | **Pass** | anim on receive |
| Auto-update | ✓ | **Stub** | deferred by design |
| Windows hooks | ✓ | **Pass** | WH_* skeleton (cfg windows) |
| Accessibility UX | ✓ | **Pass** | macOS |

## Asset hosting

| Item | Status |
|---|---|
| Run without bundled assets (local → cache → CDN) | ✅ |
| sha256 + size verify, atomic cache writes | ✅ |
| Non-blocking on-demand fetch, wheel prefetch at startup | ✅ |
| Manifest cache (1-day refresh), offline from cache | ✅ |
| Settings: download all (zip, progress), cache size, clear cache | ✅ |
| `LOLPING_ASSET_BASE` override | ✅ |
| `--simulate` live CDN check (skips offline) | ✅ |
| v2 VP9+alpha WebM emotes, in-app decoder, v1 fallback | ✅ |

### Details

The binary does **not** need bundled assets. Lookup order per file (path relative to `assets/original`):

1. Local bundle — `assets/original` (dev) or `<App>.app/Contents/Resources/assets/original`
2. User cache — `~/Library/Caches/lolping-lite/v1` (macOS), `$XDG_CACHE_HOME`/`~/.cache/lolping-lite/v1` (Linux), `%LOCALAPPDATA%\lolping-lite\v1` (Windows)
3. CDN — `https://lolping-assets.icy-cat.com/v1/<path>`, downloaded on a background thread, size + sha256 checked against
   [`manifest.json`](https://lolping-assets.icy-cat.com/v1/manifest.json), written atomically into the cache.

The UI never waits on the network: missing icons/emotes are skipped (or show nothing) until they land, then pick up automatically.
At startup the current ping/emote wheel's textures, emotes and sounds are prefetched. The manifest is cached and re-fetched when older than 1 day;
offline the app runs from cache. Settings → **Assets** has *Download all assets* (full
[zip](https://lolping-assets.icy-cat.com/v1/lolping-assets-v1.zip), every entry verified, with progress), cache size and *Clear cache*.

Env overrides: `LOLPING_ASSET_BASE` (CDN base URL), `LOLPING_CACHE_DIR` (cache root), `LOLPING_NO_LOCAL_ASSETS=1` (ignore local bundle),
`LOLPING_OFFLINE=1` (skip simulate's network check), `LOLPING_SIM_FULL=1` (simulate also tests full-zip download).

Publishing new assets: `set -a; . ~/.config/lolping/r2.env; set +a; python3 scripts/upload-assets.py` (credentials from env only).

### v2 assets: VP9 WebM emotes

`https://lolping-assets.icy-cat.com/v2/` has the same layout as v1, but the 44 animated emotes are
transparent VP9 WebM (`emotes/<name>.webm`, yuva420p, CRF 10, original 320×320, same frame count,
per-frame timing kept with VFR timestamps; the last frame's hold is in the stream tag `LAST_FRAME_MS`).
PNG/OGG/WAV are byte-identical to v1. Total size: v1 55.7 MB → v2 35.1 MB (emotes 50.0 → 29.4 MB),
zip 52 → 33.9 MB. Mean composited PSNR vs the WebP originals: 44.7 dB (min 41.6), alpha 53.9 dB.

The app prefers v2 when it was built with the in-app VP9 decoder (static, decoder-only libvpx through a
tiny C shim + a ~250-line WebM demuxer in `src/webm.rs`). If a WebM is missing, fails to download, or
fails to decode, it falls back to the v1 WebP for that emote. Builds without libvpx compile fine and use v1 only.

```bash
scripts/build-libvpx.sh            # → vendor/libvpx (needs make, cc, nasm on x86)
cargo build --release              # build.rs links it automatically (LIBVPX_DIR to override, LOLPING_NO_VP9=1 to skip)
scripts/build-v2-assets.sh         # assets/original → dist/assets-v2 (+ manifest.json, zip); originals untouched
scripts/verify-v2-assets.py        # frame counts, alpha, composited PSNR
R2_PREFIX=v2 python3 scripts/upload-assets.py dist/assets-v2   # with R2_* env from ~/.config/lolping/r2.env
```

Runtime env: `LOLPING_NO_V2=1` (force v1), `LOLPING_ASSET_BASE_V2` (override v2 base; default = v1 base with `/v1`→`/v2`).
Settings → Download all fetches the v2 zip when VP9 is available (its png/ogg/wav also serve v1 lookups).

## Upstream sync up to f24a51e (darrenprx/lolPing)

| Upstream commit(s) | What | Status in lolping-lite |
|---|---|---|
| 6030e94, 645cad9, bd64923, 684de87 | Pause wheel gestures while a fullscreen app/game owns the monitor under the cursor; maximized windows don't count; no WS_CAPTION check (CS2 keeps it) except where the monitor has no work area | ✅ Windows: same probe (`src/input/fullscreen.rs`, rule in `decision::covers_monitor`). macOS: best effort — front-most layer-0 window of another app whose CGWindowList bounds contain the display (zoomed windows stop under the menu bar). Setting **Pause in fullscreen apps** (`game_check`, default on) + en/zh strings as upstream. Probed only when a gesture would start. |
| c104b08 | Trigger that is part of another chord (Alt+Tab, Win+Tab, Alt+F4, Cmd+Tab …) passes through; cleared when every trigger is released / on key re-sync | ✅ both platforms. lolping-lite opens its wheel on trigger-down, so a foreign key before any slot is chosen drops the wheel silently and blocks gestures until release. Esc stays the wheel's cancel key; modifiers never mark. |
| 9256530, e2e26a1 | SendInput only from the hook thread's message loop (FIFO queue, "LPIN" marker); real input that would overtake queued injects is swallowed and replayed; injected keys carry scan codes | ✅ in `src/input/windows.rs` (`decision::InjectQueue`). lolping-lite's gestures are listen-only today, so the queue is normally empty — the ordering guarantees are in place for any future inject. |
| 7b118fc, f24a51e, b1181e9 | Helper-process watchdog, settings-save test flake, version bump | n/a (no separate helper process / Electron) |

Tests mirroring `helper_tests.cpp`: `src/input/decision.rs`, `src/gesture.rs` (`cargo test`), plus `--simulate` "upstream sync" checks.
