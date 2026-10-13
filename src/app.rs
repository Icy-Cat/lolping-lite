//! GUI: per-display overlays, settings-driven hooks, rooms, remote FX.

use crate::assets::{AssetCatalog, AssetId, EmoteAnimation};
use crate::audio::AudioOut;
use crate::display::{self, DisplayInfo};
use crate::fx::{self, EmoteAnim, FxLayer};
use crate::gesture::{GestureKind, GestureOutcome, GestureSession};
use crate::input::{InputConfig, InputEvent, InputHub};
use crate::overlay::OverlayController;
use crate::room::{RoomEvent, RoomHub};
use crate::settings::Settings;
use crate::ui::SettingsUiState;
use crate::wheel::{paint_wheel, hover_slot, WheelPaint, PINGS, WHEEL_SLOTS};
use eframe::egui::{self, Pos2, TextureHandle, ViewportBuilder, ViewportClass, ViewportId};
use std::collections::VecDeque;

struct PlacedOnDisplay {
    display: u32,
    fx: FxLayer,
}

pub struct LolPingApp {
    catalog: AssetCatalog,
    input: InputHub,
    overlay: OverlayController,
    gesture: GestureSession,
    /// FX layers keyed by display number.
    layers: Vec<PlacedOnDisplay>,
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
    /// Which display the active wheel is on.
    wheel_display: u32,
    asset_gen: u64,
    settings_ui: SettingsUiState,
}

impl LolPingApp {
    pub fn new(_cc: &eframe::CreationContext<'_>, open_settings: bool) -> Self {
        let mut settings = Settings::load();
        let catalog = AssetCatalog::discover();
        let names: Vec<String> = catalog.emotes().iter().map(|e| e.name.clone()).collect();
        settings.ensure_emote_wheel(&names);
        settings.ensure_ping_wheel();
        if !settings.enabled_on_start {
            settings.enabled = false;
        }
        let mut emote_slot_names = std::array::from_fn(|_| String::new());
        for (i, n) in settings.emote_wheel.iter().take(WHEEL_SLOTS).enumerate() {
            emote_slot_names[i] = n.clone();
        }
        let displays = display::list_displays();
        let layers = displays
            .iter()
            .map(|d| PlacedOnDisplay {
                display: d.number,
                fx: FxLayer::default(),
            })
            .collect();
        let cfg = InputConfig::from_settings(&settings);
        let mut audio = AudioOut::new();
        audio.set_volume(settings.volume);
        audio.set_muted(settings.muted);
        let status = format!(
            "{} emotes · {} displays · audio ready",
            catalog.emote_count(),
            displays.len()
        );
        let overlay = OverlayController::default();
        let app = Self {
            catalog,
            input: InputHub::start(cfg),
            overlay,
            gesture: GestureSession::default(),
            layers,
            audio,
            settings,
            show_settings: open_settings,
            room: RoomHub::new(&Settings::load()),
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
            wheel_display: 1,
            asset_gen: 0,
            settings_ui: SettingsUiState::default(),
        };
        app.prefetch_wheels();
        app
    }

    fn log(&mut self, line: impl Into<String>) {
        let line = line.into();
        log::info!("{line}");
        self.event_log.push_front(line);
        while self.event_log.len() > 40 {
            self.event_log.pop_back();
        }
    }

    fn layer_mut(&mut self, display: u32) -> &mut FxLayer {
        if let Some(i) = self.layers.iter().position(|l| l.display == display) {
            return &mut self.layers[i].fx;
        }
        self.layers.push(PlacedOnDisplay {
            display,
            fx: FxLayer::default(),
        });
        &mut self.layers.last_mut().unwrap().fx
    }

