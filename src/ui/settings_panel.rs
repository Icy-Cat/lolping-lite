//! Fluent-inspired settings viewport (layout mirrors upstream App.tsx).

use super::theme::*;
use super::widgets::*;
use crate::assets::AssetCatalog;
use crate::audio::AudioOut;
use crate::display::DisplayInfo;
use crate::i18n::{self, strings, Strings};
use crate::input::HookStatus;
use crate::remote::BulkState;
use crate::room::RoomHub;
use crate::settings::{Settings, TriggerKey};
use crate::wheel::{PINGS, WHEEL_SLOTS};
use eframe::egui::{
    self, Color32, Pos2, RichText, Sense, Shape, Stroke, TextureHandle, Vec2, ViewportBuilder,
    ViewportClass, ViewportId,
};
use std::f32::consts::TAU;
use std::sync::atomic::Ordering;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavSection {
    Trigger,
    Toggle,
    Pings,
    Wheel,
    Emotes,
    Room,
    App,
}

impl NavSection {
    pub const ALL: [NavSection; 7] = [
        Self::Trigger,
        Self::Toggle,
        Self::Pings,
        Self::Wheel,
        Self::Emotes,
        Self::Room,
        Self::App,
    ];

    fn label(self, t: &Strings) -> &'static str {
        match self {
            Self::Trigger => t.nav_trigger,
            Self::Toggle => t.nav_toggle,
            Self::Pings => t.nav_pings,
            Self::Wheel => t.nav_wheel,
            Self::Emotes => t.nav_emotes,
            Self::Room => t.nav_room,
            Self::App => t.nav_app,
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Self::Trigger => "⌖",
            Self::Toggle => "⌨",
            Self::Pings => "🔊",
            Self::Wheel => "◐",
            Self::Emotes => "☺",
            Self::Room => "👥",
            Self::App => "⚙",
        }
    }
}

#[derive(Debug, Default)]
pub struct SettingsUiState {
    pub nav: NavSection,
    pub ping_pick: Option<String>,
    pub emote_pick: Option<String>,
    pub room_error: Option<String>,
    pub copied_at: Option<std::time::Instant>,
    pub theme_applied: bool,
}

impl Default for NavSection {
    fn default() -> Self {
        Self::Trigger
    }
}

pub struct SettingsHost<'a> {
    pub settings: &'a mut Settings,
    pub ui_state: &'a mut SettingsUiState,
    pub input_status: &'a HookStatus,
    pub needs_accessibility: bool,
    pub hooks_live: bool,
    pub catalog: &'a AssetCatalog,
    pub ping_icons: &'a [Option<TextureHandle>; WHEEL_SLOTS],
    pub emote_icons: &'a [Option<TextureHandle>; WHEEL_SLOTS],
    pub emote_names: &'a [String],
    pub room: &'a mut RoomHub,
    pub room_code_input: &'a mut String,
    pub displays: &'a [DisplayInfo],
    pub audio: &'a mut AudioOut,
    pub event_log: &'a [String],
    pub status_line: &'a str,
    pub open_accessibility: &'a mut dyn FnMut(),
    pub apply_launch_at_login: &'a mut dyn FnMut(bool),
    pub on_settings_changed: &'a mut dyn FnMut(&Settings),
    pub close_requested: &'a mut bool,
}

pub fn paint_settings_viewport(ctx: &egui::Context, open: &mut bool, mut host: SettingsHost<'_>) {
    if !*open {
        return;
    }
    if !host.ui_state.theme_applied {
        apply_fluent_dark(ctx);
        host.ui_state.theme_applied = true;
    }

    let t = strings(host.settings.language);
    let builder = ViewportBuilder::default()
        .with_title(format!("{} — {}", t.app_title, t.page_title))
        .with_inner_size([960.0, 700.0])
        .with_min_inner_size([720.0, 480.0])
        .with_decorations(true)
        .with_transparent(false)
        .with_resizable(true)
        .with_taskbar(true);

    let id = ViewportId::from_hash_of("lolping-settings");
    let mut closed = false;
    ctx.show_viewport_immediate(id, builder, |ctx, class| {
        let _ = class;
        if ctx.input(|i| i.viewport().close_requested()) {
            closed = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(BG).inner_margin(0.0))
            .show(ctx, |ui| {
                paint_settings_body(ui, &mut host, &t);
            });
        ctx.request_repaint_after(std::time::Duration::from_millis(200));
    });
    if closed {
        *open = false;
        *host.close_requested = true;
    }
}

