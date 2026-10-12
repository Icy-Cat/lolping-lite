# Roadmap

MVP skeleton (this commit) → usable Mac overlay → Windows parity → rooms.

## Milestone 0 — Skeleton (done)

- [x] Public GitHub repo, MIT code + asset attribution
- [x] Cargo workspace with egui/eframe window
- [x] Original assets under `assets/original/`
- [x] Size-oriented release profile (LTO / strip / opt-level z)
- [x] On-demand asset loader interface (stub)

## Milestone 1 — Global input hooks

- [ ] macOS: `CGEventTap` for hold Option / Control + mouse drag
- [ ] Windows: low-level keyboard / mouse hook (`WH_KEYBOARD_LL` / `WH_MOUSE_LL`)
- [ ] Modifier-hold state machine (press → drag → release / cancel)
- [ ] Accessibility / privilege UX (Mac permission prompt)

## Milestone 2 — Transparent click-through overlay

- [ ] eframe / winit transparent window, always-on-top
- [ ] Runtime toggle mouse passthrough (ignore clicks when idle)
- [ ] Multi-monitor placement (one overlay or per-display — decide on Mac first)
- [ ] Hot path: no per-frame heap alloc for wheel geometry

## Milestone 3 — Ping + emote wheels

- [ ] 8-slice ping wheel using `assets/original/textures/`
- [ ] Emote wheel slots using PNG thumbnails
- [ ] Play animated WebP on release (decode + cache; keep original quality)
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
