//! Hold-modifier gesture → wheel → place / cancel.

use crate::overlay::OverlayState;
use crate::wheel::{self, hover_slot};
use eframe::egui::Pos2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GestureKind {
    Ping,
    Emote,
}

#[derive(Debug, Clone)]
pub struct ActiveWheel {
    pub kind: GestureKind,
    pub center: Pos2,
    pub hover: Option<usize>,
}

#[derive(Debug, Clone, Copy)]
pub enum GestureCommand {
    /// Open wheel centered at cursor.
    Begin { kind: GestureKind, center: Pos2 },
    /// Update hover from cursor.
    Move { cursor: Pos2 },
    /// Modifier released — place hover slot or cancel (dead-zone).
    End { cursor: Pos2 },
    /// Esc / explicit cancel.
    Cancel,
}

#[derive(Debug, Clone)]
pub enum GestureOutcome {
    None,
    Placed {
        kind: GestureKind,
        slot: usize,
        at: Pos2,
    },
    Cancelled,
}

#[derive(Debug, Default)]
pub struct GestureSession {
    pub wheel: Option<ActiveWheel>,
    prev_ping: bool,
    prev_emote: bool,
}

impl GestureSession {
    pub fn wheel_active(&self) -> bool {
        self.wheel.is_some()
    }

    pub fn overlay_hint(&self) -> Option<OverlayState> {
        self.wheel.as_ref().map(|w| match w.kind {
            GestureKind::Ping => OverlayState::PingGesture,
            GestureKind::Emote => OverlayState::EmoteGesture,
        })
    }

    /// Drive from modifier edges + cursor. `cancel_key` = Esc this frame.
    pub fn tick(
        &mut self,
        ping_down: bool,
        emote_down: bool,
        cursor: Pos2,
        cancel_key: bool,
    ) -> GestureOutcome {
        if cancel_key {
            self.prev_ping = ping_down;
            self.prev_emote = emote_down;
            if self.wheel.take().is_some() {
                return GestureOutcome::Cancelled;
            }
            return GestureOutcome::None;
        }

        // Rising edges — prefer ping if both (chord handled by overlay debug).
        let ping_rise = ping_down && !self.prev_ping;
        let emote_rise = emote_down && !self.prev_emote;
        let ping_fall = !ping_down && self.prev_ping;
        let emote_fall = !emote_down && self.prev_emote;

        let mut outcome = GestureOutcome::None;

        if let Some(w) = self.wheel.as_mut() {
            w.hover = hover_slot(w.center, cursor);
            let end = match w.kind {
                GestureKind::Ping => ping_fall,
                GestureKind::Emote => emote_fall,
            };
            if end {
                let finished = self.wheel.take().unwrap();
                outcome = if let Some(slot) = finished.hover {
                    GestureOutcome::Placed {
                        kind: finished.kind,
                        slot,
                        at: cursor,
                    }
                } else {
                    GestureOutcome::Cancelled
                };
            }
        } else if ping_rise {
            self.wheel = Some(ActiveWheel {
                kind: GestureKind::Ping,
                center: cursor,
                hover: hover_slot(cursor, cursor),
            });
        } else if emote_rise {
            self.wheel = Some(ActiveWheel {
                kind: GestureKind::Emote,
                center: cursor,
                hover: hover_slot(cursor, cursor),
            });
        }

        self.prev_ping = ping_down;
        self.prev_emote = emote_down;
        outcome
    }

    pub fn hover_label(&self) -> String {
        match &self.wheel {
            Some(w) => match (w.kind, w.hover) {
                (GestureKind::Ping, Some(i)) => {
                    format!("ping: {}", wheel::PINGS[i].label)
                }
                (GestureKind::Ping, None) => "ping: cancel".into(),
                (GestureKind::Emote, Some(i)) => format!("emote slot {i}"),
                (GestureKind::Emote, None) => "emote: cancel".into(),
            },
            None => String::new(),
        }
    }
}