fn paint_settings_body(ui: &mut egui::Ui, host: &mut SettingsHost<'_>, t: &Strings) {
    let mut dirty = false;
    ui.horizontal(|ui| {
        // —— side nav ——
        egui::Frame::NONE
            .fill(BG_SIDE)
            .inner_margin(egui::Margin::symmetric(8, 8))
            .show(ui, |ui| {
                ui.set_width(220.0);
                ui.set_min_height(ui.available_height());
                ui.horizontal(|ui| {
                    ui.label(RichText::new("◉").size(28.0).color(BRAND));
                    ui.vertical(|ui| {
                        ui.label(RichText::new(t.app_title).strong().size(16.0));
                        ui.label(RichText::new(t.tagline).size(11.0).color(TEXT3));
                    });
                });
                ui.add_space(12.0);
                for sec in NavSection::ALL {
                    if nav_button(ui, sec.icon(), sec.label(t), host.ui_state.nav == sec) {
                        host.ui_state.nav = sec;
                    }
                }
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    ui.add_space(8.0);
                    if primary_button(ui, t.close).clicked() {
                        *host.close_requested = true;
                    }
                });
            });

        // —— content ——
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                egui::Frame::NONE
                    .inner_margin(egui::Margin {
                        left: 20,
                        right: 28,
                        top: 8,
                        bottom: 28,
                    })
                    .show(ui, |ui| {
                        dirty |= paint_content(ui, host, t);
                    });
            });
    });

    if dirty {
        (host.on_settings_changed)(host.settings);
    }
    if *host.close_requested {
        // parent clears `open`
    }
}

fn paint_content(ui: &mut egui::Ui, host: &mut SettingsHost<'_>, t: &Strings) -> bool {
    let mut dirty = false;
    page_title(ui, t.page_title);

    if host.needs_accessibility {
        permission_card(
            ui,
            t.perm_title,
            &[t.perm_step1, t.perm_step2, t.perm_step3],
            t.perm_open,
            || (host.open_accessibility)(),
        );
    }

    let failed = matches!(host.input_status, HookStatus::Failed(_));
    let no_access = host.needs_accessibility;
    let on = host.settings.enabled && host.hooks_live && !no_access && !failed;
    let title = if failed {
        t.status_failed
    } else if no_access {
        t.status_no_access
    } else if host.settings.enabled {
        t.status_on
    } else {
        t.status_off
    };
    let trigger = trigger_name(host.settings.trigger, t);
    let hint = if failed {
        t.status_failed_hint.to_string()
    } else if no_access {
        t.status_no_access_hint.to_string()
    } else {
        format!(
            "{} {} · {} {} {}",
            t.status_hint_hold,
            trigger,
            host.settings.pause_hotkey_label,
            t.status_hint_toggle,
            if host.settings.enabled { t.off } else { t.on }
        )
    };
    if status_card(
        ui,
        on,
        failed,
        no_access,
        title,
        &hint,
        t.on,
        t.off,
        &mut host.settings.enabled,
    ) {
        dirty = true;
    }

    // Jump: paint the active section (scroll-spy approximated by exclusive sections).
    // Upstream scrolls one page; we switch content by nav for clarity in egui.
    match host.ui_state.nav {
        NavSection::Trigger => dirty |= section_trigger(ui, host, t),
        NavSection::Toggle => dirty |= section_toggle(ui, host, t),
        NavSection::Pings => dirty |= section_pings(ui, host, t),
        NavSection::Wheel => dirty |= section_wheel(ui, host, t),
        NavSection::Emotes => dirty |= section_emotes(ui, host, t),
        NavSection::Room => dirty |= section_room(ui, host, t),
        NavSection::App => dirty |= section_app(ui, host, t),
    }

    ui.add_space(16.0);
    ui.collapsing(RichText::new(t.developer).color(TEXT3), |ui| {
        ui.label(RichText::new(host.status_line).size(12.0).color(TEXT3));
        ui.label(format!("input: {:?}", host.input_status));
        for line in host.event_log.iter().take(12) {
            ui.monospace(RichText::new(line).size(11.0).color(TEXT3));
        }
    });

    dirty
}

