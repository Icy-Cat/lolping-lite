//! lolPing-lite binary entry.

use clap::Parser;
use lolping_lite::simulate;
use anyhow::Result;

#[derive(Parser, Debug)]
#[command(name = "lolping-lite", about = "Ultra-light LoL-style desktop ping / emote overlay")]
struct Cli {
    /// Headless checks: gesture state machine, room code/crypto (no window).
    #[arg(long)]
    simulate: bool,

    /// Open settings panel pinned on launch.
    #[arg(long)]
    settings: bool,
}

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let cli = Cli::parse();
    if cli.simulate {
        std::process::exit(simulate::run_simulate());
    }
    run_gui(cli.settings)
}

fn run_gui(open_settings: bool) -> Result<()> {
    use eframe::egui;
    use lolping_lite::app::LolPingApp;

    // Root viewport is the settings shell (Fluent left-nav). Overlay FX
    // lives in per-display child viewports. Start opaque + interactive so
    // `--settings` / Accessibility prompts are usable immediately.
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([960.0, 700.0])
            .with_min_inner_size([720.0, 480.0])
            .with_title("lolPing Lite — Settings")
            .with_transparent(false)
            .with_decorations(true)
            .with_resizable(true)
            .with_taskbar(true),
        ..Default::default()
    };

    eframe::run_native(
        "lolPing-lite",
        native_options,
        Box::new(move |cc| Ok(Box::new(LolPingApp::new(cc, open_settings)))),
    )
    .map_err(|e| anyhow::anyhow!("eframe failed: {e}"))?;
    Ok(())
}
