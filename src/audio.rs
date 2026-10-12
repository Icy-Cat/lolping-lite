//! Audio playback stub.
//!
//! `rodio` was evaluated but pulled `encoding_rs` needing rustc ≥ 1.88 while
//! this project targets stable 1.85 on the build box. TODO: re-enable rodio
//! (or `kira` / `cpal`+`hound`/`lewton`) once the toolchain allows, and play
//! the path logged below.

use std::path::{Path, PathBuf};

#[derive(Debug, Default)]
pub struct AudioOut {
    enabled: bool,
}

impl AudioOut {
    pub fn new() -> Self {
        Self { enabled: true }
    }

    pub fn play_file(&mut self, path: &Path) {
        if !self.enabled {
            return;
        }
        if path.exists() {
            log::info!("audio stub: would play {}", path.display());
        } else {
            log::debug!("audio stub: missing {}", path.display());
        }
    }

    pub fn play_optional(&mut self, path: Option<PathBuf>) {
        if let Some(p) = path {
            self.play_file(&p);
        }
    }
}
