#!/usr/bin/env bash
set -euo pipefail
PROJECT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$PROJECT_DIR"
WORK_DIR="$(mktemp -d /private/tmp/snipsprig-signing-test.XXXXXX)"
trap 'rm -rf "$WORK_DIR"' EXIT
for variant in first second; do
    APP="$WORK_DIR/$variant.app"
    mkdir -p "$APP/Contents/MacOS"
    rustc packaging/macos/tests/signing.rs --cfg "${variant}_build" \
        -o "$APP/Contents/MacOS/snipsprig"
    cp packaging/macos/Info.plist "$APP/Contents/Info.plist"
    bash packaging/macos/signing.sh --sign "$APP"
done
requirement() {
    codesign -d -r- "$1" 2>&1 | awk '/^designated => / {sub(/^designated => /, ""); print}'
}
FIRST_REQUIREMENT="$(requirement "$WORK_DIR/first.app")"
[[ "$FIRST_REQUIREMENT" == "$(requirement "$WORK_DIR/second.app")" ]]
FIRST_HASH="$(codesign -d --verbose=4 "$WORK_DIR/first.app" 2>&1 | awk '/^CDHash=/ {print}')"
SECOND_HASH="$(codesign -d --verbose=4 "$WORK_DIR/second.app" 2>&1 | awk '/^CDHash=/ {print}')"
[[ "$FIRST_HASH" != "$SECOND_HASH" ]]
codesign --verify --strict -R "$FIRST_REQUIREMENT" "$WORK_DIR/second.app"
# An unrelated ad-hoc signature with the same bundle ID must not impersonate an update.
codesign --force --sign - --identifier io.github.rensilin.snipsprig "$WORK_DIR/second.app"
if codesign --verify --strict -R "$FIRST_REQUIREMENT" "$WORK_DIR/second.app" 2>/dev/null; then
    printf 'FAIL: an unsigned identity impersonated a certificate-signed update.\n' >&2
    exit 1
fi
printf 'PASS: different builds retain the same authenticated identity; impostors fail.\n'
