#!/usr/bin/env bash
set -euo pipefail

PROJECT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$PROJECT_DIR"
RELEASE=false
INSTALL=false
for argument in "$@"; do
    case "$argument" in
        --release) RELEASE=true ;;
        --install) INSTALL=true ;;
        *) printf 'Usage: build.sh [--install] [--release]\n' >&2; exit 2 ;;
    esac
done
BUILD_ARGS=(--locked)
PROFILE=debug
OUTPUT_DIR="$PROJECT_DIR/.build/macos"
if "$RELEASE"; then
    [[ -n "${SNIPSPRIG_SIGN_IDENTITY:-}" ]] || {
        printf 'Distribution requires an explicit Developer ID signing identity.\n' >&2
        exit 1
    }
    BUILD_ARGS+=(--release)
    PROFILE=release
    OUTPUT_DIR="$PROJECT_DIR/dist"
fi
cargo build "${BUILD_ARGS[@]}"
APP_DIR="$OUTPUT_DIR/SnipSprig.app"
ICONSET="$PROJECT_DIR/target/iconset/AppIcon.iconset"
rm -rf "$APP_DIR" "$ICONSET"
mkdir -p "$APP_DIR/Contents/MacOS" "$APP_DIR/Contents/Resources" "$ICONSET"
cp "target/$PROFILE/snipsprig" "$APP_DIR/Contents/MacOS/snipsprig"
cp packaging/macos/Info.plist "$APP_DIR/Contents/Info.plist"
for size in 16 32 128 256 512; do
    sips -z "$size" "$size" assets/icons/line.png \
        --out "$ICONSET/icon_${size}x${size}.png" >/dev/null
    doubled=$((size * 2))
    sips -z "$doubled" "$doubled" assets/icons/line.png \
        --out "$ICONSET/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$APP_DIR/Contents/Resources/AppIcon-Line.icns"
plutil -lint "$APP_DIR/Contents/Info.plist"
bash packaging/macos/signing.sh --sign "$APP_DIR"
if "$RELEASE"; then
    codesign --verify --strict \
        -R '=anchor apple generic and certificate leaf[field.1.2.840.113635.100.6.1.13] exists' \
        "$APP_DIR"
    ditto -c -k --sequesterRsrc --keepParent "$APP_DIR" "$OUTPUT_DIR/SnipSprig-macOS.zip"
fi
if "$INSTALL"; then
    DESTINATION=/Applications/SnipSprig.app
    if [[ -d "$DESTINATION" ]]; then
        codesign --verify --strict "$DESTINATION"
        OLD_REQUIREMENT="$(codesign -d -r- "$DESTINATION" 2>&1 | awk '/^(# )?designated => / {sub(/^(# )?designated => /, ""); print}')"
        [[ -n "$OLD_REQUIREMENT" ]] || {
            printf 'Could not read the installed app\047s signing requirement.\n' >&2
            exit 1
        }
        if [[ "$OLD_REQUIREMENT" != cdhash\ * ]] && \
            ! codesign --verify --strict -R "=$OLD_REQUIREMENT" "$APP_DIR"; then
            printf 'The signing identity changed. Reuse the installed app\047s certificate before updating.\n' >&2
            exit 1
        fi
    fi
    INSTALL_DIR="$(mktemp -d /Applications/.snipsprig-install.XXXXXX)"
    ditto "$APP_DIR" "$INSTALL_DIR/SnipSprig.app"
    if [[ -d "$DESTINATION" ]]; then
        BACKUP_DIR="$(mktemp -d /private/tmp/snipsprig-previous.XXXXXX)"
        mv "$DESTINATION" "$BACKUP_DIR/SnipSprig.app"
    fi
    if ! mv "$INSTALL_DIR/SnipSprig.app" "$DESTINATION"; then
        if [[ -n "${BACKUP_DIR:-}" ]]; then
            mv "$BACKUP_DIR/SnipSprig.app" "$DESTINATION"
        fi
        exit 1
    fi
    rmdir "$INSTALL_DIR"
    codesign --verify --strict "$DESTINATION"
    # Update only this app's metadata and notify Finder; do not restart Dock or Finder.
    touch "$DESTINATION"
    /System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister \
        -f "$DESTINATION"
    osascript -l JavaScript -e \
        'ObjC.import("AppKit"); $.NSWorkspace.sharedWorkspace.noteFileSystemChanged("/Applications/SnipSprig.app");'
    printf 'Installed: %s\n' "$DESTINATION"
fi
printf 'Built: %s\n' "$APP_DIR"
