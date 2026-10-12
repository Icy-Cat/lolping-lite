//! Remote asset hosting (Cloudflare R2 CDN) with local cache.
//!
//! Lookup order for a relative asset path (relative to `assets/original`):
//! 1. local bundle (`assets/original` in dev, `.app/Contents/Resources/assets/original`)
//!    — skipped when `LOLPING_NO_LOCAL_ASSETS=1`
//! 2. user cache dir (`<cache>/lolping-lite/v1/<rel>`)
//! 3. background download from `$LOLPING_ASSET_BASE` (default CDN), sha256-verified
//!    against the manifest, written atomically into the cache.
//!
//! Nothing here blocks the UI thread unless a `*_blocking` fn is called explicitly.

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::time::{Duration, SystemTime};

pub const DEFAULT_BASE: &str = "https://lolping-assets.icy-cat.com/v1";
pub const ZIP_NAME: &str = "lolping-assets-v1.zip";
const MANIFEST_MAX_AGE: Duration = Duration::from_secs(24 * 3600);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ManifestFile {
    pub path: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Manifest {
    pub version: u32,
    pub files: Vec<ManifestFile>,
}

impl Manifest {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let m: Manifest = serde_json::from_slice(bytes).context("manifest json")?;
        if m.version != 1 {
            bail!("unsupported manifest version {}", m.version);
        }
        for f in &m.files {
            safe_rel(&f.path)?;
        }
        Ok(m)
    }
    pub fn get(&self, rel: &str) -> Option<&ManifestFile> {
        self.files.iter().find(|f| f.path == rel)
    }
    /// Emote names (stems of `emotes/*.webp`), sorted.
    pub fn emote_names(&self) -> Vec<String> {
        let mut v: Vec<String> = self
            .files
            .iter()
            .filter_map(|f| f.path.strip_prefix("emotes/")?.strip_suffix(".webp").map(String::from))
            .filter(|n| !n.contains('/'))
            .collect();
        v.sort();
        v
    }
}

/// Reject absolute / parent-escaping paths (manifest and zip entries are untrusted).
pub fn safe_rel(rel: &str) -> Result<&str> {
    if rel.is_empty()
        || rel.starts_with('/')
        || rel.contains('\\')
        || rel.split('/').any(|c| c == ".." || c.is_empty() || c.contains(':'))
    {
        bail!("unsafe asset path {rel:?}");
    }
    Ok(rel)
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

pub fn verify(bytes: &[u8], expect: &ManifestFile) -> Result<()> {
    if bytes.len() as u64 != expect.size {
        bail!("{}: size {} != {}", expect.path, bytes.len(), expect.size);
    }
    let got = sha256_hex(bytes);
    if !got.eq_ignore_ascii_case(&expect.sha256) {
        bail!("{}: sha256 mismatch", expect.path);
    }
    Ok(())
}

pub fn base_url() -> String {
    std::env::var("LOLPING_ASSET_BASE")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_BASE.to_string())
        .trim_end_matches('/')
        .to_string()
}

/// Platform cache root without pulling in `dirs`.
pub fn platform_cache_root() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("LOLPING_CACHE_DIR") {
        if !p.is_empty() {
            return Some(PathBuf::from(p));
        }
    }
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Caches"))
    }
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
    }
}

pub fn cache_dir_under(root: &Path) -> PathBuf {
    root.join("lolping-lite").join("v1")
}

pub fn local_assets_disabled() -> bool {
    matches!(std::env::var("LOLPING_NO_LOCAL_ASSETS").as_deref(), Ok("1") | Ok("true"))
}

pub fn find_local_root() -> Option<PathBuf> {
    if local_assets_disabled() {
        return None;
    }
    let mut c: Vec<PathBuf> = ["assets/original", "../assets/original", "../../assets/original"]
        .iter()
        .map(PathBuf::from)
        .collect();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            c.push(dir.join("assets/original"));
            c.push(dir.join("../Resources/assets/original")); // .app bundle
            c.push(dir.join("../../../assets/original"));
        }
    }
    c.into_iter().find(|p| p.join("emotes").is_dir())
}

