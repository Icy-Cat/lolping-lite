//! lolPing-lite — ultra-light desktop ping / emote overlay (Rust).

mod app;
mod assets;
mod audio;
mod fx;
mod gesture;
mod input;
mod overlay;
mod wheel;

use anyhow::Result;
use eframe::egui;

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 720.0])
            .with_title("lolPing-lite")
            .with_transparent(true)
            .with_decorations(true)
            .with_always_on_top()
            .with_mouse_passthrough(true)
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
