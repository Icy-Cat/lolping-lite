//! Transparent overlay + mouse-passthrough state machine.
//!
//! ## States
//!
//! | State            | UI chrome        | Mouse passthrough | Typical trigger                         |
//! |------------------|------------------|-------------------|-----------------------------------------|
//! | `Idle`           | beacon only      | **on**            | no gesture, debug UI closed             |
//! | `PingGesture`    | ping HUD         | **off**           | Option held (macOS)                     |
//! | `EmoteGesture`   | emote HUD        | **off**           | Control held (macOS)                    |
//! | `DebugUi`        | full debug panel | **off**           | pinned, or Option+Control chord         |
//!
//! Rule (product): **wheel / debug UI visible → passthrough off**;
//! **hidden / idle → passthrough on** so clicks reach apps underneath.
//!
//! egui 0.32 exposes `ViewportBuilder::with_mouse_passthrough` and
//! `ViewportCommand::MousePassthrough`. On platforms where
//! `window.set_cursor_hittest` fails, egui-winit logs the error and we keep
//! trying each transition (see `apply`).
//!
//! Escape hatch while click-through: hold **Option+Control** together to
//! re-pin the debug UI (works because CGEventTap still sees flags).

use eframe::egui::{self, ViewportCommand};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayState {
    Idle,
    PingGesture,
    EmoteGesture,
    DebugUi,
}

impl OverlayState {
    pub fn ui_visible(self) -> bool {
        !matches!(self, Self::Idle)
    }

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
        // Start interactive so the user can see status / grant Accessibility.
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
        if pinned {
            self.state = OverlayState::DebugUi;
        } else if matches!(self.state, OverlayState::DebugUi) {
            self.state = OverlayState::Idle;
        }
    }

    /// Modifier / gesture updates from the input hook.
    pub fn on_gesture(&mut self, ping: bool, emote: bool) {
        // Chord: both modifiers → re-open debug even if click-through.
        if ping && emote {
            self.debug_pinned = true;
            self.state = OverlayState::DebugUi;
            return;
        }
        if self.debug_pinned {
            self.state = OverlayState::DebugUi;
            return;
        }
        self.state = if ping {
            OverlayState::PingGesture
        } else if emote {
            OverlayState::EmoteGesture
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