/// Write via temp file + rename so readers never see partial files.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let dir = path.parent().ok_or_else(|| anyhow!("no parent"))?;
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join(format!(
        ".{}.{}.tmp",
        path.file_name().and_then(|s| s.to_str()).unwrap_or("f"),
        std::process::id() ^ rand::random::<u32>()
    ));
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all().ok();
    }
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })?;
    Ok(())
}

pub fn dir_size(dir: &Path) -> u64 {
    let Ok(rd) = std::fs::read_dir(dir) else { return 0 };
    rd.flatten()
        .map(|e| match e.file_type() {
            Ok(t) if t.is_dir() => dir_size(&e.path()),
            Ok(_) => e.metadata().map(|m| m.len()).unwrap_or(0),
            Err(_) => 0,
        })
        .sum()
}

fn agent() -> &'static ureq::Agent {
    static A: OnceLock<ureq::Agent> = OnceLock::new();
    A.get_or_init(|| {
        let mut b = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(8))
            .timeout_read(Duration::from_secs(30))
            .user_agent(concat!("lolping-lite/", env!("CARGO_PKG_VERSION")));
        if let Ok(tls) = native_tls::TlsConnector::new() {
            b = b.tls_connector(Arc::new(tls));
        }
        b.build()
    })
}

fn http_get(url: &str, max: u64, progress: Option<&AtomicU64>) -> Result<Vec<u8>> {
    let resp = agent().get(url).call().map_err(|e| anyhow!("GET {url}: {e}"))?;
    let mut r = resp.into_reader().take(max + 1);
    let mut out = Vec::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = r.read(&mut buf)?;
        if n == 0 {
            break;
        }
        out.extend_from_slice(&buf[..n]);
        if let Some(p) = progress {
            p.store(out.len() as u64, Ordering::Relaxed);
        }
    }
    if out.len() as u64 > max {
        bail!("GET {url}: response too large");
    }
    Ok(out)
}

#[derive(Debug, Clone, Default)]
pub enum BulkState {
    #[default]
    Idle,
    Running,
    Done(usize),
    Failed(String),
}

/// Process-wide asset store.
#[derive(Debug)]
pub struct AssetStore {
    pub local_root: Option<PathBuf>,
    pub cache_dir: Option<PathBuf>,
    pub base: String,
    manifest: RwLock<Option<Arc<Manifest>>>,
    manifest_fetching: AtomicBool,
    pending: Mutex<HashSet<String>>,
    failed: Mutex<HashMap<String, SystemTime>>,
    pub bulk_done: AtomicU64,
    pub bulk_total: AtomicU64,
    pub bulk_state: Mutex<BulkState>,
    /// Bumps whenever a new file lands in the cache (UI can poll this).
    pub generation: AtomicU64,
}

impl AssetStore {
    pub fn new(local_root: Option<PathBuf>, cache_dir: Option<PathBuf>, base: String) -> Self {
        let s = Self {
            local_root,
            cache_dir,
            base,
            manifest: RwLock::new(None),
            manifest_fetching: AtomicBool::new(false),
            pending: Mutex::default(),
            failed: Mutex::default(),
            bulk_done: AtomicU64::new(0),
            bulk_total: AtomicU64::new(0),
            bulk_state: Mutex::default(),
            generation: AtomicU64::new(0),
        };
        if let Some(m) = s.cached_manifest_path().and_then(|p| std::fs::read(p).ok()) {
            if let Ok(m) = Manifest::parse(&m) {
                *s.manifest.write().unwrap() = Some(Arc::new(m));
            }
        }
        s
    }

