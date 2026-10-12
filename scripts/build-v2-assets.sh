#!/usr/bin/env bash
# Build the v2 asset set: animated emote WebP -> transparent VP9 WebM (alpha), rest copied as-is.
# Usage: scripts/build-v2-assets.sh [src=assets/original] [out=dist/assets-v2]
# Never modifies the source directory.
set -euo pipefail
SRC=${1:-assets/original}
OUT=${2:-dist/assets-v2}
JOBS=${JOBS:-$(nproc 2>/dev/null || sysctl -n hw.ncpu)}
command -v ffmpeg >/dev/null || { echo "ffmpeg (with libvpx-vp9) required"; exit 1; }
mkdir -p "$OUT/emotes" "$OUT/sounds" "$OUT/textures" "$OUT/.frames"
cp -p "$SRC"/sounds/* "$OUT/sounds/"
cp -p "$SRC"/textures/* "$OUT/textures/"
cp -p "$SRC"/emotes/*.png "$SRC"/emotes/*.ogg "$OUT/emotes/"

convert_one() {
  set -euo pipefail
  src=$1; out=$2
  name=$(basename "$src" .webp)
  fr="$out/.frames/$name"; rm -rf "$fr"; mkdir -p "$fr"
  # ffmpeg can't decode animated WebP: extract composited RGBA frames with Pillow and the
  # exact per-frame durations from the ANMF chunks, then encode VFR with an explicit setpts expression
  # (keeps frame count and the long hold on the last frame).
  # WebM stores no duration for the last frame; it is kept in the stream tag LAST_FRAME_MS.
  python3 - "$src" "$fr" <<'PY'
import struct, sys
from PIL import Image
src, fr = sys.argv[1], sys.argv[2]
b = open(src, "rb").read(); i = 12; durs = []
while i < len(b):
    t = b[i:i+4]; n = struct.unpack("<I", b[i+4:i+8])[0]
    if t == b"ANMF": durs.append(int.from_bytes(b[i+20:i+23], "little") or 33)
    i += 8 + n + (n & 1)
im = Image.open(src)
assert im.n_frames == len(durs), (im.n_frames, len(durs))
for k in range(len(durs)):
    im.seek(k); im.convert("RGBA").save(f"{fr}/{k:04d}.png")
# PTS(ms) of frame N = sum of previous durations, as a flat ffmpeg expression.
open(f"{fr}/pts.txt", "w").write("+".join(f"{d}*gt(N,{k})" for k, d in enumerate(durs[:-1])) or "0")
open(f"{fr}/last.txt", "w").write(str(durs[-1]))
PY
  ffmpeg -nostdin -loglevel error -y -framerate 1000 -i "$fr/%04d.png" \
    -vf "setpts='($(cat "$fr/pts.txt"))/1000/TB'" -fps_mode passthrough -enc_time_base 1/1000 \
    -c:v libvpx-vp9 -pix_fmt yuva420p -crf 10 -b:v 0 -cpu-used 1 -row-mt 1 \
    -auto-alt-ref 0 -metadata:s:v:0 LAST_FRAME_MS="$(cat "$fr/last.txt")" -an "$out/emotes/$name.webm"
  rm -rf "$fr"
  echo "ok $name"
}
export -f convert_one
ls "$SRC"/emotes/*.webp | xargs -P "$JOBS" -I{} bash -c 'convert_one "$@"' _ {} "$OUT"
rmdir "$OUT/.frames" 2>/dev/null || true

# Manifest + zip
python3 - "$OUT" <<'PY'
import hashlib, json, os, sys, zipfile
root = sys.argv[1]; files = []
for d, _, fs in os.walk(root):
    for f in sorted(fs):
        p = os.path.join(d, f); rel = os.path.relpath(p, root).replace(os.sep, "/")
        if rel.startswith(".") or rel in ("manifest.json", "lolping-assets-v2.zip"): continue
        b = open(p, "rb").read(); files.append({"path": rel, "size": len(b), "sha256": hashlib.sha256(b).hexdigest()})
files.sort(key=lambda f: f["path"])
json.dump({"version": 1, "files": files}, open(os.path.join(root, "manifest.json"), "w"), indent=1)
with zipfile.ZipFile(os.path.join(root, "lolping-assets-v2.zip"), "w", zipfile.ZIP_DEFLATED) as z:
    for f in files: z.write(os.path.join(root, f["path"]), f["path"])
print(len(files), "files,", sum(f["size"] for f in files), "bytes")
PY
