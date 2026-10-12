#!/usr/bin/env bash
# Build a minimal static, decoder-only libvpx (VP9 only) for the in-app WebM emote decoder.
# Output: vendor/libvpx/{include,lib/libvpx.a}. build.rs picks it up automatically
# (or set LIBVPX_DIR). Without it the app still builds and falls back to v1 WebP emotes.
# Needs: git, make, a C compiler, and nasm (x86) for SIMD.
set -euo pipefail
VER=${LIBVPX_VERSION:-v1.14.1}
ROOT=$(cd "$(dirname "$0")/.." && pwd)
OUT=${1:-$ROOT/vendor/libvpx}
WORK=${WORK:-$ROOT/target/libvpx-build}
mkdir -p "$WORK"
[ -d "$WORK/src" ] || git clone -q --depth 1 -b "$VER" https://github.com/webmproject/libvpx.git "$WORK/src"
mkdir -p "$WORK/build" && cd "$WORK/build"
EXTRA=()
if [ "$(uname)" = Darwin ] && [ -n "${VPX_TARGET:-}" ]; then EXTRA+=(--target="$VPX_TARGET"); fi
"$WORK/src/configure" --prefix="$OUT" "${EXTRA[@]}" \
  --disable-vp8 --disable-vp9-encoder --disable-vp9-highbitdepth \
  --disable-examples --disable-tools --disable-docs --disable-unit-tests \
  --enable-static --disable-shared --enable-pic --size-limit=2048x2048 --as=auto
make -j"$(nproc 2>/dev/null || sysctl -n hw.ncpu)"
make install
echo "libvpx -> $OUT"
