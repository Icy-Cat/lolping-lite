# lolPing-lite

Ultra-light Rust rewrite of [darrenprx/lolPing](https://github.com/darrenprx/lolPing):
League of Legends-style desktop ping / emote wheels without Electron.

**Goals:** extreme binary size, extreme overlay performance, macOS Apple Silicon
first (Windows architecture reserved).

> This is an early MVP skeleton. Global hotkeys, click-through overlay, and the
> full wheels are not implemented yet — see [docs/PLAN.md](docs/PLAN.md).

## Why rewrite?

| | Electron lolPing | lolPing-lite (target) |
|---|---|---|
| Shell | Chromium + Electron (~150+ MiB) | Native Rust + egui |
| Install (Mac arm64) | ~175 MiB DMG / ~346 MB .app | Aim for single-digit–low tens of MiB with on-demand assets |
| UI | React + Fluent | egui / eframe |
| Input | Native C++ / Swift helper | Platform hooks (CGEventTap on Mac) |
| Overlay | Browser transparent window | egui transparent + mouse passthrough |

Business logic (wheels, rooms, AES) is straightforward; the hard parts are
hold-to-drag modifiers, runtime click-through, and multi-monitor always-on-top.

## Volume strategy

1. **No Electron / no Chromium** — biggest win.
2. **Keep original animated WebP** as the quality source of truth (see
   [ASSETS.md](ASSETS.md)). Optional WebM CRF≈10 is derivative only.
3. **On-demand asset load** — ship small PNG wheel icons + sounds in the binary
   footprint; load full WebP animations when an emote is used (interface
   stubbed in code).
4. **Release profile** — LTO, `opt-level = "z"`, strip, `panic = "abort"`.

## Assets (not MIT)

Files under `assets/original/` are © Riot Games. See [ASSETS.md](ASSETS.md).
Code is MIT; assets are not open-sourced.

```
assets/original/
  emotes/     # 44 × .webp + .png + .ogg
  textures/   # ping wheel icons
  sounds/     # ping .wav
```

## Build

Requires [Rust stable](https://rustup.rs/) (see `rust-toolchain.toml`).

```bash
cargo build --release
./target/release/lolping-lite
```

For speed-oriented releases instead of size:

```bash
cargo build --profile release-fast
```

### macOS notes

- Apple Silicon preferred.
- Accessibility permission will be required once CGEventTap hooks land.
- Transparent + mouse-passthrough windows come after the basic egui shell.

### Windows (planned)

Same crate; platform modules behind `cfg(target_os = ...)`. Not built out yet.

## Project layout

```
src/
  main.rs          # entry
  app.rs           # egui App shell
  assets.rs        # asset paths + on-demand loader stub
docs/PLAN.md       # milestones
ASSETS.md          # Riot attribution + quality policy
```

## License

- **Code:** [MIT](LICENSE)
- **Ping / emote icons, animations, sounds:** © Riot Games — not covered by MIT

Not endorsed by Riot Games.
