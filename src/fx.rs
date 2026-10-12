//! Short-lived placed ping / emote visuals on the overlay.

use eframe::egui::{self, Color32, ColorImage, Pos2, Rect, TextureHandle, Vec2};
use std::time::{Duration, Instant};

pub struct PlacedPing {
    pub at: Pos2,
    pub tex: TextureHandle,
    pub born: Instant,
    pub life: Duration,
}

pub struct EmoteAnim {
    pub name: String,
    /// Pre-uploaded frame textures (single emote cache).
    pub frames: Vec<TextureHandle>,
    pub delays_ms: Vec<u32>,
}

pub struct PlacedEmote {
    pub at: Pos2,
    pub anim: EmoteAnim,
    pub born: Instant,
    pub frame: usize,
    pub frame_started: Instant,
    /// Play through this many loops then fade (1 = once).
    pub loops_left: u32,
    pub fading: bool,
    pub fade_born: Option<Instant>,
}

pub struct FxLayer {
    pub pings: Vec<PlacedPing>,
    pub emotes: Vec<PlacedEmote>,
}

impl Default for FxLayer {
    fn default() -> Self {
        Self {
            pings: Vec::new(),
            emotes: Vec::new(),
        }
    }
}

impl FxLayer {
    pub fn push_ping(&mut self, at: Pos2, tex: TextureHandle) {
        self.pings.push(PlacedPing {
            at,
            tex,
            born: Instant::now(),
            life: Duration::from_millis(1600),
        });
    }

    pub fn push_emote(&mut self, at: Pos2, anim: EmoteAnim) {
        self.emotes.push(PlacedEmote {
            at,
            anim,
            born: Instant::now(),
            frame: 0,
            frame_started: Instant::now(),
            loops_left: 1,
            fading: false,
            fade_born: None,
        });
    }

    pub fn tick_and_paint(&mut self, ui: &mut egui::Ui) {
        let now = Instant::now();
        let painter = ui.painter();

        self.pings.retain(|p| now.duration_since(p.born) < p.life);
        for p in &self.pings {
            let t = now.duration_since(p.born).as_secs_f32() / p.life.as_secs_f32();
            let alpha = ((1.0 - t) * 255.0).clamp(0.0, 255.0) as u8;
            let size = 48.0 + 12.0 * (1.0 - t);
            let rect = Rect::from_center_size(p.at, Vec2::splat(size));
            painter.image(
                p.tex.id(),
                rect,
                Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                Color32::from_rgba_unmultiplied(255, 255, 255, alpha),
            );
        }

        let mut keep = Vec::new();
        for mut e in self.emotes.drain(..) {
            if e.anim.frames.is_empty() {
                continue;
            }
            // Advance frames
            let delay = Duration::from_millis(e.anim.delays_ms.get(e.frame).copied().unwrap_or(33) as u64);
            if now.duration_since(e.frame_started) >= delay {
                e.frame_started = now;
                e.frame += 1;
                if e.frame >= e.anim.frames.len() {
                    e.frame = 0;
                    if e.loops_left > 0 {
                        e.loops_left -= 1;
                    }
                    if e.loops_left == 0 {
                        e.fading = true;
                        e.fade_born = Some(now);
                    }
                }
            }

            let mut alpha: u8 = 255;
            if e.fading {
                let fb = e.fade_born.unwrap_or(now);
                let ft = now.duration_since(fb).as_secs_f32() / 0.45;
                if ft >= 1.0 {
                    continue;
                }
                alpha = ((1.0 - ft) * 255.0).clamp(0.0, 255.0) as u8;
            }

            let tex = &e.anim.frames[e.frame.min(e.anim.frames.len() - 1)];
            let rect = Rect::from_center_size(e.at, Vec2::splat(96.0));
            painter.image(
                tex.id(),
                rect,
                Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                Color32::from_rgba_unmultiplied(255, 255, 255, alpha),
            );
            keep.push(e);
        }
        self.emotes = keep;
    }

    pub fn active(&self) -> bool {
        !self.pings.is_empty() || !self.emotes.is_empty()
    }
}

/// Helper to upload an RGBA buffer as an egui texture.
pub fn upload_rgba(
    ctx: &egui::Context,
    name: impl Into<String>,
    w: u32,
    h: u32,
    raw: &[u8],
) -> TextureHandle {
    let image = ColorImage::from_rgba_unmultiplied([w as usize, h as usize], raw);
    ctx.load_texture(name, image, Default::default())
}
