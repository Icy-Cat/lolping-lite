#!/usr/bin/env bash
# Build LolPing Lite.app + DMG for macOS (Apple Silicon / Intel).
# Does NOT bundle the ~54 MB assets/original — CDN on-demand.
# Usage: scripts/package-macos.sh [path-to-release-binary]
# Env:   SKIP_BUILD=1  — reuse existing target/release/lolping-lite
#        SIGN_IDENTITY — codesign identity (default: ad-hoc "-")
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "$ROOT"

VERSION=$(grep -m1 '^version' Cargo.toml | sed 's/.*"\(.*\)"/\1/')
APP_NAME="LolPing Lite"
BUNDLE_ID="com.icy-cat.lolping-lite"
DIST="$ROOT/dist/macos"
APP="$DIST/${APP_NAME}.app"
DMG="$DIST/LolPing-Lite-${VERSION}-macos-$(uname -m).dmg"
SIGN_IDENTITY=${SIGN_IDENTITY:--}

if [ -z "${1:-}" ] && [ "${SKIP_BUILD:-}" != "1" ]; then
  if [ ! -f vendor/libvpx/lib/libvpx.a ]; then
    echo "==> building libvpx (VP9 decoder)"
    scripts/build-libvpx.sh
  fi
  echo "==> cargo build --release"
  cargo build --release
fi

BIN=${1:-"$ROOT/target/release/lolping-lite"}
if [ ! -x "$BIN" ]; then
  echo "missing binary: $BIN" >&2
  exit 1
fi

# Confirm VP9 (look for linked libvpx symbols; UI strings alone are unreliable)
if nm -gU "$BIN" 2>/dev/null | grep -q ' _vpx_codec_dec_init_ver\|_vpx_codec_decode'; then
  echo "VP9: linked"
elif otool -L "$BIN" 2>/dev/null | grep -qi vpx; then
  echo "VP9: linked (dylib)"
else
  echo "warning: no libvpx symbols found — likely v1 WebP only" >&2
fi

rm -rf "$DIST"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"

cp "$BIN" "$APP/Contents/MacOS/lolping-lite"
chmod +x "$APP/Contents/MacOS/lolping-lite"
cp LICENSE "$APP/Contents/Resources/LICENSE"
cp ASSETS.md "$APP/Contents/Resources/ASSETS.md"

# Minimal icon-less Info.plist. LSUIElement=true → no Dock icon (overlay tool).
cat > "$APP/Contents/Info.plist" << PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key><string>en</string>
  <key>CFBundleExecutable</key><string>lolping-lite</string>
  <key>CFBundleIdentifier</key><string>${BUNDLE_ID}</string>
  <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
  <key>CFBundleName</key><string>${APP_NAME}</string>
  <key>CFBundleDisplayName</key><string>${APP_NAME}</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>${VERSION}</string>
  <key>CFBundleVersion</key><string>${VERSION}</string>
  <key>LSMinimumSystemVersion</key><string>13.0</string>
  <key>LSUIElement</key><true/>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSSupportsAutomaticGraphicsSwitching</key><true/>
  <key>NSHumanReadableCopyright</key>
  <string>Copyright © Icy-Cat. Code MIT; assets © Riot Games.</string>
</dict>
</plist>
PLIST

# Ad-hoc or Developer ID sign
echo "==> codesign (${SIGN_IDENTITY})"
# Ad-hoc ("-") cannot use hardened runtime without a real Developer ID + entitlements.
if [ "$SIGN_IDENTITY" = "-" ]; then
  codesign --force --deep --sign - "$APP"
else
  codesign --force --deep --options runtime --sign "$SIGN_IDENTITY" "$APP"
fi
codesign --verify --verbose=2 "$APP" || true

# DMG with Applications symlink (drag-to-Applications)
STAGE="$DIST/dmg-stage"
rm -rf "$STAGE"
mkdir -p "$STAGE"
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
# Brief readme for first-run Gatekeeper / Accessibility
cat > "$STAGE/README.txt" << TXT
LolPing Lite ${VERSION}

Install: drag "LolPing Lite" into Applications.

First run (ad-hoc / unsigned builds):
  Right-click → Open, or: xattr -cr "/Applications/LolPing Lite.app"

Then grant Accessibility:
  System Settings → Privacy & Security → Accessibility → enable LolPing Lite

Assets download on demand from https://lolping-assets.icy-cat.com/
(or Settings → Assets → Download all). Nothing Riot-owned is bundled here.

Code MIT; ping/emote media © Riot Games. Not endorsed by Riot.
TXT

rm -f "$DMG"
hdiutil create -volname "LolPing Lite" -srcfolder "$STAGE" -ov -format UDZO "$DMG"
rm -rf "$STAGE"

echo
echo "App:  $APP"
du -sh "$APP"
echo "DMG:  $DMG"
ls -lh "$DMG"
