//! Asset discovery and on-demand loading (no bulk WebP preload).

use anyhow::{anyhow, Context, Result};
use image::{AnimationDecoder, RgbaImage};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use crate::remote::AssetStore;

#[derive(Debug, Clone, Copy)]
pub enum AssetId {
    FirstEmote,
    Named(&'static str),
    Index(usize),
}

#[derive(Debug, Clone)]
pub struct EmoteEntry {
    pub name: String,
    /// Paths relative to the asset root (`emotes/<name>.webp` …).
    pub webp: String,
    pub png: String,
    pub ogg: String,
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
    store: Option<Arc<AssetStore>>,
    /// v2 store (WebM/VP9 emotes) if this build can decode them.
    v2: Option<Arc<AssetStore>>,
    /// Emotes whose v2 WebM failed to decode — use v1 WebP for them.
    v2_bad: std::collections::HashSet<String>,
    emotes: Vec<EmoteEntry>,
    textures: Vec<PathBuf>,
    sounds: Vec<PathBuf>,
    /// At most one full animated emote in RAM.
    cached_anim: Option<EmoteAnimation>,
}

impl AssetCatalog {
    /// Local bundle if present, else cache + CDN via the global [`AssetStore`].
    pub fn discover() -> Self {
        let store = AssetStore::global().clone();
        store.refresh_manifest_bg(false);
        let v2 = AssetStore::global_v2().cloned();
        if let Some(d) = v2.as_ref().and_then(|v| v.cache_dir.clone()) {
            store.extra_dirs.lock().unwrap().push(d);
        }
        let mut c = Self::with_store(store);
        c.v2 = v2;
        c
    }

    pub fn v2_store(&self) -> Option<&Arc<AssetStore>> {
        self.v2.as_ref()
    }

    pub fn with_stores(store: Arc<AssetStore>, v2: Option<Arc<AssetStore>>) -> Self {
        let mut c = Self::with_store(store);
        c.v2 = v2;
        c
    }

    pub fn v2_enabled(&self) -> bool {
        self.v2.is_some()
    }

    /// Background-prefetch what's needed to animate these emotes (v2 WebM if enabled, else WebP).
    pub fn prefetch_animations(&self, names: &[String]) {
        match &self.v2 {
            Some(v2) => v2.request_many(names.iter().map(|n| format!("emotes/{n}.webm")).collect::<Vec<_>>().iter().map(|s| s.as_str())),
            None => self.prefetch(&names.iter().map(|n| format!("emotes/{n}.webp")).collect::<Vec<_>>()),
        }
    }

    /// Pick the animation source for an emote without blocking.
    /// Ok(Some(path, is_webm)) ready; Ok(None) downloading.
    fn anim_source(&self, entry: &EmoteEntry) -> Option<(PathBuf, bool)> {
        if let Some(v2) = self.v2.as_ref().filter(|_| !self.v2_bad.contains(&entry.name)) {
            let rel = format!("emotes/{}.webm", entry.name);
            if let Some(p) = v2.lookup(&rel) {
                return Some((p, true));
            }
            // Local v1 bundle present: use it now, upgrade to v2 in the background.
            if let Some(p) = self.store.as_ref().and_then(|s| s.local_root.as_ref()).map(|r| r.join(&entry.webp)).filter(|p| p.is_file()) {
                v2.request(&rel);
                return Some((p, false));
            }
            if !v2.has_failed(&rel) {
                v2.request(&rel);
                return None;
            }
            // v2 unavailable → fall through to v1.
        }
        self.resolve(&entry.webp).map(|p| (p, false))
    }

    pub fn with_store(store: Arc<AssetStore>) -> Self {
        let root = store
            .local_root
            .clone()
            .or_else(|| store.cache_dir.clone())
            .unwrap_or_else(|| PathBuf::from("assets/original"));
        let mut catalog = Self {
            root,
            store: Some(store),
            ..Default::default()
        };
        catalog.rescan();
        catalog
    }

