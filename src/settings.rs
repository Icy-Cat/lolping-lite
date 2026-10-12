//! Persisted settings (JSON), lolPing-inspired subset.

use crate::i18n::Lang;
use crate::wheel::{PINGS, WHEEL_SLOTS};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TriggerKey {
    Alt,
    Ctrl,
    Shift,
    Win,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub version: u32,
    pub trigger: TriggerKey,
    pub emote_trigger: TriggerKey,
    pub ping_size_px: u32,
    pub emote_size_px: u32,
    pub ping_duration_s: f32,
    pub volume: u8,
    pub muted: bool,
    pub language: Lang,
    pub launch_at_login: bool,
    pub display_name: String,
    pub tag_color: u8,
    pub allow_internet: bool,
    pub enabled: bool,
    /// Ping wheel slot ids (PINGS[].id), length 8.
    pub wheel: Vec<String>,
    /// Emote wheel names, length 8.
    pub emote_wheel: Vec<String>,
    pub auto_update_check: bool,
    /// Pause chord: Ctrl+Alt+P (documented; mods fixed for now).
    pub pause_hotkey_label: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: 1,
            trigger: TriggerKey::Alt,
            emote_trigger: TriggerKey::Ctrl,
            ping_size_px: 110,
            emote_size_px: 150,
            ping_duration_s: 3.2,
            volume: 70,
            muted: false,
            language: Lang::En,
            launch_at_login: false,
            display_name: "Player".into(),
            tag_color: 0,
            allow_internet: true,
            enabled: true,
            wheel: PINGS.iter().map(|p| p.id.to_string()).collect(),
            emote_wheel: Vec::new(),
            auto_update_check: false,
            pause_hotkey_label: "Ctrl+Alt+P".into(),
        }
    }
}

impl Settings {
    pub fn config_path() -> PathBuf {
        if let Some(dir) = dirs_next_config() {
            dir.join("lolping-lite").join("settings.json")
        } else {
            PathBuf::from("settings.json")
        }
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        match fs::read_to_string(&path) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let s = serde_json::to_string_pretty(self)?;
        fs::write(&path, s).with_context(|| format!("write {}", path.display()))?;
        Ok(())
    }

    pub fn ensure_emote_wheel(&mut self, names: &[String]) {
        if self.emote_wheel.len() > WHEEL_SLOTS {
            self.emote_wheel.truncate(WHEEL_SLOTS);
        }
        for n in names.iter() {
            if self.emote_wheel.len() >= WHEEL_SLOTS {
                break;
            }
            if !self.emote_wheel.contains(n) {
                self.emote_wheel.push(n.clone());
            }
        }
        while self.emote_wheel.len() < WHEEL_SLOTS {
            self.emote_wheel.push(String::new());
        }
    }

    pub fn ensure_ping_wheel(&mut self) {
        if self.wheel.len() != WHEEL_SLOTS {
            self.wheel = PINGS.iter().map(|p| p.id.to_string()).collect();
        }
        for id in &mut self.wheel {
            if !PINGS.iter().any(|p| p.id == id.as_str()) {
                *id = PINGS[0].id.to_string();
            }
        }
    }

    pub fn ping_def_at(&self, slot: usize) -> &'static crate::wheel::PingDef {
        let id = self.wheel.get(slot).map(|s| s.as_str()).unwrap_or("omw");
        PINGS
            .iter()
            .find(|p| p.id == id)
            .unwrap_or(&PINGS[0])
    }
}

fn dirs_next_config() -> Option<PathBuf> {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(xdg));
    }
    if let Ok(home) = std::env::var("HOME") {
        #[cfg(target_os = "macos")]
        {
            return Some(PathBuf::from(home).join("Library/Application Support"));
        }
        #[cfg(not(target_os = "macos"))]
        {
            return Some(PathBuf::from(home).join(".config"));
        }
    }
    if let Ok(ad) = std::env::var("APPDATA") {
        return Some(PathBuf::from(ad));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_default() {
        let s = Settings::default();
        let j = serde_json::to_string(&s).unwrap();
        let back: Settings = serde_json::from_str(&j).unwrap();
        assert_eq!(back.ping_size_px, 110);
        assert_eq!(back.wheel.len(), 8);
    }

    #[test]
    fn input_config_from_settings() {
        let mut s = Settings::default();
        s.trigger = TriggerKey::Shift;
        s.emote_trigger = TriggerKey::Win;
        s.enabled = false;
        let c = crate::input::InputConfig::from_settings(&s);
        assert_eq!(c.ping_trigger, TriggerKey::Shift);
        assert!(!c.enabled);
    }
}
