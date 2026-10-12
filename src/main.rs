//! lolPing-lite — ultra-light desktop ping / emote overlay (Rust).

mod app;
mod assets;
mod input;
mod overlay;

use anyhow::Result;
use eframe::egui;

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([480.0, 420.0])
            .with_title("lolPing-lite")
            .with_transparent(true)
            .with_decorations(true)
            .with_always_on_top()
            // Start click-through; OverlayController toggles at runtime.
            .with_mouse_passthrough(true)
            .with_resizable(true),
        ..Default::default()
    };

    // Fallback note: if the compositor rejects transparency, egui still runs with
    // an opaque clear — clear_color alpha 0 is best-effort. Passthrough uses
    // winit `set_cursor_hittest`; failures are logged inside egui-winit.

    eframe::run_native(
        "lolPing-lite",
        native_options,
        Box::new(|cc| Ok(Box::new(app::LolPingApp::new(cc)))),
    )
    .map_err(|e| anyhow::anyhow!("eframe failed: {e}"))?;

    Ok(())
}
