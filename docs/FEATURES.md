# Feature checklist vs darrenprx/lolPing

| Feature | Upstream | lolping-lite | Notes |
|--------|----------|--------------|-------|
| Ping wheel 8 slices | ✓ | **Pass** | textures + place FX |
| Emote wheel 8 slots | ✓ | **Pass** | PNG thumbs + on-demand WebP anim |
| Hold Option/Alt ping | ✓ | **Pass** | macOS tap / in-window Alt |
| Hold Control emote | ✓ | **Pass** | macOS tap / in-window Ctrl |
| Center / Esc cancel | ✓ | **Pass** | |
| Transparent click-through | ✓ | **Pass** | Idle ON / wheel OFF |
| Sound effects | ✓ | **Partial** | stub logs path |
| Settings window | ✓ | **Pass** | egui panel + JSON |
| Trigger key select | ✓ | **Partial** | Alt/Ctrl/Shift stored; hooks still Alt/Ctrl hardware |
| Pause hotkey | ✓ | **Partial** | `enabled` in settings only |
| Ping/emote size | ✓ | **Pass** | saved (size applied to FX gradually) |
| Volume / mute | ✓ | **Partial** | mute gates stub audio |
| Wheel layout edit | ✓ | **Partial** | defaults persisted; no DnD editor |
| Language en/zh | ✓ | **Pass** | |
| Open at login (macOS) | ✓ | **Pass** | LaunchAgent plist |
| Multi-monitor overlays | ✓ | **Partial** | enum + room d/x/y; one window |
| Room LAN UDP | ✓ | **Pass** | :47474 AES-GCM |
| Room Nostr/internet | ✓ | **Partial** | topic stub only |
| Room presence/name tag | ✓ | **Partial** | presence + members list; no name tag draw |
| Auto-update | ✓ | **Stub** | deferred in PLAN |
| Windows hooks | ✓ | **Stub** | `src/input/windows.rs` |
| Accessibility UX | ✓ | **Pass** | macOS |
