//! App shell: gesture wheels, placed FX, debug HUD.

use crate::assets::{AssetCatalog, AssetId, EmoteAnimation};
use crate::audio::AudioOut;
use crate::fx::{self, EmoteAnim, FxLayer};
use crate::gesture::{GestureKind, GestureOutcome, GestureSession};
use crate::input::{HookStatus, InputHub};
use crate::overlay::OverlayController;
use crate::wheel::{paint_wheel, WheelPaint, PINGS, WHEEL_SLOTS};
use eframe::egui::{self, Color32, Pos2, TextureHandle};

pub struct LolPingApp {
    catalog: AssetCatalog,
    input: InputHub,
    overlay: OverlayController,
    gesture: GestureSession,
    fx: FxLayer,
    audio: AudioOut,
    status: String,
    /// Ping icons for the 8 wheel slots (loaded once).
    ping_icons: [Option<TextureHandle>; WHEEL_SLOTS],
    ping_icons_ready: bool,
    /// Emote PNG thumbs for first 8 emotes (loaded once).
    emote_icons: [Option<TextureHandle>; WHEEL_SLOTS],
    emote_slot_names: [String; WHEEL_SLOTS],
    emote_icons_ready: bool,
    /// Last decoded emote uploaded to GPU (name + frames).
    emote_gpu_cache: Option<(String, Vec<TextureHandle>, Vec<u32>)>,
}

