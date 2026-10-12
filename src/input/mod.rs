//! Global input hooks — settings-driven triggers + pause chord.

mod config;
pub mod decision;
pub mod fullscreen;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod stub;

pub use config::{new_shared, update_config, InputConfig, SharedConfig};

use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(dead_code)]
pub enum HookStatus {
    Starting,
    Running,
    NeedsAccessibility,
    Unsupported(&'static str),
    Failed(&'static str),
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub enum InputEvent {
    PingModifier { down: bool },
    EmoteModifier { down: bool },
    MouseMove { x: f64, y: f64 },
    /// A non-modifier key went down while a wheel trigger was held (Alt+Tab, Win+Tab …).
    ForeignKey,
    /// Pause hotkey toggled enabled state.
    ToggleEnabled,
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
    pub config: SharedConfig,
    rx: Receiver<InputEvent>,
    pub status: HookStatus,
    log_lines: Vec<String>,
    /// Mirror of settings.enabled for UI.
    pub enabled: bool,
}

impl InputHub {
    pub fn start(cfg: InputConfig) -> Self {
        let (tx, rx) = mpsc::channel();
        let shared = SharedInput::default();
        let config = new_shared(cfg.clone());
        let status = spawn_platform_hook(tx.clone(), shared.clone(), config.clone());
        let _ = tx.send(InputEvent::Status(status));
        Self {
            shared,
            config,
            rx,
            status,
            log_lines: Vec::new(),
            enabled: cfg.enabled,
        }
    }

    pub fn apply_settings(&self, s: &crate::settings::Settings) {
        update_config(&self.config, |c| {
            *c = InputConfig::from_settings(s);
        });
    }

    pub fn poll(&mut self) -> Vec<InputEvent> {
        let mut extras = Vec::new();
        loop {
            match self.rx.try_recv() {
                Ok(ev) => {
                    self.handle(&ev);
                    if matches!(ev, InputEvent::ToggleEnabled | InputEvent::ForeignKey) {
                        extras.push(ev);
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.status = HookStatus::Failed("input channel disconnected");
                    break;
                }
            }
        }
        extras
    }

    fn handle(&mut self, ev: &InputEvent) {
        match *ev {
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
            InputEvent::ForeignKey => {}
            InputEvent::MouseMove { x, y } => {
                self.shared.update(|g| {
                    g.mouse_x = x;
                    g.mouse_y = y;
                    g.move_count = g.move_count.saturating_add(1);
                });
            }
            InputEvent::ToggleEnabled => {
                update_config(&self.config, |c| {
                    c.enabled = !c.enabled;
                    self.enabled = c.enabled;
                });
                self.push_log(format!(
                    "pause hotkey → enabled={}",
                    self.enabled
                ));
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

fn spawn_platform_hook(
    tx: Sender<InputEvent>,
    shared: SharedInput,
    config: SharedConfig,
) -> HookStatus {
    #[cfg(target_os = "macos")]
    {
        macos::spawn(tx, shared, config)
    }
    #[cfg(target_os = "windows")]
    {
        windows::spawn(tx, shared, config)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        stub::spawn(tx, shared, config)
    }
}

/// Pure helper for tests: map settings triggers.
pub fn map_triggers_for_test(
    cfg: &InputConfig,
    alt: bool,
    ctrl: bool,
    shift: bool,
    meta: bool,
) -> (bool, bool) {
    cfg.map_modifiers(alt, ctrl, shift, meta)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::TriggerKey;

    #[test]
    fn settings_map_swap_triggers() {
        let mut cfg = InputConfig::default();
        cfg.ping_trigger = TriggerKey::Ctrl;
        cfg.emote_trigger = TriggerKey::Alt;
        assert_eq!(cfg.map_modifiers(true, false, false, false), (false, true));
        assert_eq!(cfg.map_modifiers(false, true, false, false), (true, false));
    }

    #[test]
    fn pause_chord() {
        let cfg = InputConfig::default();
        assert!(cfg.pause_chord_matches(true, true, false, false));
        assert!(!cfg.pause_chord_matches(true, false, false, false));
    }
}