fn trigger_name(k: TriggerKey, _t: &Strings) -> &'static str {
    match k {
        TriggerKey::Alt => {
            if cfg!(target_os = "macos") {
                "⌥ Option"
            } else {
                "Alt"
            }
        }
        TriggerKey::Ctrl => {
            if cfg!(target_os = "macos") {
                "⌃ Control"
            } else {
                "Ctrl"
            }
        }
        TriggerKey::Shift => "Shift",
        TriggerKey::Win => {
            if cfg!(target_os = "macos") {
                "⌘ Command"
            } else {
                "Win"
            }
        }
    }
}

fn trigger_combo(ui: &mut egui::Ui, id: &str, value: &mut TriggerKey, t: &Strings) -> bool {
    let mut changed = false;
    let label = trigger_name(*value, t);
    egui::ComboBox::from_id_salt(id)
        .selected_text(label)
        .width(140.0)
        .show_ui(ui, |ui| {
            for k in [TriggerKey::Alt, TriggerKey::Ctrl, TriggerKey::Shift, TriggerKey::Win] {
                let name = trigger_name(k, t);
                if ui.selectable_value(value, k, name).changed() {
                    changed = true;
                }
            }
        });
    changed
}

fn section_trigger(ui: &mut egui::Ui, host: &mut SettingsHost<'_>, t: &Strings) -> bool {
    let mut dirty = false;
    section_title(ui, t.nav_trigger);
    let mut trig = host.settings.trigger;
    setting_card(ui, "⌖", t.trigger_key, Some(t.trigger_key_desc), false, |ui| {
        if trigger_combo(ui, "ping-trigger", &mut trig, t) {
            dirty = true;
        }
    });
    if dirty {
        host.settings.trigger = trig;
    }
    let unit = t.length_unit;
    dirty |= slider_card_u32(
        ui,
        "↔",
        t.drag_distance,
        Some(t.drag_distance_desc),
        &mut host.settings.drag_threshold_px,
        2..=40,
        |v| format!("{v} {unit}"),
        false,
    );
    let tr = trigger_name(host.settings.trigger, t);
    let click_title = format!("{tr} {}", t.click_ping);
    dirty |= switch_card(
        ui,
        "🖱",
        &click_title,
        Some(t.click_ping_desc),
        &mut host.settings.click_ping,
        t.on,
        t.off,
        false,
    );
    if crate::input::fullscreen::PROBE_AVAILABLE {
        dirty |= switch_card(
            ui,
            "🎮",
            t.game_check,
            Some(t.game_check_desc),
            &mut host.settings.game_check,
            t.on,
            t.off,
            false,
        );
    }
    dirty
}

fn section_toggle(ui: &mut egui::Ui, host: &mut SettingsHost<'_>, t: &Strings) -> bool {
    let mut dirty = false;
    section_title(ui, t.nav_toggle);
    setting_card(
        ui,
        "⌨",
        t.toggle_shortcut,
        Some(t.toggle_shortcut_desc),
        false,
        |ui| {
            ui.label(
                RichText::new(&host.settings.pause_hotkey_label)
                    .strong()
                    .size(13.0)
                    .color(Color32::WHITE),
            );
        },
    );
    dirty |= switch_card(
        ui,
        "⏻",
        t.start_enabled,
        Some(t.start_enabled_desc),
        &mut host.settings.enabled_on_start,
        t.on,
        t.off,
        false,
    );
    dirty
}

