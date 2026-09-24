#!/bin/bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

VERSION="$(awk -F'"' '/^version = / {print $2; exit}' Cargo.toml)"

ARCH="${1:-universal}"
case "$ARCH" in arm64|x86_64|universal) ;; *) echo "Expected arm64, x86_64 or universal" >&2; exit 2;; esac
BUNDLE_VERSION="${VERSION%%-*}"
APP="$ROOT/dist/$ARCH/Yeti3-Cleaner.app"
CONTENTS="$APP/Contents"
MACOS="$CONTENTS/MacOS"
RESOURCES="$CONTENTS/Resources"

ICON="$ROOT/assets/generated/appicon/Yeti3.icns"
ABOUT="$ROOT/assets/generated/about/Yeti3-About.png"
LAUNCHER="$ROOT/assets/generated/launcher"

test -s "$ICON"
test -s "$ABOUT"
test -s "$LAUNCHER/frame-00.png"
test -s "$LAUNCHER/frame-23.png"

printf '\n===== BUILD RELEASE =====\n'

BUILD_ARCHES=(arm64 x86_64)
if [ "$ARCH" != universal ]; then BUILD_ARCHES=("$ARCH"); fi
for CPU in "${BUILD_ARCHES[@]}"; do
  TARGET=aarch64-apple-darwin
  if [ "$CPU" = x86_64 ]; then TARGET=x86_64-apple-darwin; fi
  RUSTC="$(rustup which --toolchain 1.86.0 rustc)" MACOSX_DEPLOYMENT_TARGET=14.0 RUSTFLAGS="-D warnings" \
    rustup run 1.86.0 cargo build --release --target "$TARGET" \
    --bin yeti3-cleaner --bin yeti3-cleaner-tray
  mkdir -p "target/$CPU"
  xcrun swiftc -O -parse-as-library -target "$CPU-apple-macosx14.0" \
    native/DiskScanner.swift native/DiskCache.swift native/SettingsPanel.swift native/UpdatePolicy.swift native/DiskMap.swift \
    -o "target/$CPU/yeti3-disk-map"
done

printf '\n===== APP BUNDLE =====\n'

rm -rf "$APP"

mkdir -p \
  "$MACOS" \
  "$RESOURCES"

if [ "$ARCH" = universal ]; then
  lipo -create target/aarch64-apple-darwin/release/yeti3-cleaner-tray target/x86_64-apple-darwin/release/yeti3-cleaner-tray -output "$MACOS/Yeti3-Cleaner"
  lipo -create target/aarch64-apple-darwin/release/yeti3-cleaner target/x86_64-apple-darwin/release/yeti3-cleaner -output "$MACOS/yeti3-cleaner-engine"
else
  TARGET=aarch64-apple-darwin
  if [ "$ARCH" = x86_64 ]; then TARGET=x86_64-apple-darwin; fi
  cp "target/$TARGET/release/yeti3-cleaner-tray" "$MACOS/Yeti3-Cleaner"
  cp "target/$TARGET/release/yeti3-cleaner" "$MACOS/yeti3-cleaner-engine"
fi

printf '\n===== PREMIUM RESOURCES =====\n'

cp -f \
  "$ICON" \
  "$RESOURCES/Yeti3.icns"

cp -f \
  "$ABOUT" \
  "$RESOURCES/Yeti3-About.png"

cp -f \
  "$LAUNCHER"/frame-*.png \
  "$RESOURCES/"

cat > "$CONTENTS/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC
  "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>YETI³ Cleaner</string>

    <key>CFBundleDisplayName</key>
    <string>YETI³ Cleaner</string>

    <key>CFBundleIdentifier</key>
    <string>ru.yeti3.cleaner</string>

    <key>CFBundleExecutable</key>
    <string>Yeti3-Cleaner</string>

    <key>CFBundlePackageType</key>
    <string>APPL</string>

    <key>CFBundleIconFile</key>
    <string>Yeti3.icns</string>

    <key>CFBundleShortVersionString</key>
    <string>${BUNDLE_VERSION}</string>

    <key>CFBundleVersion</key>
    <string>${BUNDLE_VERSION}</string>

    <key>YetiReleaseVersion</key>
    <string>${VERSION}</string>
    <key>YetiReleaseChannel</key>
    <string>prerelease</string>

    <key>LSMinimumSystemVersion</key>
    <string>14.0</string>

    <key>LSUIElement</key>
    <true/>

    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
PLIST

chmod 755 \
  "$MACOS/Yeti3-Cleaner" \
  "$MACOS/yeti3-cleaner-engine"

HELPER="$CONTENTS/Helpers/Yeti3-DiskMap.app"
mkdir -p "$HELPER/Contents/MacOS" "$HELPER/Contents/Resources"
if [ "$ARCH" = universal ]; then
  lipo -create target/arm64/yeti3-disk-map target/x86_64/yeti3-disk-map -output "$HELPER/Contents/MacOS/yeti3-disk-map"
else
  cp "target/$ARCH/yeti3-disk-map" "$HELPER/Contents/MacOS/yeti3-disk-map"
fi
cp "$ICON" "$HELPER/Contents/Resources/Yeti3.icns"
cat > "$HELPER/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>ru.yeti3.cleaner.diskmap</string>
<key>CFBundleName</key><string>YETI³ Карта диска</string>
<key>CFBundleExecutable</key><string>yeti3-disk-map</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleIconFile</key><string>Yeti3.icns</string>
<key>CFBundleShortVersionString</key><string>${BUNDLE_VERSION}</string>
<key>CFBundleVersion</key><string>${BUNDLE_VERSION}</string>
<key>YetiReleaseVersion</key><string>${VERSION}</string>
<key>YetiReleaseChannel</key><string>prerelease</string>
<key>LSMinimumSystemVersion</key><string>14.0</string>
</dict></plist>
PLIST
codesign --force --sign - "$HELPER"

printf '\n===== VERIFY =====\n'

plutil -lint "$CONTENTS/Info.plist"

test -s "$RESOURCES/Yeti3.icns"
test -s "$RESOURCES/Yeti3-About.png"
test -s "$RESOURCES/frame-00.png"
test -s "$RESOURCES/frame-23.png"

cmp \
  "$ICON" \
  "$RESOURCES/Yeti3.icns"

printf 'VERSION='
/usr/libexec/PlistBuddy \
  -c 'Print :CFBundleShortVersionString' \
  "$CONTENTS/Info.plist"

printf 'ICON='
/usr/libexec/PlistBuddy \
  -c 'Print :CFBundleIconFile' \
  "$CONTENTS/Info.plist"

printf '\n===== ARCHITECTURE =====\n'

file "$MACOS/Yeti3-Cleaner"
file "$MACOS/yeti3-cleaner-engine"

printf '\n===== SIGN =====\n'

codesign \
  --force \
  --deep \
  --sign - \
  "$APP"

codesign \
  --verify \
  --deep \
  --strict \
  "$APP"

printf '\n===== RESULT =====\n'
echo "$APP"
