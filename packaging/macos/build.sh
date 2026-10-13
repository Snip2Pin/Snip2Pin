#!/usr/bin/env bash
set -euo pipefail

PROJECT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$PROJECT_DIR"
cargo build --locked --release
APP_DIR="$PROJECT_DIR/dist/SnipSprig.app"
rm -rf "$APP_DIR"
mkdir -p "$APP_DIR/Contents/MacOS" "$APP_DIR/Contents/Resources" target/iconset/AppIcon.iconset
cp target/release/snipsprig "$APP_DIR/Contents/MacOS/snipsprig"
cp packaging/macos/Info.plist "$APP_DIR/Contents/Info.plist"
target/release/snipsprig --write-icon target/iconset/icon.png
for size in 16 32 128 256 512; do
    sips -z "$size" "$size" target/iconset/icon.png --out "target/iconset/AppIcon.iconset/icon_${size}x${size}.png" >/dev/null
    doubled=$((size * 2))
    sips -z "$doubled" "$doubled" target/iconset/icon.png --out "target/iconset/AppIcon.iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns target/iconset/AppIcon.iconset -o "$APP_DIR/Contents/Resources/AppIcon.icns"
plutil -lint "$APP_DIR/Contents/Info.plist"
codesign --force --sign - --identifier io.github.rensilin.snipsprig "$APP_DIR"
codesign --verify --strict "$APP_DIR"
ditto -c -k --sequesterRsrc --keepParent "$APP_DIR" "$PROJECT_DIR/dist/SnipSprig-macOS.zip"
printf 'Built: %s\n' "$APP_DIR"
