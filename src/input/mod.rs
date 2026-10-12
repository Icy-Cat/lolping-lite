//! Global input hooks for hold+drag gestures.
//!
//! ## Crate evaluation (this milestone)
//!
//! | Option | Fit for Option/Ctrl **hold** + mouse drag | Notes |
//! |--------|-------------------------------------------|-------|
//! | **core-graphics `CGEventTap`** (chosen on macOS) | Excellent | Flags + mouse in one tap; explicit permission errors |
//! | `rdev` | OK | Silent no-events if Accessibility denied; Sonoma thread caveats |
//! | `carbonhotkey` / `global-hotkey` | Poor | Chord register, not continuous modifier hold + move |
//! | `keytap` | Partial | Keyboard-only (no mouse) in v1 |
//!
//! ## Swallow policy (conservative)
//!
//! macOS tap uses `CGEventTapOptions::ListenOnly` — we **never** swallow or
//! rewrite events this milestone. Option/Control still reach other apps.
//! Later we may switch to an active filter only while a gesture is armed.
//!
//! ## Platforms
//!
//! - macOS: `macos` module (CGEventTap + Accessibility check)
//! - Windows: stub (`windows`)
//! - others: stub reporting unsupported

use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod stub;

#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(dead_code)] // constructed on macOS; matched on all targets in the UI
pub enum HookStatus {
    Starting,
    Running,
    NeedsAccessibility,
    Unsupported(&'static str),
    Failed(&'static str),
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)] // constructed on macOS hook thread
pub enum InputEvent {
    /// Option / Alt style modifier for ping wheel.
    PingModifier { down: bool },
    /// Control modifier for emote wheel.
    EmoteModifier { down: bool },
    MouseMove { x: f64, y: f64 },
    /// Hook thread status (permission etc.).
    Status(HookStatus),
}

#[derive(Debug, Clone, Copy, Default)]
pub struct GestureSnapshot {
    pub ping_down: bool,
    pub emote_down: bool,
    pub mouse_x: f64,
    pub mouse_y: f64,
    pub move_count: u64,
}

/// Shared live state updated by the hook thread; UI reads each frame.
#[derive(Clone, Default)]
pub struct SharedInput {
    inner: Arc<Mutex<GestureSnapshot>>,
}

impl SharedInput {
    pub fn get(&self) -> GestureSnapshot {
        *self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub(crate) fn update(&self, f: impl FnOnce(&mut GestureSnapshot)) {
        if let Ok(mut g) = self.inner.lock() {
            f(&mut g);
        }
    }
}

pub struct InputHub {
    pub shared: SharedInput,
    rx: Receiver<InputEvent>,
    pub status: HookStatus,
    log_lines: Vec<String>,
}

impl InputHub {
    pub fn start() -> Self {
        let (tx, rx) = mpsc::channel();
        let shared = SharedInput::default();
        let status = spawn_platform_hook(tx.clone(), shared.clone());
        let _ = tx.send(InputEvent::Status(status));
        Self {
            shared,
            rx,
            status,
            log_lines: Vec::new(),
        }
    }

    pub fn poll(&mut self) {
        loop {
            match self.rx.try_recv() {
                Ok(ev) => self.handle(ev),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.status = HookStatus::Failed("input channel disconnected");
                    break;
                }
            }
        }
    }

    fn handle(&mut self, ev: InputEvent) {
        match ev {
            InputEvent::Status(s) => {
                self.status = s;
                self.push_log(format!("hook status: {s:?}"));
            }
            InputEvent::PingModifier { down } => {
                self.shared.update(|g| g.ping_down = down);
                self.push_log(format!("ping modifier {}", if down { "DOWN" } else { "UP" }));
            }
            InputEvent::EmoteModifier { down } => {
                self.shared.update(|g| g.emote_down = down);
                self.push_log(format!("emote modifier {}", if down { "DOWN" } else { "UP" }));
            }
            InputEvent::MouseMove { x, y } => {
                self.shared.update(|g| {
                    g.mouse_x = x;
                    g.mouse_y = y;
                    g.move_count = g.move_count.saturating_add(1);
                });
            }
        }
    }

    fn push_log(&mut self, line: String) {
        log::info!("{line}");
        self.log_lines.push(line);
        if self.log_lines.len() > 40 {
            let drain = self.log_lines.len() - 40;
            self.log_lines.drain(0..drain);
        }
    }

    pub fn recent_logs(&self) -> &[String] {
        &self.log_lines
    }
}

fn spawn_platform_hook(tx: Sender<InputEvent>, shared: SharedInput) -> HookStatus {
    #[cfg(target_os = "macos")]
    {
        macos::spawn(tx, shared)
    }
    #[cfg(target_os = "windows")]
    {
        windows::spawn(tx, shared)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        stub::spawn(tx, shared)
    }
}
