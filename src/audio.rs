//! Audio via rodio (wav/vorbis) with ffmpeg fallback for Opus-in-Ogg emote sounds.

use anyhow::{anyhow, Context, Result};
use rodio::buffer::SamplesBuffer;
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{self, Sender};
use std::thread;

enum Cmd {
    Play(PathBuf, f32),
}

pub struct AudioOut {
    tx: Option<Sender<Cmd>>,
    pub last_played: Option<PathBuf>,
    pub play_count: u64,
    volume: f32,
    muted: bool,
}

impl Default for AudioOut {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioOut {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel::<Cmd>();
        thread::Builder::new()
            .name("lolping-audio".into())
            .spawn(move || {
                let (_stream, handle) = match OutputStream::try_default() {
                    Ok(v) => v,
                    Err(e) => {
                        log::warn!("audio: no output device ({e}); decode-only");
                        while let Ok(Cmd::Play(path, _)) = rx.recv() {
                            let _ = decode_file(&path);
                        }
                        return;
                    }
                };
                while let Ok(Cmd::Play(path, vol)) = rx.recv() {
                    if let Err(e) = play_path(&handle, &path, vol) {
                        log::warn!("audio play {}: {e:#}", path.display());
                    }
                }
            })
            .ok();
        Self {
            tx: Some(tx),
            last_played: None,
            play_count: 0,
            volume: 0.7,
            muted: false,
        }
    }

    pub fn set_volume(&mut self, volume_0_100: u8) {
        self.volume = (volume_0_100 as f32 / 100.0).clamp(0.0, 1.0);
    }

    pub fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
    }

    pub fn play_file(&mut self, path: &Path) {
        self.last_played = Some(path.to_path_buf());
        self.play_count = self.play_count.saturating_add(1);
        if self.muted {
            return;
        }
        if let Some(tx) = &self.tx {
            let _ = tx.send(Cmd::Play(path.to_path_buf(), self.volume));
        }
    }

    pub fn play_optional(&mut self, path: Option<PathBuf>) {
        if let Some(p) = path {
            self.play_file(&p);
        }
    }
}

fn play_path(handle: &OutputStreamHandle, path: &Path, vol: f32) -> Result<()> {
    let sink = Sink::try_new(handle)?;
    sink.set_volume(vol);
    if let Ok(file) = File::open(path) {
        if let Ok(decoder) = Decoder::new(BufReader::new(file)) {
            sink.append(decoder);
            return Ok(());
        }
    }
    // Opus-in-Ogg (LoL emotes) and other formats: ffmpeg → f32le PCM
    let buf = decode_via_ffmpeg(path)?;
    sink.append(buf);
    Ok(())
}

fn decode_via_ffmpeg(path: &Path) -> Result<SamplesBuffer<f32>> {
    let out = Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-i",
            path.to_str().ok_or_else(|| anyhow!("bad path"))?,
            "-f",
            "f32le",
            "-ac",
            "1",
            "-ar",
            "48000",
            "pipe:1",
        ])
        .output()
        .context("ffmpeg")?;
    if !out.status.success() {
        return Err(anyhow!(
            "ffmpeg failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    let mut samples = Vec::with_capacity(out.stdout.len() / 4);
    for chunk in out.stdout.chunks_exact(4) {
        samples.push(f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }
    if samples.is_empty() {
        return Err(anyhow!("ffmpeg produced no samples"));
    }
    Ok(SamplesBuffer::new(1, 48_000, samples))
}

/// Decode without requiring an output device (tests / simulate).
pub fn decode_file(path: &Path) -> Result<u64> {
    if let Ok(file) = File::open(path) {
        if let Ok(decoder) = Decoder::new(BufReader::new(file)) {
            let mut frames = 0u64;
            for s in decoder {
                let _ = s;
                frames += 1;
                if frames > 50_000 {
                    break;
                }
            }
            if frames > 0 {
                return Ok(frames);
            }
        }
    }
    let buf = decode_via_ffmpeg(path)?;
    Ok(buf.total_duration().map(|d| d.as_millis().max(1) as u64).unwrap_or(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_ping_wav() {
        let p = Path::new("assets/original/sounds/OnMyWay.wav");
        if !p.exists() {
            return;
        }
        assert!(decode_file(p).unwrap() > 0);
    }

    #[test]
    fn decode_emote_ogg_opus() {
        let p = Path::new("assets/original/emotes/angry_kitty.ogg");
        if !p.exists() {
            return;
        }
        assert!(decode_file(p).unwrap() > 0);
    }
}
