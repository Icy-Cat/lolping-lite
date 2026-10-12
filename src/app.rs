//! GUI app: wheels, FX, settings, room.

use crate::assets::{AssetCatalog, AssetId, EmoteAnimation};
use crate::audio::AudioOut;
use crate::display::{self, DisplayInfo};
use crate::fx::{self, EmoteAnim, FxLayer};
use crate::gesture::{GestureKind, GestureOutcome, GestureSession};
use crate::i18n::{self, strings};
use crate::input::{HookStatus, InputHub};
use crate::overlay::OverlayController;
use crate::room::{RoomEvent, RoomHub};
use crate::settings::{Settings, TriggerKey};
use crate::wheel::{paint_wheel, WheelPaint, PINGS, WHEEL_SLOTS};
use eframe::egui::{self, Pos2, TextureHandle};
use std::collections::VecDeque;

pub struct LolPingApp {
    catalog: AssetCatalog,
    input: InputHub,
    overlay: OverlayController,
    gesture: GestureSession,
    fx: FxLayer,
    audio: AudioOut,
    settings: Settings,
    show_settings: bool,
    room: RoomHub,
    room_code_input: String,
    displays: Vec<DisplayInfo>,
    status: String,
    ping_icons: [Option<TextureHandle>; WHEEL_SLOTS],
    ping_icons_ready: bool,
    emote_icons: [Option<TextureHandle>; WHEEL_SLOTS],
    emote_slot_names: [String; WHEEL_SLOTS],
    emote_icons_ready: bool,
    emote_gpu_cache: Option<(String, Vec<TextureHandle>, Vec<u32>)>,
    event_log: VecDeque<String>,
}

impl LolPingApp {
    pub fn new(_cc: &eframe::CreationContext<'_>, open_settings: bool) -> Self {
        let mut settings = Settings::load();
        let catalog = AssetCatalog::discover();
        let names: Vec<String> = catalog.emotes().iter().map(|e| e.name.clone()).collect();
        settings.ensure_emote_wheel(&names);
        let mut emote_slot_names = std::array::from_fn(|_| String::new());
        for (i, n) in settings.emote_wheel.iter().take(WHEEL_SLOTS).enumerate() {
            emote_slot_names[i] = n.clone();
        }
        let room = RoomHub::new(&settings);
        let displays = display::list_displays();
        let status = format!(
            "{} emotes · {} displays · settings {}",
            catalog.emote_count(),
            displays.len(),
            Settings::config_path().display()
        );
        Self {
            catalog,
            input: InputHub::start(),
            overlay: OverlayController::default(),
            gesture: GestureSession::default(),
            fx: FxLayer::default(),
            audio: AudioOut::new(),
            settings,
            show_settings: open_settings,
            room,
            room_code_input: String::new(),
            displays,
            status,
            ping_icons: std::array::from_fn(|_| None),
            ping_icons_ready: false,
            emote_icons: std::array::from_fn(|_| None),
            emote_slot_names,
            emote_icons_ready: false,
            emote_gpu_cache: None,
            event_log: VecDeque::new(),
        }
    }

    fn log(&mut self, line: impl Into<String>) {
        let line = line.into();
        log::info!("{line}");
        self.event_log.push_front(line);
        while self.event_log.len() > 30 {
            self.event_log.pop_back();
        }
    }

