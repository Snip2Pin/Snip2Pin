#!/usr/bin/env bash
set -euo pipefail
PROJECT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$PROJECT_DIR"
WORK_DIR="$(mktemp -d /private/tmp/snip2pin-signing-test.XXXXXX)"
trap 'rm -rf "$WORK_DIR"' EXIT
ORIGINAL_SEARCH_LIST="$(security list-keychains -d user)"
for variant in first second; do
    APP="$WORK_DIR/$variant.app"
    mkdir -p "$APP/Contents/MacOS"
    rustc packaging/macos/tests/signing.rs --cfg "${variant}_build" \
        -o "$APP/Contents/MacOS/snip2pin"
    cp packaging/macos/Info.plist "$APP/Contents/Info.plist"
    bash packaging/macos/signing.sh --sign "$APP"
done
[[ "$(security list-keychains -d user)" == "$ORIGINAL_SEARCH_LIST" ]]
if bash packaging/macos/signing.sh --sign "$WORK_DIR/missing.app" 2>/dev/null; then
    printf 'FAIL: signing a missing app succeeded.\n' >&2
    exit 1
fi
[[ "$(security list-keychains -d user)" == "$ORIGINAL_SEARCH_LIST" ]]
requirement() {
    codesign -d -r- "$1" 2>&1 | awk '/^(# )?designated => / {sub(/^(# )?designated => /, ""); print}'
}
FIRST_REQUIREMENT="$(requirement "$WORK_DIR/first.app")"
[[ "$FIRST_REQUIREMENT" == "$(requirement "$WORK_DIR/second.app")" ]]
FIRST_HASH="$(codesign -d --verbose=4 "$WORK_DIR/first.app" 2>&1 | awk '/^CDHash=/ {print}')"
SECOND_HASH="$(codesign -d --verbose=4 "$WORK_DIR/second.app" 2>&1 | awk '/^CDHash=/ {print}')"
[[ "$FIRST_HASH" != "$SECOND_HASH" ]]
codesign --verify --strict -R "=$FIRST_REQUIREMENT" "$WORK_DIR/second.app"
bash packaging/macos/signing.sh --verify-update "$WORK_DIR/second.app" "$WORK_DIR/first.app"
# Identifier migration must be explicit and retain the installed certificate.
TEST_ID="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' packaging/macos/Info.plist).test"
plutil -replace CFBundleIdentifier -string "$TEST_ID" "$WORK_DIR/second.app/Contents/Info.plist"
bash packaging/macos/signing.sh --sign "$WORK_DIR/second.app"
if bash packaging/macos/signing.sh --verify-update "$WORK_DIR/second.app" "$WORK_DIR/first.app" 2>/dev/null; then
    printf 'FAIL: identifier changed without explicit migration.\n' >&2
    exit 1
fi
bash packaging/macos/signing.sh --verify-update \
    "$WORK_DIR/second.app" "$WORK_DIR/first.app" --migrate-identity
# An unrelated ad-hoc signature with the same bundle ID must not impersonate an update.
codesign --force --sign - --identifier "$TEST_ID" "$WORK_DIR/second.app"
[[ "$(requirement "$WORK_DIR/second.app")" == cdhash\ * ]]
if codesign --verify --strict -R "=$FIRST_REQUIREMENT" "$WORK_DIR/second.app" 2>/dev/null; then
    printf 'FAIL: an unsigned identity impersonated a certificate-signed update.\n' >&2
    exit 1
fi
if bash packaging/macos/signing.sh --verify-update \
    "$WORK_DIR/second.app" "$WORK_DIR/first.app" --migrate-identity 2>/dev/null; then
    printf 'FAIL: identifier migration accepted a different signing identity.\n' >&2
    exit 1
fi
printf 'PASS: stable identity and explicit migration verified; impostors fail; keychain search list restored.\n'