fn section_pings(ui: &mut egui::Ui, host: &mut SettingsHost<'_>, t: &Strings) -> bool {
    let mut dirty = false;
    section_title(ui, t.nav_pings);
    let unit = t.length_unit;
    dirty |= slider_card_u32(
        ui,
        "⛶",
        t.ping_size,
        None,
        &mut host.settings.ping_size_px,
        60..=220,
        |v| format!("{v} {unit}"),
        false,
    );
    dirty |= slider_card_f32(
        ui,
        "⏱",
        t.ping_duration,
        Some(t.ping_duration_desc),
        &mut host.settings.ping_duration_s,
        0.5..=8.0,
        |v| format!("{v:.1} s"),
        false,
    );
    let mut vol = host.settings.volume as u32;
    let muted = host.settings.muted;
    if slider_card_u32(
        ui,
        "🔊",
        t.volume,
        None,
        &mut vol,
        0..=100,
        |v| format!("{v}"),
        muted,
    ) {
        host.settings.volume = vol as u8;
        host.audio.set_volume(host.settings.volume);
        dirty = true;
    }
    if switch_card(
        ui,
        "🔇",
        t.mute,
        None,
        &mut host.settings.muted,
        t.on,
        t.off,
        false,
    ) {
        host.audio.set_muted(host.settings.muted);
        dirty = true;
    }
    dirty |= switch_card(
        ui,
        "♪",
        t.tick_sound,
        Some(t.tick_sound_desc),
        &mut host.settings.tick_sound,
        t.on,
        t.off,
        false,
    );
    dirty
}

fn section_wheel(ui: &mut egui::Ui, host: &mut SettingsHost<'_>, t: &Strings) -> bool {
    let mut dirty = false;
    section_title(ui, t.nav_wheel);
    egui::Frame::NONE
        .fill(CARD)
        .stroke(Stroke::new(1.0, STROKE))
        .corner_radius(4.0)
        .inner_margin(egui::Margin::same(16))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new(t.wheel_title).strong().size(14.0));
                    ui.label(RichText::new(t.wheel_desc).size(12.0).color(TEXT3));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                    if ui.button(t.wheel_reset).clicked() {
                        host.settings.wheel = PINGS.iter().map(|p| p.id.to_string()).collect();
                        host.settings.click_ping_id = "omw".into();
                        host.ui_state.ping_pick = None;
                        dirty = true;
                    }
                });
            });
            if let Some(id) = &host.ui_state.ping_pick {
                ui.label(
                    RichText::new(format!("{} ({id})", t.wheel_pick_hint))
                        .size(12.0)
                        .color(BRAND),
                );
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                dirty |= paint_ping_wheel_editor(ui, host, t);
                ui.add_space(16.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new(t.all_pings).strong().size(12.0));
                    ui.label(RichText::new(t.all_pings_desc).size(11.0).color(TEXT3));
                    ui.add_space(6.0);
                    egui::ScrollArea::vertical()
                        .max_height(320.0)
                        .show(ui, |ui| {
                            ui.horizontal_wrapped(|ui| {
                                for (i, p) in PINGS.iter().enumerate() {
                                    let selected = host.ui_state.ping_pick.as_deref() == Some(p.id);
                                    let fill = if selected { BRAND_SOFT } else { Color32::from_rgb(40, 40, 40) };
                                    let stroke = if selected {
                                        Stroke::new(2.0, BRAND)
                                    } else {
                                        Stroke::new(1.0, STROKE)
                                    };
                                    egui::Frame::NONE
                                        .fill(fill)
                                        .stroke(stroke)
                                        .corner_radius(6.0)
                                        .inner_margin(egui::Margin::symmetric(6, 6))
                                        .show(ui, |ui| {
                                            ui.set_min_size(Vec2::new(72.0, 64.0));
                                            ui.vertical_centered(|ui| {
                                                if let Some(tex) = host.ping_icons.get(i).and_then(|t| t.as_ref()) {
                                                    ui.add(egui::Image::from_texture(tex).fit_to_exact_size(Vec2::splat(34.0)));
                                                } else {
                                                    ui.label("·");
                                                }
                                                ui.label(RichText::new(p.label).size(10.0).color(TEXT2));
                                            });
                                        })
                                        .response
                                        .interact(Sense::click())
                                        .clicked()
                                        .then(|| {
                                            host.ui_state.ping_pick = Some(p.id.to_string());
                                        });
                                }
                            });
                        });
                });
            });
        });
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        host.ui_state.ping_pick = None;
    }
    dirty
}

