//! Live input configuration shared with the hook thread.

use crate::settings::TriggerKey;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct InputConfig {
    pub ping_trigger: TriggerKey,
    pub emote_trigger: TriggerKey,
    /// When false, hooks still run but report paused (no gestures).
    pub enabled: bool,
    /// Pause chord: Ctrl+Alt+P by default (VK 0x50).
    pub pause_requires_ctrl: bool,
    pub pause_requires_alt: bool,
    pub pause_requires_shift: bool,
    pub pause_requires_meta: bool,
    /// Virtual-key / keycode for pause (macOS kVK_ANSI_P = 0x23, Windows 'P' = 0x50).
    pub pause_key_vk: u16,
}

impl Default for InputConfig {
    fn default() -> Self {
        Self {
            ping_trigger: TriggerKey::Alt,
            emote_trigger: TriggerKey::Ctrl,
            enabled: true,
            pause_requires_ctrl: true,
            pause_requires_alt: true,
            pause_requires_shift: false,
            pause_requires_meta: false,
            pause_key_vk: 0x50, // 'P' Win; macOS remaps in platform module
        }
    }
}

impl InputConfig {
    pub fn from_settings(s: &crate::settings::Settings) -> Self {
        Self {
            ping_trigger: s.trigger,
            emote_trigger: s.emote_trigger,
            enabled: s.enabled,
            ..Self::default()
        }
    }

    /// Map raw platform modifiers to (ping_down, emote_down).
    pub fn map_modifiers(&self, alt: bool, ctrl: bool, shift: bool, meta: bool) -> (bool, bool) {
        let ping = match self.ping_trigger {
            TriggerKey::Alt => alt,
            TriggerKey::Ctrl => ctrl,
            TriggerKey::Shift => shift,
            TriggerKey::Win => meta,
        };
        let emote = match self.emote_trigger {
            TriggerKey::Alt => alt,
            TriggerKey::Ctrl => ctrl,
            TriggerKey::Shift => shift,
            TriggerKey::Win => meta,
        };
        (ping, emote)
    }

    pub fn pause_chord_matches(&self, alt: bool, ctrl: bool, shift: bool, meta: bool) -> bool {
        (!self.pause_requires_ctrl || ctrl)
            && (!self.pause_requires_alt || alt)
            && (!self.pause_requires_shift || shift)
            && (!self.pause_requires_meta || meta)
    }
}

pub type SharedConfig = Arc<Mutex<InputConfig>>;

pub fn new_shared(cfg: InputConfig) -> SharedConfig {
    Arc::new(Mutex::new(cfg))
}

pub fn with_config(shared: &SharedConfig, f: impl FnOnce(&InputConfig)) {
    if let Ok(g) = shared.lock() {
        f(&g);
    }
}

pub fn update_config(shared: &SharedConfig, f: impl FnOnce(&mut InputConfig)) {
    if let Ok(mut g) = shared.lock() {
        f(&mut g);
    }
}
