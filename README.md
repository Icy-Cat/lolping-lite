# lolPing-lite

Ultra-light Rust rewrite of [darrenprx/lolPing](https://github.com/darrenprx/lolPing):
League of Legends-style desktop ping / emote wheels without Electron.

**Goals:** extreme binary size, extreme overlay performance, macOS Apple Silicon
first (Windows architecture reserved).

## Status

Milestone 1–2 in progress: transparent always-on-top window, runtime mouse
passthrough state machine, macOS `CGEventTap` skeleton, on-demand WebP first-frame
load. Full wheels / audio / rooms are not done — see [docs/PLAN.md](docs/PLAN.md).

## Why rewrite?

| | Electron lolPing | lolPing-lite (target) |
|---|---|---|
| Shell | Chromium + Electron (~150+ MiB) | Native Rust + egui |
| Install (Mac arm64) | ~175 MiB DMG / ~346 MB .app | Aim for single-digit–low tens of MiB with on-demand assets |
| UI | React + Fluent | egui / eframe |
| Input | Native C++ / Swift helper | `CGEventTap` on Mac (Windows hooks later) |
| Overlay | Browser transparent window | egui transparent + mouse passthrough |

## Volume strategy

1. **No Electron / no Chromium** — biggest win.
2. **Keep original animated WebP** as the quality source of truth (see
   [ASSETS.md](ASSETS.md)). Optional WebM CRF≈10 is derivative only.
3. **On-demand asset load** — startup only lists paths; PNG thumbs / WebP first
   frame decode when requested (never preload all ~50 MB animations).
4. **Release profile** — LTO, `opt-level = "z"`, strip, `panic = "abort"`.

## Assets (not MIT)

Files under `assets/original/` are © Riot Games. See [ASSETS.md](ASSETS.md).
Code is MIT; assets are not open-sourced.

## Build

Requires [Rust stable](https://rustup.rs/) (see `rust-toolchain.toml`).

```bash
cargo build --release
./target/release/lolping-lite
```

Useful logs:

```bash
RUST_LOG=info ./target/release/lolping-lite
```

### macOS — Accessibility permission (required for global hooks)

Global Option / Control hold detection uses **`CGEventTap`**, which needs
**Privacy & Security → Accessibility** for the app that launches lolPing-lite
(the binary itself, or Terminal / your IDE if you run from there).

1. Build and run once — macOS may prompt; if not, open **System Settings**.
2. Enable the hosting app under **Accessibility**.
3. Restart lolPing-lite.

Without permission the UI still opens and shows a yellow notice; hooks stay
inactive (`NeedsAccessibility`).

**Try gestures:** unpin “Pin debug UI”, then hold **⌥ Option** (ping) or
**⌃ Control** (emote). Hold **⌥+⌃** together to re-pin debug while click-through.

**Swallow policy:** the tap is **ListenOnly** — modifiers are not swallowed yet.

### Windows (planned)

Same crate; `src/input/windows.rs` is a stub. No low-level hooks yet.

### Linux

Builds for CI; global hooks report `Unsupported`. Overlay APIs are best-effort
under X11/Wayland.

## Project layout

```
src/
  main.rs          # transparent + always-on-top + initial passthrough
  app.rs           # debug HUD / preview
  overlay.rs       # Idle · Ping · Emote · DebugUi state machine
  assets.rs        # list + on-demand PNG / WebP first frame
  input/
    mod.rs         # hub + crate evaluation notes
    macos.rs       # CGEventTap glue
    windows.rs     # stub
    stub.rs        # non-Mac/Win
docs/PLAN.md
ASSETS.md
```

## License

- **Code:** [MIT](LICENSE)
- **Ping / emote icons, animations, sounds:** © Riot Games — not covered by MIT

Not endorsed by Riot Games.
