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
        Box::new(move |cc| Ok(Box::new(LolPingApp::new(cc, open_settings)))),
    )
    .map_err(|e| anyhow::anyhow!("eframe failed: {e}"))?;
    Ok(())
}
