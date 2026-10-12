//! Asset discovery and on-demand loading.
//!
//! Startup only **lists** names/paths under `assets/original/` — it does **not**
//! decode the ~50 MB of animated WebP. Decode happens when asked
//! (`load_emote_png_thumb`, `load_emote_webp_first_frame`).

use anyhow::{anyhow, Context, Result};
use image::RgbaImage;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub enum AssetId {
    FirstEmote,
    Named(&'static str),
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct EmoteEntry {
    pub name: String,
    pub webp: PathBuf,
    pub png: PathBuf,
    pub ogg: PathBuf,
}

#[derive(Debug, Default)]
pub struct AssetCatalog {
    root: PathBuf,
    emotes: Vec<EmoteEntry>,
    textures: Vec<PathBuf>,
    sounds: Vec<PathBuf>,
    /// Single-slot cache so repeat probes don't re-decode (not a full LRU yet).
    cached_frame: Option<(String, RgbaImage)>,
}

#[allow(dead_code)]
impl AssetCatalog {
    pub fn discover() -> Self {
        let root = find_assets_root().unwrap_or_else(|| PathBuf::from("assets/original"));
        let mut catalog = Self {
            root,
            ..Default::default()
        };
        catalog.scan_names_only();
        catalog
    }

    /// Directory listing only — no image decode.
    fn scan_names_only(&mut self) {
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
            self.emotes = names
                .into_iter()
                .map(|name| EmoteEntry {
                    webp: emote_dir.join(format!("{name}.webp")),
                    png: emote_dir.join(format!("{name}.png")),
                    ogg: emote_dir.join(format!("{name}.ogg")),
                    name,
                })
                .collect();
        }

        self.textures = list_files(self.root.join("textures"), &["png"]);
        self.sounds = list_files(self.root.join("sounds"), &["wav", "ogg"]);
    }

    pub fn root(&self) -> &PathBuf {
        &self.root
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

    pub fn emote_names(&self) -> impl Iterator<Item = &str> {
        self.emotes.iter().map(|e| e.name.as_str())
    }

    pub fn emote(&self, id: AssetId) -> Option<&EmoteEntry> {
        match id {
            AssetId::FirstEmote => self.emotes.first(),
            AssetId::Named(n) => self.emotes.iter().find(|e| e.name == n),
        }
    }

    pub fn emote_animation_path(&self, id: AssetId) -> Option<PathBuf> {
        self.emote(id).map(|e| e.webp.clone())
    }

    /// Cheap wheel icon (small PNG) — preferred for lists.
    pub fn load_emote_png_thumb(&self, id: AssetId) -> Result<RgbaImage> {
        let entry = self.emote(id).ok_or_else(|| anyhow!("unknown emote"))?;
        let img = image::open(&entry.png)
            .with_context(|| format!("open {}", entry.png.display()))?;
        Ok(img.to_rgba8())
    }

    /// Decode **only the first frame** of the animated WebP (on demand).
    pub fn load_emote_webp_first_frame(&mut self, id: AssetId) -> Result<&RgbaImage> {
        let entry = self
            .emote(id)
            .ok_or_else(|| anyhow!("unknown emote"))?
            .clone();

        if self
            .cached_frame
            .as_ref()
            .is_some_and(|(name, _)| name == &entry.name)
        {
            return Ok(&self.cached_frame.as_ref().unwrap().1);
        }

        let img = image::open(&entry.webp)
            .with_context(|| format!("open {}", entry.webp.display()))?
            .to_rgba8();
        log::info!(
            "decoded first WebP frame for '{}' ({}×{}, {} bytes raw RGBA)",
            entry.name,
            img.width(),
            img.height(),
            img.len()
        );
        self.cached_frame = Some((entry.name, img));
        Ok(&self.cached_frame.as_ref().unwrap().1)
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
        PathBuf::from("../../assets/original"),
    ];
    for c in candidates {
        if c.join("emotes").is_dir() {
            return Some(c);
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            for rel in ["assets/original", "../../../assets/original"] {
                let c = dir.join(rel);
                if c.join("emotes").is_dir() {
                    return Some(c);
                }
            }
        }
    }
    None
}