impl LolPingApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let catalog = AssetCatalog::discover();
        let mut emote_slot_names = std::array::from_fn(|_| String::new());
        for (i, e) in catalog.emotes().iter().take(WHEEL_SLOTS).enumerate() {
            emote_slot_names[i] = e.name.clone();
        }
        let status = format!(
            "listed {} emotes / {} textures / {} sounds — wheels ready",
            catalog.emote_count(),
            catalog.texture_count(),
            catalog.sound_count(),
        );
        Self {
            catalog,
            input: InputHub::start(),
            overlay: OverlayController::default(),
            gesture: GestureSession::default(),
            fx: FxLayer::default(),
            audio: AudioOut::new(),
            status,
            ping_icons: std::array::from_fn(|_| None),
            ping_icons_ready: false,
            emote_icons: std::array::from_fn(|_| None),
            emote_slot_names,
            emote_icons_ready: false,
            emote_gpu_cache: None,
        }
    }

    fn ensure_ping_icons(&mut self, ctx: &egui::Context) {
        if self.ping_icons_ready {
            return;
        }
        for (i, def) in PINGS.iter().enumerate() {
            let path = self.catalog.texture_path(def.texture_file);
            match self.catalog.load_png(&path) {
                Ok(rgba) => {
                    self.ping_icons[i] = Some(fx::upload_rgba(
                        ctx,
                        format!("ping-{i}"),
                        rgba.width(),
                        rgba.height(),
                        rgba.as_raw(),
                    ));
                }
                Err(e) => log::warn!("ping icon {}: {e:#}", def.texture_file),
            }
        }
        self.ping_icons_ready = true;
    }

    fn ensure_emote_icons(&mut self, ctx: &egui::Context) {
        if self.emote_icons_ready {
            return;
        }
        for i in 0..WHEEL_SLOTS {
            if self.emote_slot_names[i].is_empty() {
                continue;
            }
            match self.catalog.load_emote_png_thumb(AssetId::Index(i)) {
                Ok(rgba) => {
                    self.emote_icons[i] = Some(fx::upload_rgba(
                        ctx,
                        format!("emote-thumb-{i}"),
                        rgba.width(),
                        rgba.height(),
                        rgba.as_raw(),
                    ));
                }
                Err(e) => log::warn!("emote thumb {i}: {e:#}"),
            }
        }
        self.emote_icons_ready = true;
    }

    fn cursor_local(ctx: &egui::Context, snap_x: f64, snap_y: f64, use_global: bool) -> Pos2 {
        if use_global {
            let outer = ctx.input(|i| i.viewport().outer_rect);
            if let Some(outer) = outer {
                return Pos2::new(snap_x as f32 - outer.min.x, snap_y as f32 - outer.min.y);
            }
        }
        ctx.pointer_latest_pos()
            .or_else(|| ctx.input(|i| i.pointer.hover_pos()))
            .unwrap_or_else(|| {
                let rect = ctx.input(|i| i.screen_rect());
                rect.center()
            })
    }

    fn handle_outcome(&mut self, ctx: &egui::Context, outcome: GestureOutcome) {
        match outcome {
            GestureOutcome::None => {}
            GestureOutcome::Cancelled => {
                self.status = "cancelled".into();
            }
            GestureOutcome::Placed { kind, slot, at } => match kind {
                GestureKind::Ping => self.place_ping(ctx, slot, at),
                GestureKind::Emote => self.place_emote(ctx, slot, at),
            },
        }
    }

    fn place_ping(&mut self, ctx: &egui::Context, slot: usize, at: Pos2) {
        self.ensure_ping_icons(ctx);
        let def = &PINGS[slot.min(WHEEL_SLOTS - 1)];
        if let Some(tex) = self.ping_icons[slot].clone() {
            self.fx.push_ping(at, tex);
        }
        let sound = def.sound_file.map(|f| self.catalog.sound_path(f));
        self.audio.play_optional(sound);
        self.status = format!("ping '{}' at ({:.0},{:.0})", def.label, at.x, at.y);
        log::info!("{}", self.status);
    }

    fn place_emote(&mut self, ctx: &egui::Context, slot: usize, at: Pos2) {
        let name = self.emote_slot_names[slot.min(WHEEL_SLOTS - 1)].clone();
        if name.is_empty() {
            self.status = "empty emote slot".into();
            return;
        }

        // Decode animation on demand (single-slot CPU cache + GPU upload cache).
        let need_decode = self
            .emote_gpu_cache
            .as_ref()
            .map(|(n, _, _)| n != &name)
            .unwrap_or(true);

        if need_decode {
            match self.catalog.load_emote_animation(AssetId::Index(slot)) {
                Ok(anim) => {
                    let uploaded = upload_anim(ctx, anim);
                    self.emote_gpu_cache = Some(uploaded);
                }
                Err(e) => {
                    self.status = format!("emote decode failed: {e:#}");
                    return;
                }
            }
        }

        if let Some((n, frames, delays)) = &self.emote_gpu_cache {
            if n == &name {
                self.fx.push_emote(
                    at,
                    EmoteAnim {
                        name: n.clone(),
                        frames: frames.clone(),
                        delays_ms: delays.clone(),
                    },
                );
            }
        }

        if let Some(entry) = self.catalog.emote(AssetId::Index(slot)) {
            let ogg = entry.ogg.clone();
            self.audio.play_file(&ogg);
        }
        self.status = format!("emote '{name}' at ({:.0},{:.0})", at.x, at.y);
        log::info!("{}", self.status);
    }
}

fn upload_anim(ctx: &egui::Context, anim: &EmoteAnimation) -> (String, Vec<TextureHandle>, Vec<u32>) {
    let mut frames = Vec::with_capacity(anim.frames.len());
    for (i, img) in anim.frames.iter().enumerate() {
        frames.push(fx::upload_rgba(
            ctx,
            format!("emote-{}-{i}", anim.name),
            img.width(),
            img.height(),
            img.as_raw(),
        ));
    }
    (anim.name.clone(), frames, anim.delays_ms.clone())
}

