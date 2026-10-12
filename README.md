# lolPing-lite

Ultra-light **Rust** rewrite of [darrenprx/lolPing](https://github.com/darrenprx/lolPing):
desktop ping / emote wheels **without Electron**.

**Goals:** tiny binary, fast overlay, macOS Apple Silicon first.

## Status

Wheels, settings, LAN rooms, and `--simulate` are in. See [docs/FEATURES.md](docs/FEATURES.md)
and [docs/PLAN.md](docs/PLAN.md).

## Build & test

```bash
cargo build --release
cargo test --release
./target/release/lolping-lite --simulate   # headless E2E of gesture + room crypto
./target/release/lolping-lite             # GUI
./target/release/lolping-lite --settings  # GUI with settings open
```

Release binary is typically **~8 MiB** (vs Electron ~175 MiB DMG).

### macOS

1. **Accessibility** required for global ⌥/⌃ hooks (System Settings → Privacy & Security).
2. Without it, use **Alt/Ctrl inside the window** to test wheels.
3. **Open at login** writes `~/Library/LaunchAgents/com.icy-cat.lolping-lite.plist`.

### Rooms (LAN)

Create/Join a `PING-XXXXX-XXXXX` code in the debug/settings window. Same Wi‑Fi:
encrypted UDP on port **47474** (lolPing-compatible envelope). Internet/Nostr is stubbed.

### Assets

`assets/original/**` are © Riot Games — see [ASSETS.md](ASSETS.md). Code is MIT.

## Layout

```
src/lib.rs          library (testable)
src/main.rs         CLI + GUI
src/wheel.rs        ping/emote geometry
src/gesture.rs      hold→wheel→place
src/room/           code, AES-GCM, LAN, Nostr stub
src/settings.rs     JSON settings
src/simulate.rs     --simulate harness
```

## License

Code MIT; ping/emote media © Riot Games. Not endorsed by Riot Games.
