//! Minimal WebM (Matroska) demuxer + VP9-with-alpha decoder for v2 emotes.
//!
//! The demuxer only understands what our encoder (ffmpeg/libvpx-vp9, see
//! `scripts/build-v2-assets.sh`) writes: one video track, SimpleBlocks or BlockGroups
//! with the alpha plane in `BlockAdditional` (BlockAddID 1), no lacing.
//! Decoding needs libvpx (static, decoder only) — compiled in when `cfg(has_vp9)`.

use anyhow::{anyhow, bail, Result};

#[derive(Debug, Default, Clone)]
pub struct WebmPacket {
    pub pts_ms: u64,
    pub data: Vec<u8>,
    pub alpha: Option<Vec<u8>>,
}

#[derive(Debug, Default, Clone)]
pub struct WebmFile {
    pub width: u32,
    pub height: u32,
    pub codec: String,
    pub packets: Vec<WebmPacket>,
    /// From the stream tag `LAST_FRAME_MS` (WebM can't express the last frame's duration).
    pub last_frame_ms: Option<u32>,
}

const SEGMENT: u64 = 0x18538067;
const INFO: u64 = 0x1549A966;
const TIMECODE_SCALE: u64 = 0x2AD7B1;
const TRACKS: u64 = 0x1654AE6B;
const TRACK_ENTRY: u64 = 0xAE;
const CODEC_ID: u64 = 0x86;
const VIDEO: u64 = 0xE0;
const PIXEL_W: u64 = 0xB0;
const PIXEL_H: u64 = 0xBA;
const CLUSTER: u64 = 0x1F43B675;
const CLUSTER_TC: u64 = 0xE7;
const SIMPLE_BLOCK: u64 = 0xA3;
const BLOCK_GROUP: u64 = 0xA0;
const BLOCK: u64 = 0xA1;
const BLOCK_ADDITIONS: u64 = 0x75A1;
const BLOCK_MORE: u64 = 0xA6;
const BLOCK_ADD_ID: u64 = 0xEE;
const BLOCK_ADDITIONAL: u64 = 0xA5;
const TAGS: u64 = 0x1254C367;
const TAG: u64 = 0x7373;
const SIMPLE_TAG: u64 = 0x67C8;
const TAG_NAME: u64 = 0x45A3;
const TAG_STRING: u64 = 0x4487;

fn vint(b: &[u8], strip_marker: bool) -> Result<(u64, usize, bool)> {
    let first = *b.first().ok_or_else(|| anyhow!("eof in vint"))?;
    let len = first.leading_zeros() as usize + 1;
    if len > 8 || b.len() < len {
        bail!("bad vint");
    }
    let mut v = if strip_marker { (first as u64) & ((1u64 << (8 - len)) - 1) } else { first as u64 };
    let mut all_ones = strip_marker && v == (1u64 << (8 - len)) - 1;
    for &x in &b[1..len] {
        v = (v << 8) | x as u64;
        all_ones &= x == 0xFF;
    }
    Ok((v, len, all_ones))
}

fn uint(b: &[u8]) -> u64 {
    b.iter().fold(0u64, |a, &x| (a << 8) | x as u64)
}

/// Iterate child elements of a master element body.
fn children<'a>(mut b: &'a [u8], mut f: impl FnMut(u64, &'a [u8]) -> Result<()>) -> Result<()> {
    while !b.is_empty() {
        let (id, il, _) = vint(b, false)?;
        let (size, sl, unknown) = vint(&b[il..], true)?;
        let start = il + sl;
        let end = if unknown { b.len() } else { start.checked_add(size as usize).ok_or_else(|| anyhow!("size"))? };
        if end > b.len() {
            bail!("element 0x{id:X} overruns buffer");
        }
        f(id, &b[start..end])?;
        b = &b[end..];
    }
    Ok(())
}

