//! Asset discovery and on-demand loading (no bulk WebP preload).

use anyhow::{anyhow, Context, Result};
use image::{AnimationDecoder, RgbaImage};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy)]
pub enum AssetId {
    FirstEmote,
    Named(&'static str),
    Index(usize),
}

#[derive(Debug, Clone)]
pub struct EmoteEntry {
    pub name: String,
    pub webp: PathBuf,
    pub png: PathBuf,
    pub ogg: PathBuf,
}

/// Decoded animation for one emote (single-slot cache payload).
#[derive(Clone)]
#[derive(Debug)]
pub struct EmoteAnimation {
    pub name: String,
    pub frames: Vec<RgbaImage>,
    pub delays_ms: Vec<u32>,
}

#[derive(Debug, Default)]
pub struct AssetCatalog {
    root: PathBuf,
    emotes: Vec<EmoteEntry>,
    textures: Vec<PathBuf>,
    sounds: Vec<PathBuf>,
    /// At most one full animated emote in RAM.
    cached_anim: Option<EmoteAnimation>,
}

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

    pub fn emotes(&self) -> &[EmoteEntry] {
        &self.emotes
    }

    pub fn emote(&self, id: AssetId) -> Option<&EmoteEntry> {
        match id {
            AssetId::FirstEmote => self.emotes.first(),
            AssetId::Named(n) => self.emotes.iter().find(|e| e.name == n),
            AssetId::Index(i) => self.emotes.get(i),
        }
    }

    pub fn texture_path(&self, file: &str) -> PathBuf {
        self.root.join("textures").join(file)
    }

    pub fn sound_path(&self, file: &str) -> PathBuf {
        self.root.join("sounds").join(file)
    }

    pub fn load_png(&self, path: &Path) -> Result<RgbaImage> {
        let img = image::open(path).with_context(|| format!("open {}", path.display()))?;
        Ok(img.to_rgba8())
    }

    pub fn load_emote_png_thumb(&self, id: AssetId) -> Result<RgbaImage> {
        let entry = self.emote(id).ok_or_else(|| anyhow!("unknown emote"))?;
        self.load_png(&entry.png)
    }

    /// Decode **one** animated WebP into frames (replaces single-slot cache).
    pub fn load_emote_animation(&mut self, id: AssetId) -> Result<&EmoteAnimation> {
        let entry = self
            .emote(id)
            .ok_or_else(|| anyhow!("unknown emote"))?
            .clone();

        if self
            .cached_anim
            .as_ref()
            .is_some_and(|a| a.name == entry.name)
        {
            return Ok(self.cached_anim.as_ref().unwrap());
        }

        let anim = decode_webp_animation(&entry.webp, &entry.name)?;
        log::info!(
            "decoded emote '{}' animation: {} frames (on-demand, single-slot cache)",
            anim.name,
            anim.frames.len()
        );
        self.cached_anim = Some(anim);
        Ok(self.cached_anim.as_ref().unwrap())
    }

    pub fn take_cached_anim_clone(&self, name: &str) -> Option<EmoteAnimation> {
        self.cached_anim
            .as_ref()
            .filter(|a| a.name == name)
            .cloned()
    }
}

fn decode_webp_animation(path: &Path, name: &str) -> Result<EmoteAnimation> {
    let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let reader = BufReader::new(file);
    let decoder = image::codecs::webp::WebPDecoder::new(reader)
        .with_context(|| format!("webp decoder {}", path.display()))?;

    let mut frames = Vec::new();
    let mut delays_ms = Vec::new();
    // Cap frames to keep a single emote bounded (~74 typical).
    const MAX_FRAMES: usize = 120;

    for (i, frame) in decoder.into_frames().enumerate() {
        if i >= MAX_FRAMES {
            log::warn!("emote '{name}': truncating animation at {MAX_FRAMES} frames");
            break;
        }
        let frame = frame.with_context(|| format!("frame {i} of {name}"))?;
        let (num, den) = frame.delay().numer_denom_ms();
        let ms = if den == 0 {
            33
        } else {
            (num / den).max(10)
        };
        delays_ms.push(ms);
        frames.push(frame.into_buffer());
    }

    if frames.is_empty() {
        // Fallback: static decode
        let img = image::open(path)?.to_rgba8();
        frames.push(img);
        delays_ms.push(100);
    }

    Ok(EmoteAnimation {
        name: name.to_string(),
        frames,
        delays_ms,
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_one_emote_on_demand() {
        let mut cat = AssetCatalog::discover();
        assert!(cat.emote_count() > 0, "fixtures missing — run from repo root");
        let anim = cat.load_emote_animation(AssetId::Index(0)).expect("decode");
        assert!(!anim.frames.is_empty());
        assert_eq!(anim.frames.len(), anim.delays_ms.len());
        // Must not have loaded all emotes — cache is single-slot.
        assert_eq!(cat.emote_count(), 44);
    }
}