fn paint_ping_wheel_editor(ui: &mut egui::Ui, host: &mut SettingsHost<'_>, t: &Strings) -> bool {
    let mut dirty = false;
    let size = 280.0;
    let (rect, _resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    let c = rect.center();
    let r_out = size * 0.48;
    let r_in = size * 0.17;
    let r_icon = size * 0.33;
    let painter = ui.painter_at(rect);

    for i in 0..WHEEL_SLOTS {
        let a0 = (i as f32 - 0.5) * TAU / 8.0 - TAU / 4.0;
        let a1 = (i as f32 + 0.5) * TAU / 8.0 - TAU / 4.0;
        let mut points = Vec::new();
        let steps = 8;
        for s in 0..=steps {
            let a = a0 + (a1 - a0) * (s as f32 / steps as f32);
            points.push(Pos2::new(c.x + r_in * a.cos(), c.y + r_in * a.sin()));
        }
        for s in (0..=steps).rev() {
            let a = a0 + (a1 - a0) * (s as f32 / steps as f32);
            points.push(Pos2::new(c.x + r_out * a.cos(), c.y + r_out * a.sin()));
        }
        painter.add(Shape::convex_polygon(points, WHEEL_FILL, Stroke::new(1.0, WHEEL_STROKE)));

        let amid = (a0 + a1) * 0.5;
        let ip = Pos2::new(c.x + r_icon * amid.cos(), c.y + r_icon * amid.sin());
        let slot_rect = egui::Rect::from_center_size(ip, Vec2::splat(48.0));
        let id = host.settings.wheel.get(i).cloned().unwrap_or_default();
        let ping_idx = PINGS.iter().position(|p| p.id == id).unwrap_or(0);
        if let Some(tex) = host.ping_icons.get(ping_idx).and_then(|t| t.as_ref()) {
            painter.image(
                tex.id(),
                egui::Rect::from_center_size(ip, Vec2::splat(40.0)),
                egui::Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        painter.text(
            Pos2::new(ip.x, ip.y + 28.0),
            egui::Align2::CENTER_CENTER,
            t.slot_names[i],
            egui::FontId::proportional(9.0),
            TEXT3,
        );
        let click = ui.interact(slot_rect, ui.id().with(("pw", i)), Sense::click());
        if click.clicked() {
            if let Some(pick) = host.ui_state.ping_pick.take() {
                while host.settings.wheel.len() <= i {
                    host.settings.wheel.push(PINGS[0].id.to_string());
                }
                host.settings.wheel[i] = pick;
                dirty = true;
            }
        }
    }

    // centre
    painter.circle_filled(c, r_in - 2.0, Color32::from_rgb(20, 26, 38));
    painter.circle_stroke(c, r_in - 2.0, Stroke::new(1.0, WHEEL_STROKE));
    painter.text(c, egui::Align2::CENTER_CENTER, t.wheel_center, egui::FontId::proportional(10.0), BRAND);
    dirty
}

fn section_emotes(ui: &mut egui::Ui, host: &mut SettingsHost<'_>, t: &Strings) -> bool {
    let mut dirty = false;
    section_title(ui, t.nav_emotes);
    let mut et = host.settings.emote_trigger;
    setting_card(ui, "☺", t.emote_key, Some(t.emote_key_desc), false, |ui| {
        if trigger_combo(ui, "emote-trigger", &mut et, t) {
            dirty = true;
        }
    });
    if dirty {
        if et != host.settings.trigger {
            host.settings.emote_trigger = et;
        }
    }
    let unit = t.length_unit;
    dirty |= slider_card_u32(
        ui,
        "⛶",
        t.emote_size,
        None,
        &mut host.settings.emote_size_px,
        60..=260,
        |v| format!("{v} {unit}"),
        false,
    );
    dirty |= switch_card(
        ui,
        "🔊",
        t.emote_sound,
        Some(t.emote_sound_desc),
        &mut host.settings.emote_sound,
        t.on,
        t.off,
        false,
    );

    egui::Frame::NONE
        .fill(CARD)
        .stroke(Stroke::new(1.0, STROKE))
        .corner_radius(4.0)
        .inner_margin(egui::Margin::same(16))
        .show(ui, |ui| {
            ui.label(RichText::new(t.emote_wheel_title).strong().size(14.0));
            ui.label(RichText::new(t.emote_wheel_desc).size(12.0).color(TEXT3));
            if let Some(n) = &host.ui_state.emote_pick {
                ui.label(RichText::new(format!("{} ({n})", t.wheel_pick_hint)).size(12.0).color(BRAND));
            }
            ui.add_space(8.0);
            // slot grid (simpler than full pie for many custom names)
            ui.label(RichText::new(t.emote_wheel_title).size(12.0).color(TEXT2));
            for i in 0..WHEEL_SLOTS {
                while host.settings.emote_wheel.len() <= i {
                    host.settings.emote_wheel.push(String::new());
                }
                ui.horizontal(|ui| {
                    ui.label(RichText::new(t.slot_names[i]).size(12.0).color(TEXT3).monospace());
                    if let Some(tex) = host.emote_icons.get(i).and_then(|t| t.as_ref()) {
                        ui.add(egui::Image::from_texture(tex).fit_to_exact_size(Vec2::splat(28.0)));
                    }
                    let cur = host.settings.emote_wheel[i].clone();
                    let label = if cur.is_empty() { "(empty)".into() } else { cur.clone() };
                    egui::ComboBox::from_id_salt(format!("emote-slot-{i}"))
                        .selected_text(label)
                        .width(180.0)
                        .show_ui(ui, |ui| {
                            if ui.selectable_label(cur.is_empty(), "(empty)").clicked() {
                                host.settings.emote_wheel[i].clear();
                                dirty = true;
                            }
                            for n in host.emote_names {
                                if ui.selectable_label(cur == *n, n).clicked() {
                                    host.settings.emote_wheel[i] = n.clone();
                                    dirty = true;
                                }
                            }
                        });
                    if host.ui_state.emote_pick.is_some() {
                        if ui.small_button("←").clicked() {
                            if let Some(n) = host.ui_state.emote_pick.take() {
                                host.settings.emote_wheel[i] = n;
                                dirty = true;
                            }
                        }
                    }
                });
            }
            ui.add_space(8.0);
            ui.label(RichText::new(t.all_emotes).strong().size(12.0));
            ui.label(RichText::new(t.all_emotes_desc).size(11.0).color(TEXT3));
            egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for n in host.emote_names {
                        let sel = host.ui_state.emote_pick.as_deref() == Some(n.as_str());
                        let text = if sel { format!("[{n}]") } else { n.clone() };
                        if ui.selectable_label(sel, RichText::new(text).size(11.0)).clicked() {
                            host.ui_state.emote_pick = Some(n.clone());
                        }
                    }
                });
            });
        });
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        host.ui_state.emote_pick = None;
    }
    dirty
}

