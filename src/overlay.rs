//! Transparent overlay + mouse-passthrough state machine.
//!
//! ## States
//!
//! | State            | UI chrome        | Mouse passthrough | Typical trigger                         |
//! |------------------|------------------|-------------------|-----------------------------------------|
//! | `Idle`           | beacon / FX only | **on**            | no gesture, debug UI closed             |
//! | `PingGesture`    | ping wheel       | **off**           | Option held (macOS) / Alt (local)       |
//! | `EmoteGesture`   | emote wheel      | **off**           | Control held / Ctrl (local)             |
//! | `DebugUi`        | debug panel      | **off**           | pinned, or Option+Control chord         |
//!
//! Rule: **wheel / debug / active FX needing hits → passthrough off**;
//! **idle → passthrough on**. Gestures are **not** blocked by a pinned debug panel.
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
        matches!(self, Self::Idle)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Idle => "Idle (passthrough ON)",
            Self::PingGesture => "PingGesture (passthrough OFF)",
            Self::EmoteGesture => "EmoteGesture (passthrough OFF)",
            Self::DebugUi => "DebugUi (passthrough OFF)",
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
        Self {
            state: OverlayState::DebugUi,
            applied_passthrough: None,
            debug_pinned: true,
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
            // Keep passthrough off while debug is up. FX alone does not need
            // hits — they paint under passthrough — so fx_blocking reserved.
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
