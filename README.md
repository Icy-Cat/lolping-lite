# lolPing-lite

Ultra-light **Rust** rewrite of [darrenprx/lolPing](https://github.com/darrenprx/lolPing):
desktop ping / emote wheels **without Electron**.

**Goals:** tiny binary, fast overlay, macOS Apple Silicon first.

## Status

Wheels, settings, LAN rooms, and `--simulate` are in. See [docs/FEATURES.md](docs/FEATURES.md)
and [docs/PLAN.md](docs/PLAN.md).

## Build & test

```bash
cargo build --release
cargo test --release
./target/release/lolping-lite --simulate   # headless E2E of gesture + room crypto
./target/release/lolping-lite             # GUI
./target/release/lolping-lite --settings  # GUI with settings open
```

Release binary is typically **~8 MiB** (vs Electron ~175 MiB DMG).

### macOS

1. **Accessibility** required for global ⌥/⌃ hooks (System Settings → Privacy & Security).
2. Without it, use **Alt/Ctrl inside the window** to test wheels.
3. **Open at login** writes `~/Library/LaunchAgents/com.icy-cat.lolping-lite.plist`.

### Rooms (LAN)

Create/Join a `PING-XXXXX-XXXXX` code in the debug/settings window. Same Wi‑Fi:
encrypted UDP on port **47474** (lolPing-compatible envelope). Internet/Nostr is stubbed.

### Assets

`assets/original/**` are © Riot Games — see [ASSETS.md](ASSETS.md). Code is MIT.

### Asset hosting (Cloudflare R2)

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

### Games / other chords

- **Pause in fullscreen apps** (default on): holding the trigger while a fullscreen app (a game) owns the monitor under the cursor does nothing; gestures resume on the desktop and on other monitors. Maximized windows don't count.
- A trigger that is part of another chord (Alt+Tab, Win+Tab, Alt+F4, Cmd+Tab …) doesn't open/place a wheel until it is released.

## Layout

```
src/lib.rs          library (testable)
src/main.rs         CLI + GUI
src/wheel.rs        ping/emote geometry
src/gesture.rs      hold→wheel→place
src/room/           code, AES-GCM, LAN, Nostr stub
src/settings.rs     JSON settings
src/simulate.rs     --simulate harness
```

## License

Code MIT; ping/emote media © Riot Games. Not endorsed by Riot Games.