    fn ensure_ping_icons(&mut self, ctx: &egui::Context) {
        if self.ping_icons_ready {
            return;
        }
        for (i, def) in PINGS.iter().enumerate() {
            let path = self.catalog.texture_path(def.texture_file);
            if let Ok(rgba) = self.catalog.load_png(&path) {
                self.ping_icons[i] = Some(fx::upload_rgba(
                    ctx,
                    format!("ping-{i}"),
                    rgba.width(),
                    rgba.height(),
                    rgba.as_raw(),
                ));
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
            // Resolve by name in catalog
            if let Some((idx, _)) = self
                .catalog
                .emotes()
                .iter()
                .enumerate()
                .find(|(_, e)| e.name == self.emote_slot_names[i])
            {
                if let Ok(rgba) = self.catalog.load_emote_png_thumb(AssetId::Index(idx)) {
                    self.emote_icons[i] = Some(fx::upload_rgba(
                        ctx,
                        format!("emote-thumb-{i}"),
                        rgba.width(),
                        rgba.height(),
                        rgba.as_raw(),
                    ));
                }
            }
        }
        self.emote_icons_ready = true;
    }

    fn cursor_local(ctx: &egui::Context, snap_x: f64, snap_y: f64, use_global: bool) -> Pos2 {
        if use_global {
            if let Some(outer) = ctx.input(|i| i.viewport().outer_rect) {
                return Pos2::new(snap_x as f32 - outer.min.x, snap_y as f32 - outer.min.y);
            }
        }
        ctx.pointer_latest_pos()
            .or_else(|| ctx.input(|i| i.pointer.hover_pos()))
            .unwrap_or_else(|| ctx.input(|i| i.screen_rect()).center())
    }

    fn handle_outcome(&mut self, ctx: &egui::Context, outcome: GestureOutcome, cursor: Pos2) {
        match outcome {
            GestureOutcome::None => {}
            GestureOutcome::Cancelled => self.status = "cancelled".into(),
            GestureOutcome::Placed { kind, slot, at } => match kind {
                GestureKind::Ping => self.place_ping(ctx, slot, at, cursor),
                GestureKind::Emote => self.place_emote(ctx, slot, at, cursor),
            },
        }
    }

    fn place_ping(&mut self, ctx: &egui::Context, slot: usize, at: Pos2, global_hint: Pos2) {
        self.ensure_ping_icons(ctx);
        let def = &PINGS[slot.min(WHEEL_SLOTS - 1)];
        if let Some(tex) = self.ping_icons[slot].clone() {
            self.fx.push_ping(at, tex);
        }
        if !self.settings.muted {
            if let Some(s) = def.sound_file {
                self.audio.play_file(&self.catalog.sound_path(s));
            }
        }
        let (d, fx, fy) =
            display::point_to_frac(&self.displays, global_hint.x as f64, global_hint.y as f64);
        self.room.broadcast_ping(def.id, d, fx, fy);
        self.status = format!("ping '{}' d{d} ({fx:.2},{fy:.2})", def.label);
        self.log(self.status.clone());
    }

    fn place_emote(&mut self, ctx: &egui::Context, slot: usize, at: Pos2, global_hint: Pos2) {
        let name = self.emote_slot_names[slot.min(WHEEL_SLOTS - 1)].clone();
        if name.is_empty() {
            self.status = "empty emote slot".into();
            return;
        }
        let catalog_idx = self
            .catalog
            .emotes()
            .iter()
            .position(|e| e.name == name);
        let Some(idx) = catalog_idx else {
            self.status = format!("emote '{name}' not in catalog");
            return;
        };

        let need_decode = self
            .emote_gpu_cache
            .as_ref()
            .map(|(n, _, _)| n != &name)
            .unwrap_or(true);
        if need_decode {
            match self.catalog.load_emote_animation(AssetId::Index(idx)) {
                Ok(anim) => {
                    self.emote_gpu_cache = Some(upload_anim(ctx, anim));
                }
                Err(e) => {
                    self.status = format!("emote decode: {e:#}");
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
        if !self.settings.muted {
            if let Some(entry) = self.catalog.emote(AssetId::Index(idx)) {
                self.audio.play_file(&entry.ogg);
            }
        }
        let (d, fx, fy) =
            display::point_to_frac(&self.displays, global_hint.x as f64, global_hint.y as f64);
        self.room.broadcast_emote(&name, d, fx, fy);
        self.status = format!("emote '{name}' d{d}");
        self.log(self.status.clone());
    }

    fn apply_remote_ping(&mut self, ctx: &egui::Context, ping: &str, d: u32, x: f64, y: f64, name: &str) {
        self.ensure_ping_icons(ctx);
        let slot = PINGS.iter().position(|p| p.id == ping).unwrap_or(0);
        let disp = self
            .displays
            .iter()
            .find(|di| di.number == d)
            .unwrap_or(&self.displays[0]);
        let (lx, ly) = display::frac_to_local(disp, x, y);
        // Map into our single window: treat as primary local coords for now.
        let at = Pos2::new(lx, ly);
        if let Some(tex) = self.ping_icons[slot].clone() {
            self.fx.push_ping(at, tex);
        }
        self.log(format!("remote ping from {name}: {ping}"));
    }

    fn trigger_matches(&self, ping_mod: bool, emote_mod: bool) -> (bool, bool) {
        let ping = match self.settings.trigger {
            TriggerKey::Alt => ping_mod,
            TriggerKey::Ctrl => emote_mod, // if user swapped — still use alt/ctrl hardware
            _ => ping_mod,
        };
        let emote = match self.settings.emote_trigger {
            TriggerKey::Ctrl => emote_mod,
            TriggerKey::Alt => ping_mod,
            _ => emote_mod,
        };
        // Standard: alt→ping, ctrl→emote regardless of enum rename for now.
        let _ = (ping, emote);
        (ping_mod, emote_mod)
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
        if !self.settings.enabled {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ctx, |ui| {
                    ui.centered_and_justified(|ui| {
                        ui.label(strings(self.settings.language).paused);
                    });
                });
            return;
        }

        self.input.poll();
        for ev in self.room.poll() {
            match ev {
                RoomEvent::RemotePing {
                    name, ping, d, x, y, ..
                } => self.apply_remote_ping(ctx, &ping, d, x, y, &name),
                RoomEvent::RemoteEmote { name, e, .. } => {
                    self.log(format!("remote emote from {name}: {e} (playback TODO map)"));
                }
                RoomEvent::Status(s) => self.log(s),
                RoomEvent::MemberUpsert(m) => self.log(format!("member + {}", m.name)),
                RoomEvent::MemberGone(id) => self.log(format!("member - {id}")),
            }
        }

        let snap = self.input.shared.get();
        let hooks_live = matches!(
            self.input.status,
            HookStatus::Running | HookStatus::Starting
        );
        let (raw_ping, raw_emote) = if hooks_live && (snap.ping_down || snap.emote_down || snap.move_count > 0)
        {
            (snap.ping_down, snap.emote_down)
        } else {
            ctx.input(|i| (i.modifiers.alt, i.modifiers.ctrl))
        };
        let (ping_down, emote_down) = self.trigger_matches(raw_ping, raw_emote);
        let use_global = hooks_live && snap.move_count > 0;
        let cursor = Self::cursor_local(ctx, snap.mouse_x, snap.mouse_y, use_global);
        let cancel = ctx.input(|i| i.key_pressed(egui::Key::Escape));

        let outcome = self.gesture.tick(ping_down, emote_down, cursor, cancel);
        self.handle_outcome(ctx, outcome, cursor);

        if self.gesture.wheel_active() {
            match self.gesture.wheel.as_ref().map(|w| w.kind) {
                Some(GestureKind::Ping) => self.ensure_ping_icons(ctx),
                Some(GestureKind::Emote) => self.ensure_emote_icons(ctx),
                None => {}
            }
        }

        if self.show_settings {
            self.overlay.set_debug_pinned(true);
        }
        self.overlay.sync(
            ping_down,
            emote_down,
            self.gesture.overlay_hint(),
            false,
        );
        self.overlay.apply(ctx);

        if self.gesture.wheel_active() || self.fx.active() {
            ctx.request_repaint();
        } else {
            ctx.request_repaint_after(std::time::Duration::from_millis(33));
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                self.fx.tick_and_paint(ui);
                if let Some(w) = self.gesture.wheel.clone() {
                    let icons = match w.kind {
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
                                GestureKind::Ping => "PING",
                                GestureKind::Emote => "EMOTE",
                            },
                        },
                    );
                }
            });

        let lang = self.settings.language;
        let t = strings(lang);

        if self.overlay.debug_pinned() || self.show_settings {
            egui::Window::new(t.app_title)
                .default_pos(egui::pos2(16.0, 16.0))
                .default_width(360.0)
                .show(ctx, |ui| {
                    ui.label(format!("{} · {:?}", self.overlay.state().label(), self.input.status));
                    ui.label(&self.status);
                    let mut pin = self.overlay.debug_pinned();
                    if ui.checkbox(&mut pin, t.debug_pin).changed() {
                        self.overlay.set_debug_pinned(pin);
                    }
                    if ui.button(t.settings).clicked() {
                        self.show_settings = !self.show_settings;
                    }
                    if self.show_settings {
                        ui.separator();
                        self.draw_settings(ui, &t);
                    }
                    ui.separator();
                    ui.heading(t.room);
                    if let Some(code) = self.room.code_display() {
                        ui.monospace(&code);
                        if ui.button(t.leave_room).clicked() {
                            self.room.leave();
                            self.log("left room");
                        }
                    } else {
                        if ui.button(t.create_room).clicked() {
                            match self.room.create() {
                                Ok(c) => {
                                    self.room_code_input = c.clone();
                                    self.log(format!("created {c}"));
                                }
                                Err(e) => self.log(format!("create failed: {e:#}")),
                            }
                        }
                        ui.horizontal(|ui| {
                            ui.text_edit_singleline(&mut self.room_code_input);
                            if ui.button(t.join_room).clicked() {
                                match self.room.join(&self.room_code_input) {
                                    Ok(c) => self.log(format!("joined {c}")),
                                    Err(e) => self.log(format!("join failed: {e:#}")),
                                }
                            }
                        });
                    }
                    for m in self.room.members_snapshot() {
                        ui.label(format!(
                            "{}{} [{}]",
                            m.name,
                            if m.self_ { " (you)" } else { "" },
                            format!("{:?}", m.status).to_lowercase()
                        ));
                    }
                    ui.collapsing("log", |ui| {
                        for line in self.event_log.iter().take(12) {
                            ui.monospace(line);
                        }
                    });
                });
        }
    }
}

