//! Card rows matching upstream SettingRow / SwitchRow / SliderRow.

use super::theme::*;
use eframe::egui::{self, Color32, RichText, Sense, Stroke, Vec2};

pub fn section_title(ui: &mut egui::Ui, text: &str) {
    ui.add_space(14.0);
    ui.label(RichText::new(text).strong().size(14.0).color(TEXT));
    ui.add_space(6.0);
}

pub fn page_title(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).strong().size(28.0).color(TEXT));
    ui.add_space(12.0);
}

/// Returns the control column `Ui` for the caller to put widgets into.
pub fn setting_card(
    ui: &mut egui::Ui,
    icon: &str,
    title: &str,
    desc: Option<&str>,
    dim: bool,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    let opacity = if dim { 0.45 } else { 1.0 };
    egui::Frame::NONE
        .fill(CARD)
        .stroke(Stroke::new(1.0, STROKE))
        .corner_radius(4.0)
        .inner_margin(egui::Margin::symmetric(16, 10))
        .show(ui, |ui| {
            ui.set_opacity(opacity);
            ui.horizontal(|ui| {
                ui.set_min_height(48.0);
                ui.add_space(2.0);
                ui.label(RichText::new(icon).size(18.0).color(TEXT2));
                ui.add_space(10.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new(title).size(14.0).color(TEXT));
                    if let Some(d) = desc {
                        ui.label(RichText::new(d).size(12.0).color(TEXT3));
                    }
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    add_contents(ui);
                });
            });
        });
    ui.add_space(3.0);
}

pub fn switch_card(
    ui: &mut egui::Ui,
    icon: &str,
    title: &str,
    desc: Option<&str>,
    checked: &mut bool,
    on_label: &str,
    off_label: &str,
    dim: bool,
) -> bool {
    let mut changed = false;
    setting_card(ui, icon, title, desc, dim, |ui| {
        let label = if *checked { on_label } else { off_label };
        ui.label(RichText::new(label).size(12.0).color(TEXT3));
        if ui.add(egui::Checkbox::without_text(checked)).changed() {
            changed = true;
        }
    });
    changed
}

pub fn slider_card_u32(
    ui: &mut egui::Ui,
    icon: &str,
    title: &str,
    desc: Option<&str>,
    value: &mut u32,
    range: std::ops::RangeInclusive<u32>,
    format: impl Fn(u32) -> String,
    dim: bool,
) -> bool {
    let mut changed = false;
    setting_card(ui, icon, title, desc, dim, |ui| {
        ui.label(
            RichText::new(format(*value))
                .size(13.0)
                .color(TEXT2)
                .monospace(),
        );
        let mut v = *value as f32;
        let r = (*range.start() as f32)..=(*range.end() as f32);
        ui.scope(|ui| {
            ui.set_width(160.0);
            if ui.add(egui::Slider::new(&mut v, r).show_value(false)).changed() {
                *value = v.round() as u32;
                changed = true;
            }
        });
    });
    changed
}

pub fn slider_card_f32(
    ui: &mut egui::Ui,
    icon: &str,
    title: &str,
    desc: Option<&str>,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    format: impl Fn(f32) -> String,
    dim: bool,
) -> bool {
    let mut changed = false;
    setting_card(ui, icon, title, desc, dim, |ui| {
        ui.label(
            RichText::new(format(*value))
                .size(13.0)
                .color(TEXT2)
                .monospace(),
        );
        ui.scope(|ui| {
            ui.set_width(160.0);
            if ui.add(egui::Slider::new(value, range.clone()).show_value(false)).changed() {
                changed = true;
            }
        });
    });
    changed
}

pub fn status_card(
    ui: &mut egui::Ui,
    on: bool,
    failed: bool,
    no_access: bool,
    title: &str,
    hint: &str,
    on_label: &str,
    off_label: &str,
    enabled: &mut bool,
) -> bool {
    let mut changed = false;
    egui::Frame::NONE
        .fill(CARD)
        .stroke(Stroke::new(1.0, STROKE))
        .corner_radius(4.0)
        .inner_margin(egui::Margin::symmetric(16, 12))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.set_min_height(52.0);
                let (dot, glow) = if failed || no_access {
                    (WARN, true)
                } else if on {
                    (OK, true)
                } else {
                    (TEXT3, false)
                };
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
                let center = rect.center();
                ui.painter().circle_filled(center, 5.0, dot);
                if glow {
                    ui.painter().circle_stroke(center, 7.0, Stroke::new(2.0, Color32::from_rgba_unmultiplied(dot.r(), dot.g(), dot.b(), 80)));
                }
                ui.add_space(10.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new(title).strong().size(16.0));
                    ui.label(RichText::new(hint).size(12.0).color(TEXT3));
                });
                if !no_access && !failed {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let label = if *enabled { on_label } else { off_label };
                        ui.label(RichText::new(label).size(12.0).color(TEXT3));
                        if ui.add(egui::Checkbox::without_text(enabled)).changed() {
                            changed = true;
                        }
                    });
                }
            });
        });
    ui.add_space(8.0);
    changed
}

pub fn permission_card(
    ui: &mut egui::Ui,
    title: &str,
    steps: &[&str],
    open_label: &str,
    on_open: impl FnOnce(),
) {
    egui::Frame::NONE
        .fill(CARD)
        .stroke(Stroke::new(1.0, WARN))
        .corner_radius(4.0)
        .inner_margin(egui::Margin::symmetric(16, 12))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("🛡").size(20.0).color(WARN));
                ui.add_space(8.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new(title).strong().size(14.0).color(TEXT));
                    ui.add_space(4.0);
                    for (i, s) in steps.iter().enumerate() {
                        ui.label(RichText::new(format!("{}. {s}", i + 1)).size(12.0).color(TEXT3));
                    }
                    ui.add_space(8.0);
                    if ui.button(RichText::new(open_label).color(Color32::WHITE)).clicked() {
                        on_open();
                    }
                });
            });
        });
    ui.add_space(8.0);
}

pub fn nav_button(ui: &mut egui::Ui, icon: &str, label: &str, selected: bool) -> bool {
    let fill = if selected { NAV_SEL } else { Color32::TRANSPARENT };
    let resp = egui::Frame::NONE
        .fill(fill)
        .corner_radius(4.0)
        .inner_margin(egui::Margin::symmetric(12, 8))
        .show(ui, |ui| {
            // Force a full-width row so SidePanel never collapses nav into a strip.
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(icon).size(16.0));
                ui.add_space(10.0);
                ui.label(
                    RichText::new(label)
                        .size(13.0)
                        .color(if selected { TEXT } else { TEXT2 }),
                );
            });
        })
        .response
        .interact(Sense::click());
    if selected {
        let r = resp.rect;
        ui.painter().rect_filled(
            egui::Rect::from_min_size(
                egui::pos2(r.left() + 2.0, r.top() + 8.0),
                Vec2::new(3.0, (r.height() - 16.0).max(12.0)),
            ),
            2.0,
            BRAND,
        );
    }
    resp.clicked()
}

pub fn primary_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(text).color(Color32::WHITE))
            .fill(BRAND)
            .corner_radius(4.0),
    )
}