fn section_room(ui: &mut egui::Ui, host: &mut SettingsHost<'_>, t: &Strings) -> bool {
    let mut dirty = false;
    section_title(ui, t.nav_room);
    setting_card(ui, "👤", t.room_name, Some(t.room_name_desc), false, |ui| {
        if ui
            .add(egui::TextEdit::singleline(&mut host.settings.display_name).desired_width(160.0))
            .changed()
        {
            dirty = true;
        }
    });
    setting_card(ui, "🎨", t.room_color, None, false, |ui| {
        for (i, c) in TAG_COLORS.iter().enumerate() {
            let sel = host.settings.tag_color as usize == i;
            let (rect, resp) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::click());
            ui.painter().circle_filled(rect.center(), 9.0, *c);
            if sel {
                ui.painter()
                    .circle_stroke(rect.center(), 11.0, Stroke::new(2.0, TEXT));
            }
            if resp.clicked() {
                host.settings.tag_color = i as u8;
                dirty = true;
            }
        }
    });

    if let Some(code) = host.room.code_display() {
        egui::Frame::NONE
            .fill(CARD)
            .stroke(Stroke::new(1.0, STROKE))
            .corner_radius(4.0)
            .inner_margin(egui::Margin::symmetric(16, 12))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new(&code).strong().size(20.0).monospace());
                        ui.label(RichText::new(t.room_share_desc).size(12.0).color(TEXT3));
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(t.room_leave).clicked() {
                            host.room.leave();
                        }
                        let copied = host
                            .ui_state
                            .copied_at
                            .map(|t0| t0.elapsed().as_millis() < 1500)
                            .unwrap_or(false);
                        if ui.button(if copied { t.room_copied } else { t.room_copy }).clicked() {
                            ui.ctx().copy_text(code.clone());
                            host.ui_state.copied_at = Some(std::time::Instant::now());
                        }
                    });
                });
            });
        ui.label(RichText::new(t.room_waiting).size(12.0).color(TEXT3));
    } else {
        egui::Frame::NONE
            .fill(CARD)
            .stroke(Stroke::new(1.0, STROKE))
            .corner_radius(4.0)
            .inner_margin(egui::Margin::symmetric(16, 12))
            .show(ui, |ui| {
                ui.label(RichText::new(t.room_join_title).strong());
                ui.label(RichText::new(t.room_join_desc).size(12.0).color(TEXT3));
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(host.room_code_input)
                            .desired_width(190.0)
                            .hint_text("PING-XXXXX-XXXXX"),
                    );
                    if primary_button(ui, t.room_join).clicked() {
                        match host.room.join(host.room_code_input) {
                            Ok(_) => host.ui_state.room_error = None,
                            Err(e) => host.ui_state.room_error = Some(e.to_string()),
                        }
                    }
                    if ui.button(t.room_create).clicked() {
                        if let Ok(c) = host.room.create() {
                            *host.room_code_input = c;
                            host.ui_state.room_error = None;
                        }
                    }
                });
                if let Some(err) = &host.ui_state.room_error {
                    ui.label(RichText::new(err).size(12.0).color(WARN));
                }
            });
    }
    ui.add_space(4.0);
    dirty |= switch_card(
        ui,
        "🔇",
        t.room_mute,
        Some(t.room_mute_desc),
        &mut host.settings.room_muted,
        t.on,
        t.off,
        false,
    );
    dirty |= switch_card(
        ui,
        "🌐",
        t.room_internet,
        Some(t.room_internet_desc),
        &mut host.settings.allow_internet,
        t.on,
        t.off,
        false,
    );
    setting_card(ui, "🖥", t.room_displays, Some(t.room_displays_desc), false, |ui| {
        ui.vertical(|ui| {
            for d in host.displays {
                let prim = if d.primary {
                    format!(" {}", t.room_primary)
                } else {
                    String::new()
                };
                ui.label(
                    RichText::new(format!("#{}{prim} · {}×{}", d.number, d.width, d.height))
                        .size(12.0)
                        .color(TEXT3),
                );
            }
        });
    });
    ui.label(RichText::new(t.room_privacy).size(11.0).color(TEXT3));
    dirty
}