    pub fn global() -> &'static Arc<AssetStore> {
        static G: OnceLock<Arc<AssetStore>> = OnceLock::new();
        G.get_or_init(|| {
            Arc::new(AssetStore::new(
                find_local_root(),
                platform_cache_root().map(|r| cache_dir_under(&r)),
                base_url(),
            ))
        })
    }

    fn cached_manifest_path(&self) -> Option<PathBuf> {
        self.cache_dir.as_ref().map(|d| d.join("manifest.json"))
    }

    pub fn manifest(&self) -> Option<Arc<Manifest>> {
        self.manifest.read().unwrap().clone()
    }

    pub fn manifest_is_stale(&self) -> bool {
        let Some(p) = self.cached_manifest_path() else { return true };
        match std::fs::metadata(p).and_then(|m| m.modified()) {
            Ok(t) => SystemTime::now().duration_since(t).unwrap_or_default() > MANIFEST_MAX_AGE,
            Err(_) => true,
        }
    }

    pub fn refresh_manifest_blocking(&self) -> Result<Arc<Manifest>> {
        let bytes = http_get(&format!("{}/manifest.json", self.base), 8 << 20, None)?;
        let m = Arc::new(Manifest::parse(&bytes)?);
        if let Some(p) = self.cached_manifest_path() {
            let _ = write_atomic(&p, &bytes);
        }
        *self.manifest.write().unwrap() = Some(m.clone());
        self.generation.fetch_add(1, Ordering::Relaxed);
        Ok(m)
    }

    /// Ensure a manifest is (or will be) available. Non-blocking.
    pub fn refresh_manifest_bg(self: &Arc<Self>, force: bool) {
        if !force && self.manifest().is_some() && !self.manifest_is_stale() {
            return;
        }
        if self.manifest_fetching.swap(true, Ordering::SeqCst) {
            return;
        }
        let me = self.clone();
        std::thread::spawn(move || {
            if let Err(e) = me.refresh_manifest_blocking() {
                log::warn!("manifest refresh failed (using cache if any): {e:#}");
            }
            me.manifest_fetching.store(false, Ordering::SeqCst);
        });
    }

    fn manifest_or_fetch_blocking(&self) -> Result<Arc<Manifest>> {
        match self.manifest() {
            Some(m) => Ok(m),
            None => self.refresh_manifest_blocking(),
        }
    }

    pub fn cache_path(&self, rel: &str) -> Option<PathBuf> {
        let rel = safe_rel(rel).ok()?;
        self.cache_dir.as_ref().map(|d| d.join(rel))
    }

    /// Local or cached path, if present right now. Never touches the network.
    pub fn lookup(&self, rel: &str) -> Option<PathBuf> {
        if let Some(root) = &self.local_root {
            let p = root.join(rel);
            if p.is_file() {
                return Some(p);
            }
        }
        let p = self.cache_path(rel)?;
        if !p.is_file() {
            return None;
        }
        // Cheap integrity check: size must match the manifest when known.
        if let Some(m) = self.manifest() {
            if let Some(f) = m.get(rel) {
                if std::fs::metadata(&p).map(|m| m.len()).ok() != Some(f.size) {
                    return None;
                }
            }
        }
        Some(p)
    }

    /// Non-blocking resolve: returns a path if available, otherwise schedules a
    /// background download and returns `None` (caller shows placeholder / skips).
    pub fn resolve(self: &Arc<Self>, rel: &str) -> Option<PathBuf> {
        if let Some(p) = self.lookup(rel) {
            return Some(p);
        }
        self.request(rel);
        None
    }

    /// Schedule background download (deduplicated, 30 s backoff after failure).
    pub fn request(self: &Arc<Self>, rel: &str) {
        if self.cache_dir.is_none() || safe_rel(rel).is_err() {
            return;
        }
        if let Some(t) = self.failed.lock().unwrap().get(rel) {
            if t.elapsed().unwrap_or_default() < Duration::from_secs(30) {
                return;
            }
        }
        if !self.pending.lock().unwrap().insert(rel.to_string()) {
            return;
        }
        let me = self.clone();
        let rel = rel.to_string();
        std::thread::spawn(move || {
            let r = me.fetch_blocking(&rel);
            me.pending.lock().unwrap().remove(&rel);
            match r {
                Ok(_) => {
                    me.failed.lock().unwrap().remove(&rel);
                }
                Err(e) => {
                    log::warn!("asset fetch {rel} failed: {e:#}");
                    me.failed.lock().unwrap().insert(rel, SystemTime::now());
                }
            }
        });
    }

    pub fn request_many<'a>(self: &Arc<Self>, rels: impl IntoIterator<Item = &'a str>) {
        let rels: Vec<String> = rels
            .into_iter()
            .filter(|r| self.lookup(r).is_none())
            .map(String::from)
            .collect();
        if rels.is_empty() {
            return;
        }
        let me = self.clone();
        // One worker for prefetch so we don't spawn dozens of threads.
        std::thread::spawn(move || {
            for r in rels {
                if me.lookup(&r).is_none() {
                    if let Err(e) = me.fetch_blocking(&r) {
                        log::warn!("prefetch {r}: {e:#}");
                    }
                }
            }
        });
    }

    pub fn is_pending(&self, rel: &str) -> bool {
        self.pending.lock().unwrap().contains(rel)
    }

    /// Download + verify + atomically cache one file. Blocking.
    pub fn fetch_blocking(&self, rel: &str) -> Result<PathBuf> {
        if let Some(p) = self.lookup(rel) {
            return Ok(p);
        }
        let dest = self.cache_path(rel).ok_or_else(|| anyhow!("no cache dir"))?;
        let m = self.manifest_or_fetch_blocking()?;
        let entry = m.get(rel).ok_or_else(|| anyhow!("{rel} not in manifest"))?.clone();
        let bytes = http_get(&format!("{}/{}", self.base, rel), entry.size, None)?;
        verify(&bytes, &entry)?;
        write_atomic(&dest, &bytes)?;
        self.generation.fetch_add(1, Ordering::Relaxed);
        log::info!("cached {rel} ({} bytes)", bytes.len());
        Ok(dest)
    }

    /// Download the full zip, verify every entry against the manifest, extract to cache.
    pub fn download_all_blocking(&self) -> Result<usize> {
        let dir = self.cache_dir.clone().ok_or_else(|| anyhow!("no cache dir"))?;
        let m = self.refresh_manifest_blocking().or_else(|e| {
            self.manifest().ok_or(e)
        })?;
        let total: u64 = m.files.iter().map(|f| f.size).sum();
        self.bulk_total.store(total, Ordering::Relaxed);
        self.bulk_done.store(0, Ordering::Relaxed);
        let bytes = http_get(
            &format!("{}/{}", self.base, ZIP_NAME),
            total * 2 + (16 << 20),
            Some(&self.bulk_done),
        )?;
        let n = extract_zip_verified(&bytes, &m, &dir)?;
        self.generation.fetch_add(1, Ordering::Relaxed);
        Ok(n)
    }

    pub fn download_all_bg(self: &Arc<Self>) {
        {
            let mut st = self.bulk_state.lock().unwrap();
            if matches!(*st, BulkState::Running) {
                return;
            }
            *st = BulkState::Running;
        }
        let me = self.clone();
        std::thread::spawn(move || {
            let r = me.download_all_blocking();
            *me.bulk_state.lock().unwrap() = match r {
                Ok(n) => BulkState::Done(n),
                Err(e) => BulkState::Failed(format!("{e:#}")),
            };
        });
    }

    pub fn cache_size(&self) -> u64 {
        self.cache_dir.as_deref().map(dir_size).unwrap_or(0)
    }

    /// Delete cached asset files (keeps manifest so offline lookups still know names).
    pub fn clear_cache(&self) -> Result<()> {
        let Some(dir) = &self.cache_dir else { return Ok(()) };
        if let Ok(rd) = std::fs::read_dir(dir) {
            for e in rd.flatten() {
                let p = e.path();
                if p.file_name().and_then(|s| s.to_str()) == Some("manifest.json") {
                    continue;
                }
                if p.is_dir() {
                    std::fs::remove_dir_all(&p)?;
                } else {
                    std::fs::remove_file(&p)?;
                }
            }
        }
        self.generation.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
}

