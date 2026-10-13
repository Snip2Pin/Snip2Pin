#!/usr/bin/env bash
set -euo pipefail

PROJECT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$PROJECT_DIR"
swift build -c release
BIN_DIR="$(swift build -c release --show-bin-path)"
APP_DIR="$PROJECT_DIR/dist/SnipSprig.app"
mkdir -p "$APP_DIR/Contents/MacOS" "$APP_DIR/Contents/Resources" .build/iconset/AppIcon.iconset
cp "$BIN_DIR/SnipSprig" "$APP_DIR/Contents/MacOS/SnipSprig"
cp Resources/Info.plist "$APP_DIR/Contents/Info.plist"
swift scripts/make-icon.swift .build/iconset/AppIcon.iconset
iconutil -c icns .build/iconset/AppIcon.iconset -o "$APP_DIR/Contents/Resources/AppIcon.icns"
plutil -lint "$APP_DIR/Contents/Info.plist"
codesign --force --sign - --identifier io.github.rensilin.snipsprig "$APP_DIR"
codesign --verify --strict "$APP_DIR"
printf 'Built: %s\n' "$APP_DIR"