impl LolPingApp {
    fn draw_settings(&mut self, ui: &mut egui::Ui, t: &i18n::Strings) {
        egui::ComboBox::from_label(t.trigger)
            .selected_text(format!("{:?}", self.settings.trigger))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.settings.trigger, TriggerKey::Alt, "Alt / Option");
                ui.selectable_value(&mut self.settings.trigger, TriggerKey::Ctrl, "Ctrl");
                ui.selectable_value(&mut self.settings.trigger, TriggerKey::Shift, "Shift");
            });
        egui::ComboBox::from_label(t.emote_trigger)
            .selected_text(format!("{:?}", self.settings.emote_trigger))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.settings.emote_trigger, TriggerKey::Ctrl, "Ctrl");
                ui.selectable_value(&mut self.settings.emote_trigger, TriggerKey::Alt, "Alt / Option");
            });
        ui.add(egui::Slider::new(&mut self.settings.ping_size_px, 60..=220).text(t.ping_size));
        ui.add(egui::Slider::new(&mut self.settings.emote_size_px, 60..=260).text(t.emote_size));
        ui.add(egui::Slider::new(&mut self.settings.volume, 0..=100).text(t.volume));
        ui.checkbox(&mut self.settings.muted, t.muted);
        egui::ComboBox::from_label(t.language)
            .selected_text(format!("{:?}", self.settings.language))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.settings.language, i18n::Lang::En, "English");
                ui.selectable_value(&mut self.settings.language, i18n::Lang::Zh, "中文");
            });
        ui.horizontal(|ui| {
            ui.label("Display name");
            ui.text_edit_singleline(&mut self.settings.display_name);
        });
        ui.checkbox(&mut self.settings.allow_internet, "Allow internet rooms (Nostr stub)");
        ui.checkbox(&mut self.settings.launch_at_login, t.launch_at_login);
        ui.checkbox(&mut self.settings.enabled, "Enabled");
        ui.weak("Auto-update: deferred (settings.auto_update_check kept off)");
        if ui.button("Save settings").clicked() {
            apply_launch_at_login(self.settings.launch_at_login);
            match self.settings.save() {
                Ok(()) => self.log(format!("saved {}", Settings::config_path().display())),
                Err(e) => self.log(format!("save failed: {e:#}")),
            }
        }
    }
}

fn apply_launch_at_login(enabled: bool) {
    #[cfg(target_os = "macos")]
    {
        // Minimal LaunchAgent write; full SMAppService later.
        let home = match std::env::var("HOME") {
            Ok(h) => h,
            Err(_) => return,
        };
        let path = std::path::PathBuf::from(home)
            .join("Library/LaunchAgents/com.icy-cat.lolping-lite.plist");
        if enabled {
            if let Ok(exe) = std::env::current_exe() {
                let plist = format!(
                    r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>Label</key><string>com.icy-cat.lolping-lite</string>
  <key>ProgramArguments</key><array><string>{}</string></array>
  <key>RunAtLoad</key><true/>
</dict></plist>"#,
                    exe.display()
                );
                let _ = std::fs::create_dir_all(path.parent().unwrap());
                let _ = std::fs::write(&path, plist);
                log::info!("wrote LaunchAgent {}", path.display());
            }
        } else {
            let _ = std::fs::remove_file(&path);
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = enabled;
        log::info!("launch-at-login: only implemented on macOS");
    }
}
