//! lolPing-lite — ultra-light desktop ping / emote overlay (Rust).
//!
//! Next steps (see `docs/PLAN.md`):
//! - transparent always-on-top window
//! - runtime mouse passthrough
//! - macOS `CGEventTap` / Windows low-level hooks for hold+drag modifiers

mod app;
mod assets;

use anyhow::Result;
use eframe::egui;

fn main() -> Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([420.0, 320.0])
            .with_title("lolPing-lite")
            // NEXT: .with_transparent(true) + always-on-top + mouse passthrough
            .with_resizable(true),
        ..Default::default()
    };

    eframe::run_native(
        "lolPing-lite",
        native_options,
        Box::new(|cc| Ok(Box::new(app::LolPingApp::new(cc)))),
    )
    .map_err(|e| anyhow::anyhow!("eframe failed: {e}"))?;

    Ok(())
}