pub fn parse(bytes: &[u8]) -> Result<WebmFile> {
    let mut out = WebmFile::default();
    let mut scale_ns: u64 = 1_000_000;
    let mut video_track: Option<u64> = None;
    children(bytes, |id, body| {
        if id != SEGMENT {
            return Ok(());
        }
        children(body, |id, body| {
            match id {
                INFO => children(body, |id, b| {
                    if id == TIMECODE_SCALE {
                        scale_ns = uint(b).max(1);
                    }
                    Ok(())
                })?,
                TRACKS => children(body, |id, b| {
                    if id != TRACK_ENTRY {
                        return Ok(());
                    }
                    let (mut num, mut codec, mut is_video, mut w, mut h) = (0, String::new(), false, 0, 0);
                    children(b, |id, b| {
                        match id {
                            0xD7 => num = uint(b),
                            CODEC_ID => codec = String::from_utf8_lossy(b).into_owned(),
                            VIDEO => {
                                is_video = true;
                                children(b, |id, b| {
                                    match id {
                                        PIXEL_W => w = uint(b) as u32,
                                        PIXEL_H => h = uint(b) as u32,
                                        _ => {}
                                    }
                                    Ok(())
                                })?;
                            }
                            _ => {}
                        }
                        Ok(())
                    })?;
                    if is_video && video_track.is_none() {
                        video_track = Some(num);
                        out.codec = codec;
                        out.width = w;
                        out.height = h;
                    }
                    Ok(())
                })?,
                CLUSTER => {
                    let mut ctc = 0u64;
                    children(body, |id, b| {
                        match id {
                            CLUSTER_TC => ctc = uint(b),
                            SIMPLE_BLOCK => push_block(&mut out, video_track, ctc, scale_ns, b, None)?,
                            BLOCK_GROUP => {
                                let mut block: Option<&[u8]> = None;
                                let mut alpha: Option<Vec<u8>> = None;
                                children(b, |id, b| {
                                    match id {
                                        BLOCK => block = Some(b),
                                        BLOCK_ADDITIONS => children(b, |id, b| {
                                            if id == BLOCK_MORE {
                                                let (mut add_id, mut data) = (1u64, None);
                                                children(b, |id, b| {
                                                    match id {
                                                        BLOCK_ADD_ID => add_id = uint(b),
                                                        BLOCK_ADDITIONAL => data = Some(b.to_vec()),
                                                        _ => {}
                                                    }
                                                    Ok(())
                                                })?;
                                                if add_id == 1 {
                                                    alpha = data;
                                                }
                                            }
                                            Ok(())
                                        })?,
                                        _ => {}
                                    }
                                    Ok(())
                                })?;
                                if let Some(blk) = block {
                                    push_block(&mut out, video_track, ctc, scale_ns, blk, alpha)?;
                                }
                            }
                            _ => {}
                        }
                        Ok(())
                    })?;
                }
                TAGS => children(body, |id, b| {
                    if id == TAG {
                        children(b, |id, b| {
                            if id == SIMPLE_TAG {
                                let (mut name, mut val) = (String::new(), String::new());
                                children(b, |id, b| {
                                    match id {
                                        TAG_NAME => name = String::from_utf8_lossy(b).into_owned(),
                                        TAG_STRING => val = String::from_utf8_lossy(b).into_owned(),
                                        _ => {}
                                    }
                                    Ok(())
                                })?;
                                if name.eq_ignore_ascii_case("LAST_FRAME_MS") {
                                    out.last_frame_ms = val.trim().parse().ok();
                                }
                            }
                            Ok(())
                        })?;
                    }
                    Ok(())
                })?,
                _ => {}
            }
            Ok(())
        })
    })?;
    if out.packets.is_empty() {
        bail!("no video packets");
    }
    out.packets.sort_by_key(|p| p.pts_ms);
    Ok(out)
}

fn push_block(
    out: &mut WebmFile,
    track: Option<u64>,
    cluster_tc: u64,
    scale_ns: u64,
    b: &[u8],
    alpha: Option<Vec<u8>>,
) -> Result<()> {
    let (tn, l, _) = vint(b, true)?;
    if track.is_some_and(|t| t != tn) || b.len() < l + 3 {
        return Ok(());
    }
    let rel = i16::from_be_bytes([b[l], b[l + 1]]) as i64;
    let flags = b[l + 2];
    if flags & 0x06 != 0 {
        bail!("laced blocks unsupported");
    }
    let tc = (cluster_tc as i64 + rel).max(0) as u64;
    out.packets.push(WebmPacket {
        pts_ms: tc * scale_ns / 1_000_000,
        data: b[l + 3..].to_vec(),
        alpha,
    });
    Ok(())
}

/// Per-frame display durations (ms) from PTS deltas; last from tag or median.
pub fn frame_delays(f: &WebmFile) -> Vec<u32> {
    let p = &f.packets;
    let mut d: Vec<u32> = p.windows(2).map(|w| (w[1].pts_ms - w[0].pts_ms).max(10) as u32).collect();
    let mut sorted = d.clone();
    sorted.sort_unstable();
    let median = sorted.get(sorted.len() / 2).copied().unwrap_or(33);
    d.push(f.last_frame_ms.unwrap_or(median).max(10));
    d
}

