# Settings UI parity: lolPing (Electron) → lolPing-lite (egui)

## Upstream inventory (darrenprx/lolPing)

Layout (`App.tsx` + `settings.css`): 40px titlebar · 240px side nav · scrollable main.
Nav sections: **Trigger · Toggle · Pings & sound · Wheel · Emotes · Room · App**.
Theme: Fluent UI (webDark / webLight; macOS custom tokens). Cards = 68px min-height rows with icon · title · desc · control.

| Section | Controls |
|---|---|
| Status | green/off/failed dot, enable switch, Accessibility permission card (macOS) |
| Trigger | trigger key picker, drag distance, click-ping switch, gameCheck (Win; Mac in lite) |
| Toggle | pause hotkey capture, start enabled |
| Pings | size, duration, volume, mute, tick sound |
| Wheel | pie editor + pool (drag/click-to-place), reset, preview |
| Emotes | emote key, emote-click, size, sound, emote wheel editor, custom import |
| Room | name, colour swatches, join/create/leave, members, mute, rate limit, internet, rejoin, displays, privacy note |
| App | launch at login, language, auto-update, about, limitations |

i18n: `en` + `zh-CN`, platform-specific Mac strings (⌥⌃⌘, login items, Finder).

## lite before

Single crude `egui::Window` on the transparent overlay root: debug pin checkbox, Dump-style combos for wheels, room join, asset buttons. Looked like a HUD, not the Fluent settings app.

## lite after (this change)

Dedicated **settings viewport** (~960×700, decorated, opaque dark Fluent-inspired theme):
- Side nav + section content matching upstream grouping/labels (en/zh)
- Status card + macOS Accessibility banner (styled)
- Card rows for switches/sliders/combos
- Ping wheel: click-to-assign pie + pool tiles (icons when cached)
- Emote wheel: per-slot combo + pool pick
- Room: name, colours, join/create/leave/copy, internet, mute, displays
- App: launch at login, language, CDN assets download/clear, about, limitations
- Developer collapsing log (replaces always-on debug HUD)
- `--settings` / Accessibility / ⌥+⌃ open this window; overlays stay click-through

## Gaps (intentional / follow-ups)

| Item | Status |
|---|---|
| Fluent React components / light theme | egui approximation, dark only |
| Full drag-and-drop wheel editor | click-to-place; no DnD |
| Custom emote import / remove | not in lite |
| Mouse4/5 / custom key capture | modifiers only |
| Live hotkey rebinding UI | label shown; chord fixed Ctrl+Alt+P |
| clickPing / dragThreshold behaviour | **settings stored**; gesture still opens on press (behaviour gap) |
| tickSound playback | setting stored; not wired to hover audio yet |
| Room members mute / rate limit / rejoin | mute+internet yes; members list / limit / rejoin stub |
| Auto-update | stub (as designed) |
| Tray / menu bar | none yet |
| Pixel-perfect Mac vibrancy titlebar | standard OS chrome |

## How to open

```bash
./target/release/lolping-lite --settings
# or hold Option+Control while running
```
