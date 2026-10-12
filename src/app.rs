//! Minimal egui shell. Overlay / passthrough / wheels come later.

use crate::assets::{AssetCatalog, AssetId};
use eframe::egui;

pub struct LolPingApp {
    catalog: AssetCatalog,
    status: String,
}

impl LolPingApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let catalog = AssetCatalog::discover();
        let status = format!(
            "assets: {} emotes, {} textures, {} sounds (lazy decode)",
            catalog.emote_count(),
            catalog.texture_count(),
            catalog.sound_count()
        );
        Self { catalog, status }
    }
}

impl eframe::App for LolPingApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Hot path: keep this allocation-light once the overlay is real.
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("lolPing-lite");
            ui.label("Rust + egui rewrite — MVP skeleton");
            ui.separator();
            ui.label(&self.status);
            ui.add_space(8.0);

            ui.label("Quality policy: runtime uses original animated WebP.");
            ui.label("WebM CRF≈10 is optional / derived only (never replaces originals).");
            ui.add_space(8.0);

            if ui.button("Probe first emote path (no decode)").clicked() {
                match self.catalog.emote_animation_path(AssetId::FirstEmote) {
                    Some(p) => self.status = format!("emote webp: {}", p.display()),
                    None => self.status = "no emote assets found".into(),
                }
            }

            ui.add_space(12.0);
            ui.weak("TODO: transparent overlay · mouse passthrough · CGEventTap hold+drag");
            ui.weak("See docs/PLAN.md");
        });
    }
}
