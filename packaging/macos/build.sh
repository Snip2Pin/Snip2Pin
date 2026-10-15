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
PROFILE=debug
OUTPUT_DIR="$PROJECT_DIR/.build/macos"
cargo build --locked
APP_DIR="$OUTPUT_DIR/Snip2Pin.app"
rm -rf "$APP_DIR"
ICON_VARIANT=line
if "$INSTALL"; then
    ICON_VARIANT="$("target/$PROFILE/snip2pin" --selected-icon)"
fi
cargo xtask bundle-macos "target/$PROFILE/snip2pin" "$APP_DIR" "$ICON_VARIANT"
bash packaging/macos/signing.sh --sign "$APP_DIR"
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
