#!/usr/bin/env bash
set -euo pipefail

PROJECT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$PROJECT_DIR"
INSTALL=false
MIGRATE=false
for argument in "$@"; do
    case "$argument" in
        --install) INSTALL=true ;;
        --migrate-identity) MIGRATE=true ;;
        *) printf 'Usage: build.sh [--install] [--migrate-identity]\nDistribution: see docs/workflow.md\n' >&2; exit 2 ;;
    esac
done
if "$MIGRATE" && ! "$INSTALL"; then
    printf 'Identity migration requires --install.\n' >&2
    exit 2
fi
PROFILE=release
CARGO_BUILD_TARGET="$(rustc -vV | awk '/^host:/ {print $2}')"
export CARGO_BUILD_TARGET
TARGET_DIR="$(cargo metadata --locked --no-deps --format-version 1 | plutil -extract target_directory raw -o - -)"
BINARY="$TARGET_DIR/$CARGO_BUILD_TARGET/$PROFILE/snip2pin"
OUTPUT_DIR="$PROJECT_DIR/.build/macos"
cargo build --locked --release
APP_DIR="$OUTPUT_DIR/Snip2Pin.app"
rm -rf "$APP_DIR"
ICON_VARIANT=line
if "$INSTALL"; then
    ICON_VARIANT="$("$BINARY" --selected-icon)"
fi
cargo xtask bundle-macos "$BINARY" "$APP_DIR" "$ICON_VARIANT"
bash packaging/macos/signing.sh --sign "$APP_DIR"
if "$INSTALL"; then
    DESTINATION=/Applications/Snip2Pin.app
    MIGRATING=false
    LSREGISTER=/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister
    if [[ -d "$DESTINATION" ]]; then
        VERIFY_ARGS=(--verify-update "$APP_DIR" "$DESTINATION")
        if "$MIGRATE"; then
            VERIFY_ARGS+=(--migrate-identity)
        fi
        bash packaging/macos/signing.sh "${VERIFY_ARGS[@]}"
        OLD_ID="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$DESTINATION/Contents/Info.plist")"
        NEW_ID="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$APP_DIR/Contents/Info.plist")"
        if [[ "$OLD_ID" != "$NEW_ID" ]]; then
            "$BINARY" --check-identity-migration "$DESTINATION"
            MIGRATING=true
        fi
    fi
    INSTALL_DIR="$(mktemp -d /Applications/.snip2pin-install.XXXXXX)"
    ditto "$APP_DIR" "$INSTALL_DIR/Snip2Pin.app"
    if [[ -d "$DESTINATION" ]]; then
        BACKUP_DIR="$(mktemp -d /private/tmp/snip2pin-previous.XXXXXX)"
        BACKUP_APP="$BACKUP_DIR/Snip2Pin.backup"
        if "$MIGRATING"; then
            "$LSREGISTER" -u "$DESTINATION"
        fi
        mv "$DESTINATION" "$BACKUP_APP"
    fi
    if ! mv "$INSTALL_DIR/Snip2Pin.app" "$DESTINATION"; then
        if [[ -n "${BACKUP_APP:-}" ]]; then
            mv "$BACKUP_APP" "$DESTINATION"
            "$LSREGISTER" -f "$DESTINATION"
        fi
        exit 1
    fi
    rmdir "$INSTALL_DIR"
    if ! codesign --verify --strict "$DESTINATION" || \
        ! "$LSREGISTER" -f "$DESTINATION" || \
        ! "$DESTINATION/Contents/MacOS/snip2pin" --check-installation; then
        rm -rf "$DESTINATION"
        if [[ -n "${BACKUP_APP:-}" ]]; then
            mv "$BACKUP_APP" "$DESTINATION"
            "$LSREGISTER" -f "$DESTINATION"
        fi
        printf 'Installation validation failed; the previous application was restored.\n' >&2
        exit 1
    fi
    if [[ -n "${BACKUP_DIR:-}" ]]; then
        rm -rf "$BACKUP_DIR"
    fi
    # Update only this app's metadata and notify Finder; do not restart Dock or Finder.
    touch "$DESTINATION"
    osascript -l JavaScript -e \
        'ObjC.import("AppKit"); $.NSWorkspace.sharedWorkspace.noteFileSystemChanged("/Applications/Snip2Pin.app");'
    printf 'Installed: %s\n' "$DESTINATION"
fi
printf 'Built: %s\n' "$APP_DIR"