    /// Re-read emote names (e.g. once the manifest arrives). Returns true if changed.
    pub fn rescan(&mut self) -> bool {
        let before = self.emotes.len();
        let mut names = Vec::new();
        if let Some(local) = self.store.as_ref().and_then(|s| s.local_root.clone()) {
            for p in list_files(local.join("emotes"), &["webp"]) {
                if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                    names.push(stem.to_string());
                }
            }
            self.textures = list_files(local.join("textures"), &["png"]);
            self.sounds = list_files(local.join("sounds"), &["wav", "ogg"]);
        } else if let Some(m) = self.store.as_ref().and_then(|s| s.manifest()) {
            names = m.emote_names();
            let pick = |pre: &str| -> Vec<PathBuf> {
                m.files.iter().filter(|f| f.path.starts_with(pre)).map(|f| PathBuf::from(&f.path)).collect()
            };
            self.textures = pick("textures/");
            self.sounds = pick("sounds/");
        }
        names.sort();
        self.emotes = names
            .into_iter()
            .map(|name| EmoteEntry {
                webp: format!("emotes/{name}.webp"),
                png: format!("emotes/{name}.png"),
                ogg: format!("emotes/{name}.ogg"),
                name,
            })
            .collect();
        self.emotes.len() != before
    }

    /// Non-blocking: path if available now, else schedules a download and returns None.
    pub fn resolve(&self, rel: &str) -> Option<PathBuf> {
        match &self.store {
            Some(s) => s.resolve(rel),
            None => Some(self.root.join(rel)).filter(|p| p.is_file()),
        }
    }

    pub fn store(&self) -> Option<&Arc<AssetStore>> {
        self.store.as_ref()
    }

    /// Background prefetch of the given relative paths.
    pub fn prefetch(&self, rels: &[String]) {
        if let Some(s) = &self.store {
            s.request_many(rels.iter().map(|s| s.as_str()));
        }
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

    /// Resolved texture path, or None while it downloads.
    pub fn texture_path(&self, file: &str) -> Option<PathBuf> {
        self.resolve(&format!("textures/{file}"))
    }

    /// Resolved sound path, or None while it downloads.
    pub fn sound_path(&self, file: &str) -> Option<PathBuf> {
        self.resolve(&format!("sounds/{file}"))
    }

    pub fn load_png(&self, path: &Path) -> Result<RgbaImage> {
        let img = image::open(path).with_context(|| format!("open {}", path.display()))?;
        Ok(img.to_rgba8())
    }

    pub fn load_emote_png_thumb(&self, id: AssetId) -> Result<RgbaImage> {
        let entry = self.emote(id).ok_or_else(|| anyhow!("unknown emote"))?;
        let p = self.resolve(&entry.png).ok_or_else(|| anyhow!("{} downloading", entry.png))?;
        self.load_png(&p)
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

        let (path, is_webm) = self
            .anim_source(&entry)
            .ok_or_else(|| anyhow!("{} not ready (downloading)", entry.name))?;
        let anim = if is_webm {
            match decode_webm_animation(&path, &entry.name) {
                Ok(a) => a,
                Err(e) => {
                    log::warn!("v2 webm '{}' failed ({e:#}); falling back to v1 webp", entry.name);
                    self.v2_bad.insert(entry.name.clone());
                    let p = self.resolve(&entry.webp).ok_or_else(|| anyhow!("{} downloading", entry.webp))?;
                    decode_webp_animation(&p, &entry.name)?
                }
            }
        } else {
            decode_webp_animation(&path, &entry.name)?
        };
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

pub fn decode_webm_animation(path: &Path, name: &str) -> Result<EmoteAnimation> {
    let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let f = crate::webm::parse(&bytes)?;
    const MAX_FRAMES: usize = 120;
    let frames = crate::webm::decode_rgba_frames(&f, MAX_FRAMES)?;
    let mut delays_ms = crate::webm::frame_delays(&f);
    delays_ms.truncate(frames.len());
    Ok(EmoteAnimation { name: name.to_string(), frames, delays_ms })
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
