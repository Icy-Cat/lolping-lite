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

