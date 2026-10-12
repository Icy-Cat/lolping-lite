# Roadmap / status

## Done
- [x] M0 Skeleton (egui, assets, MIT + Riot attribution)
- [x] M1 macOS CGEventTap + Accessibility UX (Windows stub)
- [x] M2 Transparent overlay + runtime mouse passthrough
- [x] M3 Ping / emote wheels, place FX, on-demand WebP, Esc/center cancel
- [x] M3.5 Settings JSON (trigger, sizes, volume, mute, lang en/zh, launch-at-login macOS LaunchAgent, display name)
- [x] M4 Room code + scrypt/HKDF/AES-GCM + LAN UDP :47474 + presence/ping/emote/bye
- [x] `--simulate` headless harness + unit tests
- [x] Display enumeration (macOS real / elsewhere stub primary)

## Partial
- [ ] Multi-monitor **one window per display** — displays listed & room uses d/x/y; still **one** overlay window (maps remote to primary-local)
- [ ] Emote wheel editor UI (persists `emote_wheel` in settings; no drag-reorder UI yet)
- [ ] Pause hotkey (settings `enabled` toggle only; no global Ctrl+Alt+P hook yet)
- [ ] Audio — stub logs path (rodio blocked on rustc 1.85 / encoding_rs)
- [ ] Nostr / internet rooms — topic derived; WebSocket relay **stub**
- [ ] Windows low-level hooks — stub module only

## Deferred / stub
- [ ] Auto-update (`auto_update_check` field kept `false`; no updater binary)
- [ ] Active-filter modifier swallow
- [ ] Custom imported emotes
- [ ] Game fullscreen click-through pause

## Non-goals
- League client hooks; replacing original WebP; universal Mac binary first
