//! Transparent overlay + mouse-passthrough state machine.
//!
//! ## States
//!
//! | State            | UI chrome        | Mouse passthrough | Typical trigger                         |
//! |------------------|------------------|-------------------|-----------------------------------------|
//! | `Idle`           | beacon / FX only | **on**            | no gesture, debug UI closed             |
//! | `PingGesture`    | ping wheel       | **off**           | Option held (macOS) / Alt (local)       |
//! | `EmoteGesture`   | emote wheel      | **off**           | Control held / Ctrl (local)             |
//! | `DebugUi`        | debug panel      | **on**            | pinned, or Option+Control chord         |
//!
//! Rule: **only active wheels need hits → passthrough off**;
//! **idle / debug → passthrough on**. The debug/settings panel lives on the
//! root window, not the fullscreen overlays — so pinning debug must NOT steal
//! clicks from the rest of the desktop (that used to feel like a system freeze).
//!
//! Escape hatch while click-through: hold **Option+Control** to re-pin debug.

use eframe::egui::{self, ViewportCommand};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayState {
    Idle,
    PingGesture,
    EmoteGesture,
    DebugUi,
}

impl OverlayState {
    pub fn mouse_passthrough(self) -> bool {
        // Fullscreen overlay viewports: only gesture wheels need to receive hits.
        // DebugUi paints on the root window, so overlays stay click-through.
        matches!(self, Self::Idle | Self::DebugUi)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Idle => "Idle (passthrough ON)",
            Self::PingGesture => "PingGesture (passthrough OFF)",
            Self::EmoteGesture => "EmoteGesture (passthrough OFF)",
            Self::DebugUi => "DebugUi (passthrough ON)",
        }
    }
}

#[derive(Debug)]
pub struct OverlayController {
    state: OverlayState,
    applied_passthrough: Option<bool>,
    debug_pinned: bool,
}

impl Default for OverlayController {
    fn default() -> Self {
        // Release default: idle + click-through so launching the .app never
        // blankets the desktop. `--settings` / Accessibility prompt pin debug.
        Self {
            state: OverlayState::Idle,
            applied_passthrough: None,
            debug_pinned: false,
        }
    }
}

impl OverlayController {
    pub fn state(&self) -> OverlayState {
        self.state
    }

    pub fn debug_pinned(&self) -> bool {
        self.debug_pinned
    }

    pub fn set_debug_pinned(&mut self, pinned: bool) {
        self.debug_pinned = pinned;
        self.recompute(None, false);
    }

    /// `gesture` overrides idle/debug for passthrough; chord pins debug.
    pub fn sync(
        &mut self,
        ping: bool,
        emote: bool,
        gesture: Option<OverlayState>,
        fx_blocking: bool,
    ) {
        if ping && emote {
            self.debug_pinned = true;
        }
        self.recompute(gesture, fx_blocking);
    }

    fn recompute(&mut self, gesture: Option<OverlayState>, fx_blocking: bool) {
        self.state = if let Some(g) = gesture {
            g
        } else if self.debug_pinned || fx_blocking {
            // Debug pin shows the root-window panel; overlays stay Idle/DebugUi
            // with passthrough on. FX paints under passthrough.
            let _ = fx_blocking;
            if self.debug_pinned {
                OverlayState::DebugUi
            } else {
                OverlayState::Idle
            }
        } else {
            OverlayState::Idle
        };
    }

    pub fn apply(&mut self, ctx: &egui::Context) {
        let want = self.state.mouse_passthrough();
        if self.applied_passthrough != Some(want) {
            ctx.send_viewport_cmd(ViewportCommand::MousePassthrough(want));
            self.applied_passthrough = Some(want);
            log::debug!(
                "viewport mouse_passthrough = {want} ({})",
                self.state.label()
            );
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_click_through() {
        let o = OverlayController::default();
        assert!(!o.debug_pinned());
        assert_eq!(o.state(), OverlayState::Idle);
        assert!(o.state().mouse_passthrough());
    }

    #[test]
    fn debug_pin_does_not_steal_desktop_clicks() {
        let mut o = OverlayController::default();
        o.set_debug_pinned(true);
        assert_eq!(o.state(), OverlayState::DebugUi);
        assert!(
            o.state().mouse_passthrough(),
            "DebugUi must keep overlay passthrough ON"
        );
    }

    #[test]
    fn gesture_disables_passthrough() {
        assert!(!OverlayState::PingGesture.mouse_passthrough());
        assert!(!OverlayState::EmoteGesture.mouse_passthrough());
    }
}
