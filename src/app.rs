//! egui app: transparent overlay shell + debug HUD driven by input/overlay state.

use crate::assets::{AssetCatalog, AssetId};
use crate::input::{HookStatus, InputHub};
use crate::overlay::OverlayController;
use eframe::egui::{self, Color32, ColorImage, TextureHandle, Vec2};

pub struct LolPingApp {
    catalog: AssetCatalog,
    input: InputHub,
    overlay: OverlayController,
    status: String,
    preview: Option<TextureHandle>,
    preview_label: String,
}

impl LolPingApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let catalog = AssetCatalog::discover();
        let input = InputHub::start();
        let status = format!(
            "listed {} emotes / {} textures / {} sounds from {} (no bulk decode)",
            catalog.emote_count(),
            catalog.texture_count(),
            catalog.sound_count(),
            catalog.root().display()
        );
        Self {
            catalog,
            input,
            overlay: OverlayController::default(),
            status,
            preview: None,
            preview_label: String::new(),
        }
    }

    fn load_preview_thumb(&mut self, ctx: &egui::Context) {
        let name = self
            .catalog
            .emote(AssetId::FirstEmote)
            .map(|e| e.name.clone())
            .unwrap_or_else(|| "unknown".into());
        match self.catalog.load_emote_png_thumb(AssetId::FirstEmote) {
            Ok(rgba) => {
                let size = [rgba.width() as usize, rgba.height() as usize];
                let image = ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
                self.preview = Some(ctx.load_texture(
                    format!("thumb-{name}"),
                    image,
                    Default::default(),
                ));
                self.preview_label = format!("PNG thumb '{name}' {}×{}", size[0], size[1]);
                self.status = self.preview_label.clone();
            }
            Err(e) => self.status = format!("thumb error: {e:#}"),
        }
    }

    fn load_preview_first_frame(&mut self, ctx: &egui::Context) {
        let name = self
            .catalog
            .emote(AssetId::FirstEmote)
            .map(|e| e.name.clone())
            .unwrap_or_else(|| "unknown".into());
        // Copy pixels out so we don't hold a borrow across texture upload.
        let decoded = self
            .catalog
            .load_emote_webp_first_frame(AssetId::FirstEmote)
            .map(|img| (img.width(), img.height(), img.as_raw().to_vec()));
        match decoded {
            Ok((w, h, raw)) => {
                let size = [w as usize, h as usize];
                let image = ColorImage::from_rgba_unmultiplied(size, &raw);
                self.preview = Some(ctx.load_texture(
                    format!("webp0-{name}"),
                    image,
                    Default::default(),
                ));
                self.preview_label =
                    format!("WebP first frame '{name}' {}×{} (on-demand)", size[0], size[1]);
                self.status = self.preview_label.clone();
            }
            Err(e) => self.status = format!("webp frame error: {e:#}"),
        }
    }
}

impl eframe::App for LolPingApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.input.poll();

        let snap = self.input.shared.get();
        self.overlay.on_gesture(snap.ping_down, snap.emote_down);
        self.overlay.apply(ctx);

        ctx.request_repaint_after(std::time::Duration::from_millis(33));

        let show_chrome = self.overlay.state().ui_visible() || self.overlay.debug_pinned();

        if show_chrome {
            egui::CentralPanel::default()
                .frame(
                    egui::Frame::NONE
                        .fill(Color32::from_rgba_unmultiplied(20, 24, 32, 220))
                        .inner_margin(12.0)
                        .corner_radius(8.0),
                )
                .show(ctx, |ui| {
                    ui.heading("lolPing-lite");
                    ui.label("transparent overlay + passthrough state machine");
                    ui.separator();

                    ui.label(format!("overlay: {}", self.overlay.state().label()));
                    ui.label(format!("hook: {:?}", self.input.status));
                    match self.input.status {
                        HookStatus::NeedsAccessibility => {
                            ui.colored_label(
                                Color32::YELLOW,
                                "macOS: enable this app (or Terminal) under \
                                 System Settings → Privacy & Security → Accessibility, then restart.",
                            );
                        }
                        HookStatus::Unsupported(msg) => {
                            ui.colored_label(Color32::LIGHT_BLUE, format!("Input stub: {msg}"));
                        }
                        _ => {}
                    }

                    ui.label(format!(
                        "gesture: ping={} emote={}  mouse=({:.0},{:.0}) moves={}",
                        snap.ping_down,
                        snap.emote_down,
                        snap.mouse_x,
                        snap.mouse_y,
                        snap.move_count
                    ));
                    ui.label(&self.status);
                    ui.separator();

                    let mut debug = self.overlay.debug_pinned();
                    if ui
                        .checkbox(&mut debug, "Pin debug UI (forces passthrough OFF)")
                        .changed()
                    {
                        self.overlay.set_debug_pinned(debug);
                    }

                    ui.horizontal(|ui| {
                        if ui.button("Load PNG thumb (cheap)").clicked() {
                            self.load_preview_thumb(ctx);
                        }
                        if ui.button("Load WebP first frame (on-demand)").clicked() {
                            self.load_preview_first_frame(ctx);
                        }
                    });

                    if let Some(tex) = &self.preview {
                        ui.label(&self.preview_label);
                        ui.image((tex.id(), Vec2::splat(96.0)));
                    }

                    ui.separator();
                    ui.weak("Hold ⌥ Option = ping gesture · ⌃ Control = emote (macOS tap)");
                    ui.weak(
                        "Idle → passthrough ON · gesture/debug → OFF. ListenOnly (no key swallow).",
                    );
                    ui.weak("While click-through: hold ⌥+⌃ to re-pin debug UI.");

                    ui.collapsing("hook log", |ui| {
                        for line in self.input.recent_logs().iter().rev().take(12) {
                            ui.monospace(line);
                        }
                    });
                });
        } else {
            egui::Area::new(egui::Id::new("idle_beacon"))
                .fixed_pos(egui::pos2(12.0, 12.0))
                .show(ctx, |ui| {
                    ui.label(
                        egui::RichText::new("lolPing")
                            .size(11.0)
                            .color(Color32::from_rgba_unmultiplied(255, 255, 255, 80)),
                    );
                });
        }
    }
}