/// Whether this build can decode VP9 WebM.
pub const VP9_AVAILABLE: bool = cfg!(has_vp9);

/// BT.601 limited-range YUV420 (+ full-range alpha plane) → straight RGBA.
#[allow(clippy::too_many_arguments)]
pub fn yuva420_to_rgba(
    w: usize,
    h: usize,
    y: &[u8],
    ys: usize,
    u: &[u8],
    us: usize,
    v: &[u8],
    vs: usize,
    a: Option<(&[u8], usize)>,
    out: &mut [u8],
) {
    // "Fancy" (triangle-filter) chroma upsampling, like libjpeg/libwebp: each output pixel
    // blends the nearest chroma sample 3:1 with its neighbour, horizontally and vertically.
    let cw = w.div_ceil(2);
    let ch = h.div_ceil(2);
    let mut urow = vec![0i32; w];
    let mut vrow = vec![0i32; w];
    let up = |plane: &[u8], stride: usize, row: usize, out: &mut [i32]| {
        let cr = row / 2;
        let nr = if row % 2 == 0 { cr.saturating_sub(1) } else { (cr + 1).min(ch - 1) };
        let (r0, r1) = (&plane[cr * stride..], &plane[nr * stride..]);
        // vertical blend (×4) into per-chroma-column values, then horizontal (×4)
        let vcol = |c: usize| 3 * r0[c] as i32 + r1[c] as i32;
        for (col, o) in out.iter_mut().enumerate() {
            let c = col / 2;
            let n = if col % 2 == 0 { c.saturating_sub(1) } else { (c + 1).min(cw - 1) };
            *o = (3 * vcol(c) + vcol(n) + 8) >> 4;
        }
    };
    for row in 0..h {
        up(u, us, row, &mut urow);
        up(v, vs, row, &mut vrow);
        let yr = &y[row * ys..];
        let o = &mut out[row * w * 4..(row + 1) * w * 4];
        for col in 0..w {
            let c = (yr[col] as i32 - 16) * 298;
            let d = urow[col] - 128;
            let e = vrow[col] - 128;
            let clamp = |x: i32| ((x + 128) >> 8).clamp(0, 255) as u8;
            o[col * 4] = clamp(c + 409 * e);
            o[col * 4 + 1] = clamp(c - 100 * d - 208 * e);
            o[col * 4 + 2] = clamp(c + 516 * d);
            o[col * 4 + 3] = match a {
                Some((ap, as_)) => ap[row * as_ + col],
                None => 255,
            };
        }
    }
}

#[cfg(has_vp9)]
mod ffi {
    use std::os::raw::{c_int, c_void};
    extern "C" {
        pub fn lp_vp9_new(threads: c_int) -> *mut c_void;
        pub fn lp_vp9_decode(
            h: *mut c_void,
            data: *const u8,
            len: usize,
            planes: *mut *const u8,
            strides: *mut c_int,
            w: *mut c_int,
            hh: *mut c_int,
        ) -> c_int;
        pub fn lp_vp9_free(h: *mut c_void);
    }
}

#[cfg(has_vp9)]
struct Vp9(*mut std::os::raw::c_void);

#[cfg(has_vp9)]
impl Drop for Vp9 {
    fn drop(&mut self) {
        unsafe { ffi::lp_vp9_free(self.0) }
    }
}

#[cfg(has_vp9)]
struct Plane<'a> {
    p: [&'a [u8]; 3],
    s: [usize; 3],
    w: usize,
    h: usize,
}

#[cfg(has_vp9)]
impl Vp9 {
    fn new() -> Result<Self> {
        let h = unsafe { ffi::lp_vp9_new(1) };
        if h.is_null() {
            bail!("vp9 decoder init failed");
        }
        Ok(Self(h))
    }
    fn decode(&mut self, data: &[u8]) -> Result<Option<Plane<'_>>> {
        let mut planes = [std::ptr::null::<u8>(); 3];
        let mut strides = [0i32; 3];
        let (mut w, mut h) = (0i32, 0i32);
        let r = unsafe {
            ffi::lp_vp9_decode(self.0, data.as_ptr(), data.len(), planes.as_mut_ptr(), strides.as_mut_ptr(), &mut w, &mut h)
        };
        match r {
            1 => {
                let (w, h) = (w as usize, h as usize);
                let s = strides.map(|s| s as usize);
                // SAFETY: libvpx keeps the frame buffer alive until the next decode call on
                // this context; the borrow of `self` enforces that.
                let p = unsafe {
                    [
                        std::slice::from_raw_parts(planes[0], s[0] * h),
                        std::slice::from_raw_parts(planes[1], s[1] * h.div_ceil(2)),
                        std::slice::from_raw_parts(planes[2], s[2] * h.div_ceil(2)),
                    ]
                };
                Ok(Some(Plane { p, s, w, h }))
            }
            0 => Ok(None),
            -2 => bail!("unsupported vp9 pixel format (need 8-bit 4:2:0)"),
            _ => bail!("vp9 decode error"),
        }
    }
}