impl eframe::App for LolPingApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.input.poll();
        let snap = self.input.shared.get();

        let hooks_live = matches!(
            self.input.status,
            HookStatus::Running | HookStatus::Starting
        );

        // Local fallback: Alt = ping, Ctrl = emote (works on Linux / without Accessibility).
        let (ping_down, emote_down) = if hooks_live && (snap.ping_down || snap.emote_down || snap.move_count > 0)
        {
            (snap.ping_down, snap.emote_down)
        } else {
            ctx.input(|i| (i.modifiers.alt, i.modifiers.ctrl))
        };

        let use_global = hooks_live && snap.move_count > 0;
        let cursor = Self::cursor_local(ctx, snap.mouse_x, snap.mouse_y, use_global);
        let cancel = ctx.input(|i| i.key_pressed(egui::Key::Escape));

        let outcome = self
            .gesture
            .tick(ping_down, emote_down, cursor, cancel);
        self.handle_outcome(ctx, outcome);

        if self.gesture.wheel_active() {
            match self.gesture.wheel.as_ref().map(|w| w.kind) {
                Some(GestureKind::Ping) => self.ensure_ping_icons(ctx),
                Some(GestureKind::Emote) => self.ensure_emote_icons(ctx),
                None => {}
            }
        }

        self.overlay.sync(
            ping_down,
            emote_down,
            self.gesture.overlay_hint(),
            false,
        );
        self.overlay.apply(ctx);

        // Animate FX + wheels every frame while busy.
        if self.gesture.wheel_active() || self.fx.active() {
            ctx.request_repaint();
        } else {
            ctx.request_repaint_after(std::time::Duration::from_millis(33));
        }

        // Full-viewport transparent layer for wheel + FX.
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                self.fx.tick_and_paint(ui);

                if let Some(w) = self.gesture.wheel.clone() {
                    let icons: &[Option<TextureHandle>] = match w.kind {
                        GestureKind::Ping => &self.ping_icons,
                        GestureKind::Emote => &self.emote_icons,
                    };
                    paint_wheel(
                        ui,
                        WheelPaint {
                            center: w.center,
                            cursor,
                            hover: w.hover,
                            icons,
                            kind_label: match w.kind {
                                GestureKind::Ping => "PING (release on slice · center cancels)",
                                GestureKind::Emote => "EMOTE (release on slot · center cancels)",
                            },
                        },
                    );
                }
            });

        if self.overlay.debug_pinned() {
            egui::Window::new("lolPing-lite debug")
                .default_pos(egui::pos2(16.0, 16.0))
                .resizable(true)
                .show(ctx, |ui| {
                    ui.label(format!("overlay: {}", self.overlay.state().label()));
                    ui.label(format!("hook: {:?}", self.input.status));
                    match self.input.status {
                        HookStatus::NeedsAccessibility => {
                            ui.colored_label(
                                Color32::YELLOW,
                                "Enable Accessibility for this app / Terminal, then restart.",
                            );
                        }
                        HookStatus::Unsupported(msg) => {
                            ui.colored_label(
                                Color32::LIGHT_BLUE,
                                format!("{msg} — use Alt/Ctrl + mouse in this window to test."),
                            );
                        }
                        _ => {}
                    }
                    ui.label(format!(
                        "mods ping={} emote={}  cursor=({:.0},{:.0})  {}",
                        ping_down,
                        emote_down,
                        cursor.x,
                        cursor.y,
                        self.gesture.hover_label()
                    ));
                    ui.label(&self.status);
                    ui.separator();
                    let mut debug = self.overlay.debug_pinned();
                    if ui
                        .checkbox(&mut debug, "Pin debug UI (passthrough OFF while pinned)")
                        .changed()
                    {
                        self.overlay.set_debug_pinned(debug);
                    }
                    ui.weak("⌥/Alt + drag = ping · ⌃/Ctrl + drag = emote · Esc cancel");
                    ui.weak("Unpin debug for click-through idle; ⌥+⌃ re-pins.");
                    ui.collapsing("hook log", |ui| {
                        for line in self.input.recent_logs().iter().rev().take(10) {
                            ui.monospace(line);
                        }
                    });
                });
        } else if !self.gesture.wheel_active() && !self.fx.active() {
            egui::Area::new(egui::Id::new("idle_beacon"))
                .fixed_pos(egui::pos2(12.0, 12.0))
                .show(ctx, |ui| {
                    ui.label(
                        egui::RichText::new("lolPing")
                            .size(11.0)
                            .color(Color32::from_rgba_unmultiplied(255, 255, 255, 70)),
                    );
                });
        }
    }
}
