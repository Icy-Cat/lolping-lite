//! Fluent-inspired dark theme tokens (upstream settings.css + webDarkTheme).

use eframe::egui::{self, Color32, Stroke, Visuals};

pub const BG: Color32 = Color32::from_rgb(32, 32, 32);
pub const BG_SIDE: Color32 = Color32::from_rgb(28, 28, 28);
pub const CARD: Color32 = Color32::from_rgb(42, 42, 42);
#[allow(dead_code)]
pub const CARD_HOVER: Color32 = Color32::from_rgb(48, 48, 48);
pub const STROKE: Color32 = Color32::from_rgb(55, 55, 55);
pub const TEXT: Color32 = Color32::from_rgb(255, 255, 255);
pub const TEXT2: Color32 = Color32::from_rgb(200, 200, 200);
pub const TEXT3: Color32 = Color32::from_rgb(140, 140, 140);
pub const BRAND: Color32 = Color32::from_rgb(71, 158, 245);
pub const BRAND_SOFT: Color32 = Color32::from_rgb(40, 70, 100);
pub const WARN: Color32 = Color32::from_rgb(255, 156, 72);
pub const OK: Color32 = Color32::from_rgb(108, 203, 95);
pub const NAV_SEL: Color32 = Color32::from_rgb(45, 45, 45);
pub const WHEEL_FILL: Color32 = Color32::from_rgb(14, 18, 28);
pub const WHEEL_STROKE: Color32 = Color32::from_rgb(120, 100, 70);
#[allow(dead_code)]
pub const WHEEL_HOT: Color32 = Color32::from_rgb(50, 90, 130);

pub const TAG_COLORS: [Color32; 8] = [
    Color32::from_rgb(0x4f, 0xc3, 0xf7),
    Color32::from_rgb(0xff, 0x8a, 0x65),
    Color32::from_rgb(0xae, 0xd5, 0x81),
    Color32::from_rgb(0xba, 0x68, 0xc8),
    Color32::from_rgb(0xff, 0xd5, 0x4f),
    Color32::from_rgb(0x4d, 0xb6, 0xac),
    Color32::from_rgb(0xf0, 0x62, 0x92),
    Color32::from_rgb(0xe0, 0xe0, 0xe0),
];

pub fn apply_fluent_dark(ctx: &egui::Context) {
    let mut v = Visuals::dark();
    v.panel_fill = BG;
    v.window_fill = BG;
    v.extreme_bg_color = BG_SIDE;
    v.faint_bg_color = CARD;
    v.widgets.noninteractive.bg_fill = CARD;
    v.widgets.inactive.bg_fill = Color32::from_rgb(45, 45, 45);
    v.widgets.hovered.bg_fill = Color32::from_rgb(55, 55, 55);
    v.widgets.active.bg_fill = Color32::from_rgb(60, 60, 60);
    v.selection.bg_fill = BRAND_SOFT;
    v.selection.stroke = Stroke::new(1.0, BRAND);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT2);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    v.override_text_color = Some(TEXT);
    ctx.set_visuals(v);
}
