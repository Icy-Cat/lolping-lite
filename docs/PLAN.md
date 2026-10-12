# Roadmap

MVP skeleton → overlay + macOS hooks (this milestone) → wheels → Windows parity → rooms.

## Milestone 0 — Skeleton (done)

- [x] Public GitHub repo, MIT code + asset attribution
- [x] Cargo workspace with egui/eframe window
- [x] Original assets under `assets/original/`
- [x] Size-oriented release profile (LTO / strip / opt-level z)
- [x] On-demand asset loader interface (stub)

## Milestone 1 — Global input hooks

- [x] macOS: `CGEventTap` (ListenOnly) for Option / Control + mouse move
- [ ] Windows: low-level keyboard / mouse hook (`WH_KEYBOARD_LL` / `WH_MOUSE_LL`) — stub only
- [x] Modifier-hold state machine (press → drag tracking → release); wheels not drawn yet
- [x] Accessibility / privilege UX (Mac prompt + in-app message)
- [ ] Active-filter key swallow while gesture armed (still ListenOnly / conservative)

## Milestone 2 — Transparent click-through overlay

- [x] eframe transparent window + `clear_color` alpha 0 + always-on-top
- [x] Runtime toggle mouse passthrough (Idle ON · gesture/debug OFF)
- [ ] Multi-monitor placement (one overlay or per-display — decide on Mac first)
- [x] Hot path: gesture snapshot via `Arc<Mutex>` / channel; no per-move log spam
- [ ] Decorations-free fullscreen overlay surface

## Milestone 3 — Ping + emote wheels

- [ ] 8-slice ping wheel using `assets/original/textures/`
- [ ] Emote wheel slots using PNG thumbnails
- [x] On-demand WebP **first frame** decode API (full animation playback later)
- [ ] Cancel rules: center release, right-click, Esc

## Milestone 4 — Audio

- [ ] Play OGG / WAV with a tiny backend (e.g. `rodio`)
- [ ] Volume / mute settings
- [ ] No audio decode on overlay idle path

## Milestone 5 — Settings + persistence

- [ ] Trigger keys, wheel layout, sizes, launch-at-login
- [ ] Config file (TOML/JSON) next to binary or in app support dir

## Milestone 6 — Room sync (optional / later)

- [ ] LAN UDP discovery + AES-GCM payload (parity with lolPing concepts)
- [ ] Optional Nostr relay path
- [ ] Keep networking off the render thread

## Non-goals (for now)

- Hooking or reading the League client
- Replacing original WebP with lossy WebM in-repo
- Universal macOS binary (arm64 first)