fn section_app(ui: &mut egui::Ui, host: &mut SettingsHost<'_>, t: &Strings) -> bool {
    let mut dirty = false;
    section_title(ui, t.nav_app);
    let mut launch = host.settings.launch_at_login;
    if switch_card(
        ui,
        "🚀",
        t.launch_at_startup,
        Some(t.launch_at_startup_desc),
        &mut launch,
        t.on,
        t.off,
        false,
    ) {
        host.settings.launch_at_login = launch;
        (host.apply_launch_at_login)(launch);
        dirty = true;
    }
    setting_card(ui, "文", t.language, Some(t.language_desc), false, |ui| {
        let cur = match host.settings.language {
            i18n::Lang::En => "English",
            i18n::Lang::Zh => "简体中文",
        };
        egui::ComboBox::from_id_salt("lang")
            .selected_text(cur)
            .width(140.0)
            .show_ui(ui, |ui| {
                if ui
                    .selectable_value(&mut host.settings.language, i18n::Lang::En, "English")
                    .changed()
                {
                    dirty = true;
                }
                if ui
                    .selectable_value(&mut host.settings.language, i18n::Lang::Zh, "简体中文")
                    .changed()
                {
                    dirty = true;
                }
            });
    });

    // Assets
    section_title(ui, t.assets);
    draw_assets(ui, host, t);

    setting_card(ui, "ℹ", t.about, Some(t.about_desc), false, |ui| {
        if ui.link(t.github_page).clicked() {
            ui.ctx().open_url(egui::OpenUrl::new_tab(
                "https://github.com/Icy-Cat/lolping-lite",
            ));
        }
        if ui.button(t.open_settings_folder).clicked() {
            let path = Settings::config_path();
            if let Some(parent) = path.parent() {
                let _ = open::that(parent);
            }
        }
    });
    ui.label(RichText::new(t.auto_update_stub).size(12.0).color(TEXT3));
    ui.add_space(6.0);
    ui.label(RichText::new(t.known_limitations).strong().size(12.0));
    #[cfg(target_os = "macos")]
    {
        ui.label(RichText::new("• macOS asks for Accessibility again after every update.").size(11.0).color(TEXT3));
        ui.label(RichText::new("• Some full-screen games draw above the overlay.").size(11.0).color(TEXT3));
    }
    #[cfg(not(target_os = "macos"))]
    {
        ui.label(RichText::new("• The wheel can’t be triggered over elevated windows.").size(11.0).color(TEXT3));
        ui.label(RichText::new("• Exclusive-fullscreen games draw above the overlay.").size(11.0).color(TEXT3));
    }
    dirty
}

