#!/usr/bin/env python3
"""Verify v2 WebM emotes vs v1 WebP: frame count, size, alpha, composited PSNR.
Usage: scripts/verify-v2-assets.py [src=assets/original] [v2=dist/assets-v2]
Decodes with -c:v libvpx-vp9 (ffmpeg's native vp9 decoder drops alpha)."""
import math, subprocess, sys
from pathlib import Path
from PIL import Image
import numpy as np

src = Path(sys.argv[1] if len(sys.argv) > 1 else "assets/original")
v2 = Path(sys.argv[2] if len(sys.argv) > 2 else "dist/assets-v2")
bad = 0; rows = []
for wp in sorted((src / "emotes").glob("*.webp")):
    wm = v2 / "emotes" / (wp.stem + ".webm")
    im = Image.open(wp); n = im.n_frames; W, H = im.size
    raw = subprocess.run(["ffmpeg", "-v", "error", "-c:v", "libvpx-vp9", "-i", str(wm),
                          "-f", "rawvideo", "-pix_fmt", "rgba", "-"], capture_output=True, check=True).stdout
    dec = np.frombuffer(raw, np.uint8).reshape(-1, H, W, 4).astype(np.float64)
    ok = dec.shape[0] == n
    se = {"black": 0.0, "white": 0.0, "alpha": 0.0}; cnt = 0
    for k in range(min(n, dec.shape[0])):
        im.seek(k); o = np.asarray(im.convert("RGBA"), np.float64); d = dec[k]
        ao, ad = o[..., 3:] / 255, d[..., 3:] / 255
        for name, bg in (("black", 0.0), ("white", 255.0)):
            co = o[..., :3] * ao + bg * (1 - ao); cd = d[..., :3] * ad + bg * (1 - ad)
            se[name] += ((co - cd) ** 2).mean()
        se["alpha"] += ((o[..., 3] - d[..., 3]) ** 2).mean(); cnt += 1
    psnr = {k: (99.0 if v == 0 else 10 * math.log10(255 ** 2 / (v / cnt))) for k, v in se.items()}
    has_alpha = dec[..., 3].min() < 255
    if not ok or not has_alpha: bad += 1
    rows.append((wp.stem, n, dec.shape[0], wp.stat().st_size, wm.stat().st_size, psnr, has_alpha))
    print(f"{'OK ' if ok and has_alpha else 'BAD'} {wp.stem:28s} frames {n}/{dec.shape[0]} "
          f"{wp.stat().st_size/1e3:7.0f}K -> {wm.stat().st_size/1e3:6.0f}K  PSNR black {psnr['black']:.2f} "
          f"white {psnr['white']:.2f} alpha {psnr['alpha']:.2f}")
mean = lambda k: sum(r[5][k] for r in rows) / len(rows)
print(f"\n{len(rows)} emotes, {bad} bad; mean PSNR black {mean('black'):.2f} white {mean('white'):.2f} alpha {mean('alpha'):.2f}; "
      f"min black {min(r[5]['black'] for r in rows):.2f}")
sys.exit(1 if bad else 0)