/// Extract entries listed in the manifest (others ignored), verifying each.
pub fn extract_zip_verified(zip_bytes: &[u8], m: &Manifest, dest: &Path) -> Result<usize> {
    let mut za = zip::ZipArchive::new(std::io::Cursor::new(zip_bytes)).context("open zip")?;
    let mut n = 0;
    for i in 0..za.len() {
        let mut f = za.by_index(i)?;
        if f.is_dir() {
            continue;
        }
        let name = f.name().trim_start_matches("./").to_string();
        let name = name.strip_prefix("original/").unwrap_or(&name).to_string();
        let Some(entry) = m.get(&name) else { continue };
        let mut buf = Vec::with_capacity(entry.size as usize);
        (&mut f).take(entry.size + 1).read_to_end(&mut buf)?;
        verify(&buf, entry)?;
        write_atomic(&dest.join(safe_rel(&entry.path)?), &buf)?;
        n += 1;
    }
    if n == 0 {
        bail!("zip contained no manifest files");
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("lolping-test-{tag}-{}", rand::random::<u64>()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn manifest_parse_and_names() {
        let j = br#"{"version":1,"files":[
            {"path":"emotes/b.webp","size":3,"sha256":"x"},
            {"path":"emotes/a.webp","size":3,"sha256":"x"},
            {"path":"emotes/a.ogg","size":3,"sha256":"x"},
            {"path":"sounds/s.wav","size":1,"sha256":"x"}]}"#;
        let m = Manifest::parse(j).unwrap();
        assert_eq!(m.files.len(), 4);
        assert_eq!(m.emote_names(), vec!["a", "b"]);
        assert!(m.get("sounds/s.wav").is_some());
        assert!(Manifest::parse(br#"{"version":2,"files":[]}"#).is_err());
        assert!(Manifest::parse(br#"{"version":1,"files":[{"path":"../x","size":1,"sha256":""}]}"#).is_err());
    }

    #[test]
    fn sha_verify() {
        let data = b"hello";
        let f = ManifestFile {
            path: "x".into(),
            size: 5,
            sha256: "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824".into(),
        };
        verify(data, &f).unwrap();
        assert!(verify(b"hellO", &f).is_err());
        assert!(verify(b"hell", &f).is_err());
    }

    #[test]
    fn cache_paths_and_safety() {
        let root = PathBuf::from("/tmp/c");
        assert_eq!(cache_dir_under(&root), PathBuf::from("/tmp/c/lolping-lite/v1"));
        let s = AssetStore::new(None, Some(cache_dir_under(&root)), "http://x".into());
        assert_eq!(
            s.cache_path("emotes/a.webp"),
            Some(PathBuf::from("/tmp/c/lolping-lite/v1/emotes/a.webp"))
        );
        assert!(s.cache_path("../etc/passwd").is_none());
        assert!(s.cache_path("/abs").is_none());
        assert!(s.cache_path("a//b").is_none());
    }

    #[test]
    fn atomic_write_lookup_size_clear() {
        let d = tmpdir("cache");
        let s = AssetStore::new(None, Some(d.clone()), "http://127.0.0.1:9".into());
        assert!(s.lookup("sounds/x.wav").is_none());
        write_atomic(&d.join("sounds/x.wav"), b"abc").unwrap();
        write_atomic(&d.join("manifest.json"), b"{}").unwrap();
        assert!(s.lookup("sounds/x.wav").is_some());
        assert_eq!(s.cache_size(), 5);
        s.clear_cache().unwrap();
        assert!(s.lookup("sounds/x.wav").is_none());
        assert!(d.join("manifest.json").exists());
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn zip_extract_verifies() {
        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut w = zip::ZipWriter::new(&mut buf);
            let o = zip::write::FileOptions::default();
            w.start_file("emotes/a.webp", o).unwrap();
            w.write_all(b"hello").unwrap();
            w.start_file("../evil", o).unwrap();
            w.write_all(b"x").unwrap();
            w.finish().unwrap();
        }
        let m = Manifest {
            version: 1,
            files: vec![ManifestFile {
                path: "emotes/a.webp".into(),
                size: 5,
                sha256: sha256_hex(b"hello"),
            }],
        };
        let d = tmpdir("zip");
        assert_eq!(extract_zip_verified(buf.get_ref(), &m, &d).unwrap(), 1);
        assert_eq!(std::fs::read(d.join("emotes/a.webp")).unwrap(), b"hello");
        let mut bad = m.clone();
        bad.files[0].sha256 = sha256_hex(b"nope");
        assert!(extract_zip_verified(buf.get_ref(), &bad, &d).is_err());
        let _ = std::fs::remove_dir_all(d);
    }
}