/// Decode every frame to straight RGBA. Errors if this build has no VP9.
pub fn decode_rgba_frames(f: &WebmFile, max_frames: usize) -> Result<Vec<image::RgbaImage>> {
    #[cfg(not(has_vp9))]
    {
        let _ = (f, max_frames);
        bail!("built without VP9 decoder");
    }
    #[cfg(has_vp9)]
    {
        if f.codec != "V_VP9" {
            bail!("codec {} unsupported", f.codec);
        }
        let mut color = Vp9::new()?;
        let mut alpha = Vp9::new()?;
        let mut frames = Vec::with_capacity(f.packets.len().min(max_frames));
        for pkt in f.packets.iter().take(max_frames) {
            let Some(cp) = color.decode(&pkt.data)? else { continue };
            let (w, h) = (cp.w, cp.h);
            let mut buf = vec![0u8; w * h * 4];
            let ap = match &pkt.alpha {
                Some(a) => alpha.decode(a)?,
                None => None,
            };
            if let Some(ap) = &ap {
                if ap.w < w || ap.h < h {
                    bail!("alpha plane size mismatch");
                }
            }
            yuva420_to_rgba(
                w, h, cp.p[0], cp.s[0], cp.p[1], cp.s[1], cp.p[2], cp.s[2],
                ap.as_ref().map(|a| (a.p[0], a.s[0])),
                &mut buf,
            );
            frames.push(image::RgbaImage::from_raw(w as u32, h as u32, buf).ok_or_else(|| anyhow!("buf"))?);
        }
        if frames.is_empty() {
            bail!("no frames decoded");
        }
        Ok(frames)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vint_parsing() {
        assert_eq!(vint(&[0x81], true).unwrap(), (1, 1, false));
        assert_eq!(vint(&[0x40, 0x02], true).unwrap(), (2, 2, false));
        assert_eq!(vint(&[0x1A, 0x45, 0xDF, 0xA3], false).unwrap().0, 0x1A45DFA3);
        assert!(vint(&[0xFF], true).unwrap().2, "unknown size");
        assert!(vint(&[0x00], true).is_err());
    }

    #[test]
    fn yuv_black_white() {
        let (y, u, v, a) = ([16u8, 235], [128u8], [128u8], [0u8, 255]);
        let mut out = [0u8; 8];
        yuva420_to_rgba(2, 1, &y, 2, &u, 1, &v, 1, Some((&a, 2)), &mut out);
        assert_eq!(out, [0, 0, 0, 0, 255, 255, 255, 255]);
    }

    #[test]
    fn delays_from_pts() {
        let mk = |t| WebmPacket { pts_ms: t, ..Default::default() };
        let f = WebmFile { packets: vec![mk(0), mk(33), mk(67), mk(100)], last_frame_ms: Some(567), ..Default::default() };
        assert_eq!(frame_delays(&f), vec![33, 34, 33, 567]);
    }
}

#[cfg(all(test, has_vp9))]
mod vp9_tests {
    /// Decodes a real v2 file if `dist/assets-v2` was built (skips otherwise).
    #[test]
    fn decode_dist_webm_if_present() {
        let p = std::path::Path::new("dist/assets-v2/emotes/angry_kitty.webm");
        let Ok(b) = std::fs::read(p) else { return };
        let f = super::parse(&b).unwrap();
        assert_eq!(f.packets.len(), 74);
        assert!(f.packets.iter().all(|p| p.alpha.is_some()));
        let frames = super::decode_rgba_frames(&f, 200).unwrap();
        assert_eq!(frames.len(), 74);
        if let Ok(d) = std::env::var("LOLPING_DUMP") {
            frames[10].save(format!("{d}/rust10.png")).unwrap();
        }
    }
}