    fn ensure_ping_icons(&mut self, ctx: &egui::Context) {
        if self.ping_icons_ready {
            return;
        }
        let mut missing = false;
        for (i, def) in PINGS.iter().enumerate() {
            if self.ping_icons[i].is_some() {
                continue;
            }
            let Some(path) = self.catalog.texture_path(def.texture_file) else {
                missing = true;
                continue;
            };
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
        self.ping_icons_ready = !missing;
    }

    fn ensure_emote_icons(&mut self, ctx: &egui::Context) {
        if self.emote_icons_ready {
            return;
        }
        let mut missing = false;
        for i in 0..WHEEL_SLOTS {
            let name = &self.emote_slot_names[i];
            if name.is_empty() {
                continue;
            }
            if let Some((idx, _)) = self
                .catalog
                .emotes()
                .iter()
                .enumerate()
                .find(|(_, e)| e.name == *name)
            {
                if self.emote_icons[i].is_some() {
                    continue;
                }
                let Ok(rgba) = self.catalog.load_emote_png_thumb(AssetId::Index(idx)) else {
                    missing = true;
                    continue;
                };
                {
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
        self.emote_icons_ready = !missing;
    }

    /// Background prefetch of everything the current wheels need.
    fn prefetch_wheels(&self) {
        let mut rels: Vec<String> = Vec::new();
        let names: Vec<String> = self.emote_slot_names.iter().filter(|n| !n.is_empty()).cloned().collect();
        self.catalog.prefetch_animations(&names);
        for n in &names {
            for ext in ["png", "ogg"] {
                rels.push(format!("emotes/{n}.{ext}"));
            }
        }
        for p in PINGS.iter() {
            rels.push(format!("textures/{}", p.texture_file));
            if let Some(snd) = p.sound_file {
                rels.push(format!("sounds/{snd}"));
            }
        }
        self.catalog.prefetch(&rels);
    }

    /// Pick up newly downloaded assets / a freshly fetched manifest.
    fn poll_assets(&mut self) {
        let Some(gen) = self.catalog.store().map(|s| s.generation.load(std::sync::atomic::Ordering::Relaxed)) else {
            return;
        };
        if gen == self.asset_gen {
            return;
        }
        self.asset_gen = gen;
        if self.catalog.rescan() {
            let names: Vec<String> = self.catalog.emotes().iter().map(|e| e.name.clone()).collect();
            self.settings.ensure_emote_wheel(&names);
            self.sync_emote_slots_from_settings();
            self.prefetch_wheels();
            self.status = format!(
                "{} emotes · {} displays · audio ready",
                self.catalog.emote_count(),
                self.displays.len()
            );
            self.log(format!("assets ready: {} emotes", self.catalog.emote_count()));
        }
        self.ping_icons_ready = false;
        self.emote_icons_ready = false;
    }

    fn sync_emote_slots_from_settings(&mut self) {
        for i in 0..WHEEL_SLOTS {
            self.emote_slot_names[i] = self
                .settings
                .emote_wheel
                .get(i)
                .cloned()
                .unwrap_or_default();
        }
        self.emote_icons_ready = false;
    }

    fn global_cursor(&self, snap_x: f64, snap_y: f64, use_global: bool, local: Pos2) -> (f64, f64) {
        if use_global {
            (snap_x, snap_y)
        } else {
            // Assume primary display origin for in-window testing.
            let d = self.displays.iter().find(|d| d.primary).unwrap_or(&self.displays[0]);
            (d.x as f64 + local.x as f64, d.y as f64 + local.y as f64)
        }
    }

    fn display_local(displays: &[DisplayInfo], gx: f64, gy: f64) -> (u32, Pos2) {
        let (num, fx, fy) = display::point_to_frac(displays, gx, gy);
        let d = displays
            .iter()
            .find(|d| d.number == num)
            .unwrap_or(&displays[0]);
        let (lx, ly) = display::frac_to_local(d, fx, fy);
        (num, Pos2::new(lx, ly))
    }

    fn handle_outcome(&mut self, ctx: &egui::Context, outcome: GestureOutcome, gx: f64, gy: f64) {
        match outcome {
            GestureOutcome::None => {}
            GestureOutcome::Cancelled => self.status = "cancelled".into(),
            GestureOutcome::Placed { kind, slot, at } => {
                let (d, _) = Self::display_local(&self.displays, gx, gy);
                match kind {
                    GestureKind::Ping => self.place_ping(ctx, slot, at, d, gx, gy),
                    GestureKind::Emote => self.place_emote(ctx, slot, at, d, gx, gy),
                }
            }
        }
    }

    fn place_ping(
        &mut self,
        ctx: &egui::Context,
        slot: usize,
        at: Pos2,
        display_num: u32,
        gx: f64,
        gy: f64,
    ) {
        self.ensure_ping_icons(ctx);
        let def = self.settings.ping_def_at(slot);
        let tex_idx = PINGS.iter().position(|p| p.id == def.id).unwrap_or(slot);
        if let Some(tex) = self.ping_icons[tex_idx].clone() {
            self.layer_mut(display_num).push_ping(at, tex);
        }
        if let Some(s) = def.sound_file {
            if let Some(p) = self.catalog.sound_path(s) {
                self.audio.play_file(&p);
            }
        }
        let (d, fx, fy) = display::point_to_frac(&self.displays, gx, gy);
        self.room.broadcast_ping(def.id, d, fx, fy);
        self.status = format!("ping '{}' on d{d}", def.label);
        self.log(self.status.clone());
    }

    fn place_emote(
        &mut self,
        ctx: &egui::Context,
        slot: usize,
        at: Pos2,
        display_num: u32,
        gx: f64,
        gy: f64,
    ) {
        let name = self.emote_slot_names[slot.min(WHEEL_SLOTS - 1)].clone();
        if name.is_empty() {
            self.status = "empty emote slot".into();
            return;
        }
        if let Some(anim) = self.load_emote_gpu(ctx, &name) {
            self.layer_mut(display_num).push_emote(at, anim);
        }
        if let Some(p) = self
            .catalog
            .emotes()
            .iter()
            .find(|e| e.name == name)
            .and_then(|e| self.catalog.resolve(&e.ogg))
        {
            self.audio.play_file(&p);
        }
        let (d, fx, fy) = display::point_to_frac(&self.displays, gx, gy);
        self.room.broadcast_emote(&name, d, fx, fy);
        self.status = format!("emote '{name}' on d{d}");
        self.log(self.status.clone());
    }

    fn load_emote_gpu(&mut self, ctx: &egui::Context, name: &str) -> Option<EmoteAnim> {
        if self
            .emote_gpu_cache
            .as_ref()
            .map(|(n, _, _)| n == name)
            .unwrap_or(false)
        {
            let (n, frames, delays) = self.emote_gpu_cache.as_ref().unwrap();
            return Some(EmoteAnim {
                name: n.clone(),
                frames: frames.clone(),
                delays_ms: delays.clone(),
            });
        }
        let idx = self.catalog.emotes().iter().position(|e| e.name == name)?;
        let anim = self.catalog.load_emote_animation(AssetId::Index(idx)).ok()?;
        let uploaded = upload_anim(ctx, anim);
        self.emote_gpu_cache = Some(uploaded);
        let (n, frames, delays) = self.emote_gpu_cache.as_ref().unwrap();
        Some(EmoteAnim {
            name: n.clone(),
            frames: frames.clone(),
            delays_ms: delays.clone(),
        })
    }

    fn apply_remote_ping(&mut self, ctx: &egui::Context, ping: &str, d: u32, x: f64, y: f64, from: &str) {
        self.ensure_ping_icons(ctx);
        let slot = PINGS.iter().position(|p| p.id == ping).unwrap_or(0);
        let disp_num = self
            .displays
            .iter()
            .find(|di| di.number == d)
            .map(|di| di.number)
            .unwrap_or(1);
        let disp = self
            .displays
            .iter()
            .find(|di| di.number == disp_num)
            .cloned()
            .unwrap_or_else(|| self.displays[0].clone());
        let (lx, ly) = display::frac_to_local(&disp, x, y);
        if let Some(tex) = self.ping_icons[slot].clone() {
            self.layer_mut(disp_num)
                .push_ping(Pos2::new(lx, ly), tex);
        }
        self.log(format!("remote ping {ping} from {from} @d{disp_num}"));
    }

    fn apply_remote_emote(&mut self, ctx: &egui::Context, e: &str, d: u32, x: f64, y: f64, from: &str) {
        let disp_num = self
            .displays
            .iter()
            .find(|di| di.number == d)
            .map(|di| di.number)
            .unwrap_or(1);
        let disp = self
            .displays
            .iter()
            .find(|di| di.number == disp_num)
            .cloned()
            .unwrap_or_else(|| self.displays[0].clone());
        let (lx, ly) = display::frac_to_local(&disp, x, y);
        if let Some(anim) = self.load_emote_gpu(ctx, e) {
            self.layer_mut(disp_num)
                .push_emote(Pos2::new(lx, ly), anim);
            self.log(format!("remote emote {e} from {from} @d{disp_num}"));
        } else {
            self.log(format!("remote emote {e} unknown from {from}"));
        }
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
    fn clear_color(&self, _: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_assets();
        // Pause hotkey from hooks
        for ev in self.input.poll() {
            if matches!(ev, InputEvent::ForeignKey) {
                if self.gesture.foreign_key(crate::input::decision::KeyClass::Other) {
                    self.log("trigger is part of another chord: wheel suppressed until release");
                }
                continue;
            }
            if matches!(ev, InputEvent::ToggleEnabled) {
                self.settings.enabled = self.input.enabled;
                let _ = self.settings.save();
                self.input.apply_settings(&self.settings);
                self.log(format!("enabled={}", self.settings.enabled));
            }
        }
        // In-window pause: Ctrl+Alt+P
        if ctx.input(|i| {
            i.key_pressed(egui::Key::P) && i.modifiers.ctrl && i.modifiers.alt
        }) {
            self.settings.enabled = !self.settings.enabled;
            self.input.apply_settings(&self.settings);
            let _ = self.settings.save();
            self.log(format!("GUI pause → enabled={}", self.settings.enabled));
        }

        for ev in self.room.poll() {
            match ev {
                RoomEvent::RemotePing {
                    name, ping, d, x, y, ..
                } => self.apply_remote_ping(ctx, &ping, d, x, y, &name),
                RoomEvent::RemoteEmote {
                    name, e, d, x, y, ..
                } => self.apply_remote_emote(ctx, &e, d, x, y, &name),
                RoomEvent::Status(s) => self.log(s),
                RoomEvent::MemberUpsert(m) => self.log(format!("member +{}", m.name)),
                RoomEvent::MemberGone(id) => self.log(format!("member -{id}")),
            }
        }

        // Accessibility missing → open the real settings window (permission card).
        if self.input.needs_accessibility() {
            self.show_settings = true;
        }

        if !self.settings.enabled {
            self.paint_overlays(ctx, true);
            self.paint_settings_viewport(ctx);
            return;
        }

        let snap = self.input.shared.get();
        let hooks_live = self.input.hooks_live();
        let (raw_alt, raw_ctrl) = if hooks_live && snap.move_count > 0 {
            // Already mapped inside hook via settings — ping_down/emote_down are logical.
            (snap.ping_down, snap.emote_down)
        } else {
            let (a, c) = ctx.input(|i| (i.modifiers.alt, i.modifiers.ctrl));
            let cfg = InputConfig::from_settings(&self.settings);
            cfg.map_modifiers(a, c, false, false)
        };
        let ping_down = raw_alt;
        let emote_down = raw_ctrl;

        let use_global = hooks_live && snap.move_count > 0;
        let pointer = ctx
            .pointer_latest_pos()
            .unwrap_or(egui::pos2(200.0, 200.0));
        let (gx, gy) = self.global_cursor(snap.mouse_x, snap.mouse_y, use_global, pointer);
        let (disp_num, local) = Self::display_local(&self.displays, gx, gy);
        let cancel = ctx.input(|i| i.key_pressed(egui::Key::Escape));

        let was_active = self.gesture.wheel_active();
        let game_check = self.settings.game_check && crate::input::fullscreen::PROBE_AVAILABLE;
        let was_paused = self.gesture.game_paused;
        let outcome = self.gesture.tick_gated(ping_down, emote_down, local, cancel, &mut || {
            game_check && crate::input::fullscreen::click_is_on_fullscreen_app(gx, gy)
        });
        if self.gesture.game_paused && !was_paused {
            self.status = "fullscreen app on this monitor: gesture paused".into();
            self.log(self.status.clone());
        }
        if self.gesture.wheel_active() && !was_active {
            self.wheel_display = disp_num;
        }
        self.handle_outcome(ctx, outcome, gx, gy);

        if self.gesture.wheel_active() {
            match self.gesture.wheel.as_ref().map(|w| w.kind) {
                Some(GestureKind::Ping) => self.ensure_ping_icons(ctx),
                Some(GestureKind::Emote) => self.ensure_emote_icons(ctx),
                None => {}
            }
        }

        // Option+Control chord still opens settings (via overlay sync pinning → we map to show_settings).
        let was_debug = self.overlay.debug_pinned();
        self.overlay.sync(ping_down, emote_down, self.gesture.overlay_hint(), false);
        if self.overlay.debug_pinned() && !was_debug {
            self.show_settings = true;
            self.overlay.set_debug_pinned(false);
        }
        // Passthrough applied per overlay viewport below.

        let busy = self.gesture.wheel_active() || self.layers.iter().any(|l| l.fx.active());
        if busy {
            ctx.request_repaint();
        } else {
            ctx.request_repaint_after(std::time::Duration::from_millis(33));
        }

        self.paint_overlays(ctx, self.overlay.state().mouse_passthrough());
        self.paint_settings_viewport(ctx);
    }
}

impl LolPingApp {
    fn paint_overlays(&mut self, ctx: &egui::Context, passthrough: bool) {
        let displays = self.displays.clone();
        let wheel = self.gesture.wheel.clone();
        let wheel_display = self.wheel_display;
        let ping_icons = self.ping_icons.clone();
        let emote_icons = self.emote_icons.clone();

        for d in &displays {
            let id = ViewportId::from_hash_of(("lolping-overlay", d.number));
            let builder = ViewportBuilder::default()
                .with_title(format!("lolPing overlay {}", d.number))
                .with_position(egui::pos2(d.x as f32, d.y as f32))
                .with_inner_size([d.width as f32, d.height as f32])
                .with_decorations(false)
                .with_transparent(true)
                .with_always_on_top()
                .with_mouse_passthrough(passthrough)
                .with_taskbar(false);

            let mut fx_paint = None;
            if let Some(layer) = self.layers.iter_mut().find(|l| l.display == d.number) {
                fx_paint = Some(std::mem::take(&mut layer.fx));
            }

            let show_wheel = wheel
                .as_ref()
                .map(|_| d.number == wheel_display)
                .unwrap_or(false);

            ctx.show_viewport_immediate(id, builder, |ctx, class| {
                if class == ViewportClass::Embedded {
                    return;
                }
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(ctx, |ui| {
                        if let Some(ref mut fx) = fx_paint {
                            fx.tick_and_paint(ui);
                        }
                        if show_wheel {
                            if let Some(w) = &wheel {
                                let icons = match w.kind {
                                    GestureKind::Ping => &ping_icons,
                                    GestureKind::Emote => &emote_icons,
                                };
                                let cursor = ctx
                                    .pointer_latest_pos()
                                    .unwrap_or(w.center);
                                paint_wheel(
                                    ui,
                                    WheelPaint {
                                        center: w.center,
                                        cursor,
                                        hover: w.hover.or_else(|| hover_slot(w.center, cursor)),
                                        icons,
                                        kind_label: match w.kind {
                                            GestureKind::Ping => "PING",
                                            GestureKind::Emote => "EMOTE",
                                        },
                                    },
                                );
                            }
                        }
                    });
                ctx.request_repaint();
            });

            if let Some(fx) = fx_paint {
                if let Some(layer) = self.layers.iter_mut().find(|l| l.display == d.number) {
                    layer.fx = fx;
                }
            }
        }
    }


    fn paint_settings_viewport(&mut self, ctx: &egui::Context) {
        if !self.show_settings {
            return;
        }
        self.ensure_ping_icons(ctx);
        self.ensure_emote_icons(ctx);

        let emote_names: Vec<String> = self.catalog.emotes().iter().map(|e| e.name.clone()).collect();
        let log: Vec<String> = self.event_log.iter().cloned().collect();
        let needs = self.input.needs_accessibility();
        let hooks_live = self.input.hooks_live();
        let status = self.input.status.clone();
        let status_line = self.status.clone();
        let mut close_requested = false;

        let mut open_acc = || open_accessibility_settings();
        let mut apply_launch = |on: bool| apply_launch_at_login(on);
        let dirty_flag = std::cell::Cell::new(false);
        let mut on_changed = |_s: &Settings| {
            dirty_flag.set(true);
        };

        let mut open = self.show_settings;
        {
            let host = crate::ui::SettingsHost {
                settings: &mut self.settings,
                ui_state: &mut self.settings_ui,
                input_status: &status,
                needs_accessibility: needs,
                hooks_live,
                catalog: &self.catalog,
                ping_icons: &self.ping_icons,
                emote_icons: &self.emote_icons,
                emote_names: &emote_names,
                room: &mut self.room,
                room_code_input: &mut self.room_code_input,
                displays: &self.displays,
                audio: &mut self.audio,
                event_log: &log,
                status_line: &status_line,
                open_accessibility: &mut open_acc,
                apply_launch_at_login: &mut apply_launch,
                on_settings_changed: &mut on_changed,
                close_requested: &mut close_requested,
            };
            crate::ui::paint_settings_viewport(ctx, &mut open, host);
        }
        self.show_settings = open && !close_requested;
        let dirty_apply = dirty_flag.get();
        if dirty_apply {
            let _ = self.settings.save();
            self.input.apply_settings(&self.settings);
            self.audio.set_volume(self.settings.volume);
            self.audio.set_muted(self.settings.muted);
            self.sync_emote_slots_from_settings();
        }
    }
}


fn open_accessibility_settings() {
    #[cfg(target_os = "macos")]
    {
        // Works on Ventura+ and older System Preferences URLs.
        let urls = [
            "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
            "x-apple.systempreferences:com.apple.settings.PrivacySecurity.extension",
        ];
        for u in urls {
            if std::process::Command::new("open").arg(u).status().map(|s| s.success()).unwrap_or(false) {
                return;
            }
        }
    }
}

fn apply_launch_at_login(enabled: bool) {
    #[cfg(target_os = "macos")]
    {
        let Ok(home) = std::env::var("HOME") else { return };
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
            }
        } else {
            let _ = std::fs::remove_file(&path);
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = enabled;
    }
}