fn draw_assets(ui: &mut egui::Ui, host: &mut SettingsHost<'_>, t: &Strings) {
    let Some(v1) = host.catalog.store().cloned() else {
        ui.label(RichText::new("No asset store").color(TEXT3));
        return;
    };
    let v2 = host.catalog.v2_store().cloned();
    let store = v2.clone().unwrap_or_else(|| v1.clone());
    ui.label(
        RichText::new(if v2.is_some() {
            "Emotes: v2 WebM (VP9+alpha), v1 WebP fallback"
        } else {
            "Emotes: v1 WebP"
        })
        .size(11.0)
        .color(TEXT3),
    );
    let mb = |b: u64| b as f64 / 1_048_576.0;
    let size = v1.cache_size() + v2.as_ref().map(|v| v.cache_size()).unwrap_or(0);
    ui.label(format!("Cache: {:.1} MB", mb(size)));
    let state = store.bulk_state.lock().unwrap().clone();
    ui.horizontal(|ui| {
        let running = matches!(state, BulkState::Running);
        if ui
            .add_enabled(!running, egui::Button::new(t.download_all))
            .clicked()
        {
            store.download_all_bg();
        }
        if ui
            .add_enabled(!running, egui::Button::new(t.clear_cache))
            .clicked()
        {
            let _ = v1.clear_cache();
            if let Some(v) = &v2 {
                let _ = v.clear_cache();
            }
        }
    });
    match state {
        BulkState::Running => {
            let done = store.bulk_done.load(Ordering::Relaxed);
            let total = store.bulk_total.load(Ordering::Relaxed).max(1);
            ui.add(
                egui::ProgressBar::new((done as f32 / total as f32).min(1.0))
                    .text(format!("{:.1} / {:.1} MB", mb(done), mb(total))),
            );
        }
        BulkState::Done(n) => {
            ui.label(RichText::new(format!("OK · {n} files")).size(11.0).color(OK));
        }
        BulkState::Failed(e) => {
            ui.label(RichText::new(format!("Failed: {e}")).size(11.0).color(WARN));
        }
        BulkState::Idle => {}
    }
}

// soft dependency: open crate may not exist — use std::process
mod open {
    pub fn that(path: &std::path::Path) -> std::io::Result<()> {
        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("open").arg(path).status()?;
        }
        #[cfg(target_os = "windows")]
        {
            std::process::Command::new("explorer").arg(path).status()?;
        }
        #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
        {
            std::process::Command::new("xdg-open").arg(path).status()?;
        }
        Ok(())
    }
}
