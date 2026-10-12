//! Asset discovery and on-demand loading stubs.
//!
//! Design:
//! - Install / repo keeps `assets/original/**` at full quality (animated WebP).
//! - Hot path never decodes every animation up front — only resolve paths (and
//!   later decode+cache when an emote is actually fired).
//! - Optional derived WebM lives outside `original/` and is not required.

use std::path::PathBuf;

/// Logical handle for a catalog lookup (expand as wheels land).
#[derive(Debug, Clone, Copy)]
pub enum AssetId {
    /// First emote found on disk — used by the skeleton probe button.
    FirstEmote,
}

#[derive(Debug, Default)]
pub struct AssetCatalog {
    root: PathBuf,
    emotes: Vec<String>,
    textures: Vec<PathBuf>,
    sounds: Vec<PathBuf>,
}

impl AssetCatalog {
    /// Locate `assets/original` relative to CWD or the executable.
    pub fn discover() -> Self {
        let root = find_assets_root().unwrap_or_else(|| PathBuf::from("assets/original"));
        let mut catalog = Self {
            root: root.clone(),
            ..Default::default()
        };
        catalog.scan();
        catalog
    }

    fn scan(&mut self) {
        let emote_dir = self.root.join("emotes");
        if let Ok(rd) = std::fs::read_dir(&emote_dir) {
            let mut names = Vec::new();
            for entry in rd.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("webp") {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        names.push(stem.to_string());
                    }
                }
            }
            names.sort();
            self.emotes = names;
        }

        self.textures = list_files(self.root.join("textures"), &["png"]);
        self.sounds = list_files(self.root.join("sounds"), &["wav", "ogg"]);
    }

    pub fn emote_count(&self) -> usize {
        self.emotes.len()
    }

    pub fn texture_count(&self) -> usize {
        self.textures.len()
    }

    pub fn sound_count(&self) -> usize {
        self.sounds.len()
    }

    /// Path to the original animated WebP — no decode yet (on-demand).
    pub fn emote_animation_path(&self, id: AssetId) -> Option<PathBuf> {
        match id {
            AssetId::FirstEmote => {
                let name = self.emotes.first()?;
                Some(self.root.join("emotes").join(format!("{name}.webp")))
            }
        }
    }

    /// Future: decode WebP frames into a GPU/CPU cache when first used.
    #[allow(dead_code)]
    pub fn load_emote_animation_lazy(&self, id: AssetId) -> Option<PathBuf> {
        // Placeholder for a decode+cache pipeline. Callers should treat the
        // returned path as "load from here when needed", not preload-all.
        self.emote_animation_path(id)
    }
}

fn list_files(dir: PathBuf, exts: &[&str]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in rd.flatten() {
        let path = entry.path();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if exts.iter().any(|x| x.eq_ignore_ascii_case(ext)) {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

fn find_assets_root() -> Option<PathBuf> {
    let candidates = [
        PathBuf::from("assets/original"),
        PathBuf::from("../assets/original"),
        // When launched from target/release
        PathBuf::from("../../assets/original"),
    ];
    for c in candidates {
        if c.join("emotes").is_dir() {
            return Some(c);
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let c = dir.join("assets/original");
            if c.join("emotes").is_dir() {
                return Some(c);
            }
        }
    }
    None
}
