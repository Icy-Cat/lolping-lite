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
