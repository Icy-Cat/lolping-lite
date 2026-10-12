# Roadmap

## Milestone 0 — Skeleton (done)
- [x] Public repo, MIT + asset attribution, egui shell, original assets, size profile

## Milestone 1 — Global input hooks
- [x] macOS CGEventTap ListenOnly (Option/Control + mouse)
- [ ] Windows WH_* hooks — stub
- [x] Modifier hold state machine
- [x] Accessibility UX
- [ ] Active-filter key swallow

## Milestone 2 — Transparent click-through overlay
- [x] Transparent + always-on-top + runtime passthrough
- [ ] Multi-monitor one-window-per-display (in progress → M3.5)
- [x] Hot-path gesture snapshot

## Milestone 3 — Ping + emote wheels (done this push)
- [x] 8-slice ping wheel + textures
- [x] 8-slot emote wheel + PNG thumbs
- [x] On-demand animated WebP playback (single-slot cache)
- [x] Center release / Esc cancel
- [ ] rodio audio (stub — rustc 1.85 vs encoding_rs; log path instead)

## Milestone 3.5 — Settings + i18n + launch
- [ ] Settings window (trigger, pause hotkey, sizes, volume, wheel edit, lang, launch)
- [ ] Persist JSON/TOML
- [ ] en / zh strings
- [ ] macOS Open at Login (LaunchAgent / SMAppService)

## Milestone 4 — Rooms
- [ ] Room code PING-XXXXX-XXXXX + scrypt/HKDF/AES-GCM (lolPing-compatible)
- [ ] LAN UDP broadcast port 47474
- [ ] Nostr relay path (or stub with clear TODO)
- [ ] Presence / ping / emote / bye messages

## Milestone 5 — Polish
- [ ] Auto-update — **stub / deferred** (document only; no updater binary yet)
- [ ] Windows hooks + packaging
- [ ] `--simulate` E2E without GUI

## Non-goals
- League client hooks; replacing original WebP; universal Mac binary first
