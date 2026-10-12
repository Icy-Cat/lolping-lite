//! Hold-modifier gesture → wheel → place / cancel.

use crate::input::decision::ChordGuard;
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
    /// Upstream c104b08: trigger is part of another program's chord (Alt+Tab …).
    pub chord: ChordGuard,
    /// Last trigger press was ignored because a fullscreen app owns that monitor.
    pub game_paused: bool,
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

    /// A non-modifier key went down (from the hooks). If a trigger is held and no wheel slot
    /// has been chosen yet, the trigger belongs to another chord: drop the wheel silently and
    /// block new gestures until every trigger is released. Returns true if it marked a chord.
    pub fn foreign_key(&mut self, class: crate::input::decision::KeyClass) -> bool {
        let held = self.prev_ping || self.prev_emote;
        let idle = self.wheel.as_ref().is_none_or(|w| w.hover.is_none());
        let marked = self.chord.on_key_down(class, held, idle);
        if marked {
            self.wheel = None;
        }
        marked
    }

    /// Drive from modifier edges + cursor. `cancel_key` = Esc this frame.
    pub fn tick(
        &mut self,
        ping_down: bool,
        emote_down: bool,
        cursor: Pos2,
        cancel_key: bool,
    ) -> GestureOutcome {
        self.tick_gated(ping_down, emote_down, cursor, cancel_key, &mut || false)
    }

    /// Like [`tick`](Self::tick) with the upstream fullscreen gate: `fullscreen_probe` is asked
    /// only when a gesture would actually start (so ordinary frames never pay for it); when it
    /// says a fullscreen app owns the cursor's monitor, the trigger press is ignored.
    pub fn tick_gated(
        &mut self,
        ping_down: bool,
        emote_down: bool,
        cursor: Pos2,
        cancel_key: bool,
        fullscreen_probe: &mut dyn FnMut() -> bool,
    ) -> GestureOutcome {
        self.chord.sync(ping_down || emote_down);
        if !ping_down && !emote_down {
            self.game_paused = false;
        }
        if cancel_key {
            self.prev_ping = ping_down;
            self.prev_emote = emote_down;
            if self.wheel.take().is_some() {
                return GestureOutcome::Cancelled;
            }
            return GestureOutcome::None;
        }

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
        } else if (ping_rise || emote_rise) && self.chord.blocked() {
            // the trigger is part of another program's chord
        } else if (ping_rise || emote_rise) && fullscreen_probe() {
            self.game_paused = true;
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
                (GestureKind::Ping, Some(i)) => format!("ping: {}", wheel::PINGS[i].label),
                (GestureKind::Ping, None) => "ping: cancel".into(),
                (GestureKind::Emote, Some(i)) => format!("emote slot {i}"),
                (GestureKind::Emote, None) => "emote: cancel".into(),
            },
            None => String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    //! Upstream helper_tests.cpp cases on lolping-lite's hold-trigger gesture model.
    use super::*;
    use crate::input::decision::{classify_vk, KeyClass};
    use crate::wheel::WHEEL_RADIUS;

    const C: Pos2 = Pos2::new(200.0, 200.0);

    // test_alt_tab_clicks_pass_through + ..._click_places_no_ping_even_with_click_ping_on
    #[test]
    fn alt_tab_places_nothing_then_gestures_again() {
        let mut g = GestureSession::default();
        g.tick(true, false, C, false); // Alt down: wheel opens (dead zone)
        assert!(g.wheel_active());
        assert!(g.foreign_key(classify_vk(0x09))); // Tab
        assert!(!g.wheel_active(), "the switcher owns the chord");
        let up = Pos2::new(200.0, 200.0 - WHEEL_RADIUS * 0.7);
        assert!(matches!(g.tick(true, false, up, false), GestureOutcome::None));
        assert!(matches!(g.tick(false, false, up, false), GestureOutcome::None), "no ping on Alt-up");
        g.tick(true, false, C, false); // chord over: Alt gestures again
        assert!(g.wheel_active());
    }

    #[test]
    fn mid_gesture_keys_and_modifiers_do_not_mark() {
        let mut g = GestureSession::default();
        g.tick(true, false, C, false);
        assert!(!g.foreign_key(KeyClass::Modifier)); // Alt+Shift
        let up = Pos2::new(200.0, 200.0 - WHEEL_RADIUS * 0.7);
        g.tick(true, false, up, false); // slot chosen → mid-gesture
        assert!(!g.foreign_key(KeyClass::Other));
        assert!(matches!(g.tick(false, false, up, false), GestureOutcome::Placed { slot: 0, .. }));
    }

    // test_fullscreen_game_keeps_the_click
    #[test]
    fn fullscreen_game_keeps_the_trigger() {
        let mut g = GestureSession::default();
        let mut fullscreen = true;
        g.tick_gated(true, false, C, false, &mut || fullscreen);
        assert!(!g.wheel_active() && g.game_paused);
        g.tick_gated(false, false, C, false, &mut || fullscreen);
        fullscreen = false; // back on the desktop
        g.tick_gated(true, false, C, false, &mut || fullscreen);
        assert!(g.wheel_active() && !g.game_paused);
    }

    // test_fullscreen_probe_ignored_when_disabled: the app passes a probe that is never
    // consulted when gameCheck is off; and plain frames never consult it.
    #[test]
    fn probe_only_on_would_be_gesture() {
        let mut g = GestureSession::default();
        let mut calls = 0;
        g.tick_gated(false, false, C, false, &mut || { calls += 1; true });
        assert_eq!(calls, 0);
        g.tick_gated(true, false, C, false, &mut || { calls += 1; false });
        g.tick_gated(true, false, C, false, &mut || { calls += 1; false });
        assert_eq!(calls, 1, "probed once per press");
    }
}
