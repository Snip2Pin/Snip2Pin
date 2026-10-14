#!/usr/bin/env bash
set -euo pipefail

PROJECT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$PROJECT_DIR"
RELEASE=false
INSTALL=false
MIGRATE=false
for argument in "$@"; do
    case "$argument" in
        --release) RELEASE=true ;;
        --install) INSTALL=true ;;
        --migrate-identity) MIGRATE=true ;;
        *) printf 'Usage: build.sh [--install] [--release] [--migrate-identity]\n' >&2; exit 2 ;;
    esac
done
if "$MIGRATE" && ! "$INSTALL"; then
    printf 'Identity migration requires --install.\n' >&2
    exit 2
fi
BUILD_ARGS=(--locked)
PROFILE=debug
OUTPUT_DIR="$PROJECT_DIR/.build/macos"
if "$RELEASE"; then
    [[ -n "${SNIP2PIN_SIGN_IDENTITY:-}" ]] || {
        printf 'Distribution requires an explicit Developer ID signing identity.\n' >&2
        exit 1
    }
    BUILD_ARGS+=(--release)
    PROFILE=release
    OUTPUT_DIR="$PROJECT_DIR/dist"
fi
cargo build "${BUILD_ARGS[@]}"
APP_DIR="$OUTPUT_DIR/Snip2Pin.app"
ICONSET="$PROJECT_DIR/target/iconset/AppIcon.iconset"
rm -rf "$APP_DIR" "$ICONSET"
mkdir -p "$APP_DIR/Contents/MacOS" "$APP_DIR/Contents/Resources" "$ICONSET"
cp "target/$PROFILE/snip2pin" "$APP_DIR/Contents/MacOS/snip2pin"
cp packaging/macos/Info.plist "$APP_DIR/Contents/Info.plist"
ICON_VARIANT=line
if "$INSTALL"; then
    ICON_VARIANT="$("target/$PROFILE/snip2pin" --selected-icon)"
fi
case "$ICON_VARIANT" in
    line) ICON_NAME=AppIcon-Line ;;
    metal) ICON_NAME=AppIcon-Metal ;;
    *) printf 'Invalid icon variant.\n' >&2; exit 1 ;;
esac
plutil -replace CFBundleIconFile -string "$ICON_NAME" "$APP_DIR/Contents/Info.plist"
for size in 16 32 128 256 512; do
    sips -z "$size" "$size" "assets/icons/$ICON_VARIANT.png" \
        --out "$ICONSET/icon_${size}x${size}.png" >/dev/null
    doubled=$((size * 2))
    sips -z "$doubled" "$doubled" "assets/icons/$ICON_VARIANT.png" \
        --out "$ICONSET/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$APP_DIR/Contents/Resources/$ICON_NAME.icns"
plutil -lint "$APP_DIR/Contents/Info.plist"
bash packaging/macos/signing.sh --sign "$APP_DIR"
if "$RELEASE"; then
    codesign --verify --strict \
        -R '=anchor apple generic and certificate leaf[field.1.2.840.113635.100.6.1.13] exists' \
        "$APP_DIR"
    ditto -c -k --sequesterRsrc --keepParent "$APP_DIR" "$OUTPUT_DIR/Snip2Pin-macOS.zip"
fi
if "$INSTALL"; then
    DESTINATION=/Applications/Snip2Pin.app
    if [[ -d "$DESTINATION" ]]; then
        VERIFY_ARGS=(--verify-update "$APP_DIR" "$DESTINATION")
        if "$MIGRATE"; then
            VERIFY_ARGS+=(--migrate-identity)
        fi
        bash packaging/macos/signing.sh "${VERIFY_ARGS[@]}"
    fi
    INSTALL_DIR="$(mktemp -d /Applications/.snip2pin-install.XXXXXX)"
    ditto "$APP_DIR" "$INSTALL_DIR/Snip2Pin.app"
    if [[ -d "$DESTINATION" ]]; then
        BACKUP_DIR="$(mktemp -d /private/tmp/snip2pin-previous.XXXXXX)"
        BACKUP_APP="$BACKUP_DIR/${DESTINATION##*/}"
        mv "$DESTINATION" "$BACKUP_APP"
    fi
    if ! mv "$INSTALL_DIR/Snip2Pin.app" "$DESTINATION"; then
        if [[ -n "${BACKUP_APP:-}" ]]; then
            mv "$BACKUP_APP" "$DESTINATION"
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
        'ObjC.import("AppKit"); $.NSWorkspace.sharedWorkspace.noteFileSystemChanged("/Applications/Snip2Pin.app");'
    printf 'Installed: %s\n' "$DESTINATION"
fi
printf 'Built: %s\n' "$APP_DIR"
