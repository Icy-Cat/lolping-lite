//! Ping / emote wheel geometry and painting.

use eframe::egui::{
    self, Color32, Pos2, Rect, Sense, Shape, Stroke, TextureHandle, Vec2,
};

pub const WHEEL_SLOTS: usize = 8;
pub const WHEEL_RADIUS: f32 = 140.0;
pub const DEADZONE_RADIUS: f32 = 36.0;
pub const ICON_SIZE: f32 = 40.0;

/// Default LoL-style ping order: top, then clockwise.
#[derive(Debug, Clone, Copy)]
pub struct PingDef {
    pub id: &'static str,
    pub label: &'static str,
    pub texture_file: &'static str,
    pub sound_file: Option<&'static str>,
}

pub const PINGS: [PingDef; WHEEL_SLOTS] = [
    PingDef {
        id: "danger",
        label: "Danger",
        texture_file: "pingwheel_retreatrender.png",
        sound_file: Some("SRP_4.wav"),
    },
    PingDef {
        id: "push",
        label: "Push",
        texture_file: "push.png",
        sound_file: Some("SRP_6.wav"),
    },
    PingDef {
        id: "omw",
        label: "On My Way",
        texture_file: "omw.png",
        sound_file: Some("OnMyWay.wav"),
    },
    PingDef {
        id: "all_in",
        label: "All In",
        texture_file: "all_in.png",
        sound_file: Some("SRP_7.wav"),
    },
    PingDef {
        id: "assist",
        label: "Assist Me",
        texture_file: "assist_me.png",
        sound_file: Some("ComeHere.wav"),
    },
    PingDef {
        id: "need_vision",
        label: "Need Vision",
        texture_file: "need_vision.png",
        sound_file: Some("SRP_9.wav"),
    },
    PingDef {
        id: "mia",
        label: "Enemy Missing",
        texture_file: "enemy_missing.png",
        sound_file: Some("MIA.wav"),
    },
    PingDef {
        id: "enemy_vision",
        label: "Enemy Vision",
        texture_file: "enemy_vision.png",
        sound_file: Some("AreaIsWarded.wav"),
    },
];

/// Slot index under cursor, or `None` if inside the cancel dead-zone.
pub fn hover_slot(center: Pos2, cursor: Pos2) -> Option<usize> {
    let d = cursor - center;
    let dist = d.length();
    if dist < DEADZONE_RADIUS {
        return None;
    }
    // atan2: x right, y down in egui → convert so 0 = top, clockwise.
    let mut deg = d.x.atan2(-d.y).to_degrees();
    if deg < 0.0 {
        deg += 360.0;
    }
    let slot = ((deg + 22.5) / 45.0).floor() as isize % 8;
    Some(slot.rem_euclid(8) as usize)
}

pub fn slot_center(wheel_center: Pos2, slot: usize) -> Pos2 {
    let angle = (slot as f32) * std::f32::consts::TAU / 8.0 - std::f32::consts::FRAC_PI_2;
    let r = (DEADZONE_RADIUS + WHEEL_RADIUS) * 0.55;
    wheel_center + Vec2::angled(angle) * r
}

pub struct WheelPaint<'a> {
    pub center: Pos2,
    pub cursor: Pos2,
    pub hover: Option<usize>,
    pub icons: &'a [Option<TextureHandle>],
    pub kind_label: &'static str,
}

pub fn paint_wheel(ui: &mut egui::Ui, p: WheelPaint<'_>) {
    let rect = Rect::from_center_size(p.center, Vec2::splat(WHEEL_RADIUS * 2.0 + 24.0));
    // Allocate so the area participates in hits while passthrough is off.
    let _ = ui.allocate_rect(rect, Sense::hover());
    let painter = ui.painter();

    painter.circle_filled(
        p.center,
        WHEEL_RADIUS,
        Color32::from_rgba_unmultiplied(12, 14, 20, 200),
    );
    painter.circle_stroke(
        p.center,
        WHEEL_RADIUS,
        Stroke::new(2.0, Color32::from_rgba_unmultiplied(255, 255, 255, 60)),
    );
    painter.circle_filled(
        p.center,
        DEADZONE_RADIUS,
        Color32::from_rgba_unmultiplied(30, 30, 36, 220),
    );
    painter.text(
        p.center,
        egui::Align2::CENTER_CENTER,
        "cancel",
        egui::FontId::proportional(12.0),
        Color32::from_rgba_unmultiplied(200, 200, 200, 180),
    );

    for i in 0..WHEEL_SLOTS {
        let c = slot_center(p.center, i);
        let selected = p.hover == Some(i);
        if selected {
            painter.circle_filled(
                c,
                ICON_SIZE * 0.75,
                Color32::from_rgba_unmultiplied(80, 140, 255, 90),
            );
        }
        if let Some(Some(tex)) = p.icons.get(i) {
            let size = Vec2::splat(if selected { ICON_SIZE + 6.0 } else { ICON_SIZE });
            let r = Rect::from_center_size(c, size);
            painter.image(tex.id(), r, Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
        } else {
            painter.circle_filled(c, 10.0, Color32::GRAY);
        }
        // Sector tick
        let a0 = (i as f32) * std::f32::consts::TAU / 8.0 - std::f32::consts::FRAC_PI_2
            - std::f32::consts::TAU / 16.0;
        let p0 = p.center + Vec2::angled(a0) * DEADZONE_RADIUS;
        let p1 = p.center + Vec2::angled(a0) * WHEEL_RADIUS;
        painter.line_segment(
            [p0, p1],
            Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 40)),
        );
    }

    // Cursor line
    painter.add(Shape::line_segment(
        [p.center, p.cursor],
        Stroke::new(1.5, Color32::from_rgba_unmultiplied(255, 220, 120, 160)),
    ));
    painter.circle_filled(p.cursor, 4.0, Color32::from_rgb(255, 220, 120));

    painter.text(
        p.center - Vec2::new(0.0, WHEEL_RADIUS + 16.0),
        egui::Align2::CENTER_CENTER,
        p.kind_label,
        egui::FontId::proportional(14.0),
        Color32::WHITE,
    );
}
