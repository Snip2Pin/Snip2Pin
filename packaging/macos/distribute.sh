#!/usr/bin/env bash
set -euo pipefail
[[ $# -eq 2 ]] || { printf 'Usage: distribute.sh CANDIDATE_DIR OUTPUT_DIR\n' >&2; exit 2; }
: "${SNIP2PIN_SIGN_IDENTITY:?Select the local Developer ID identity}"
: "${SNIP2PIN_NOTARY_PROFILE:?Select the local notarytool keychain profile}"
CANDIDATE="$(cd "$1" && pwd)"
mkdir -p "$2"
OUTPUT="$(cd "$2" && pwd)"
cargo xtask verify "$CANDIDATE"
MANIFESTS=("$CANDIDATE"/*apple-darwin.json)
[[ ${#MANIFESTS[@]} -eq 1 && -f "${MANIFESTS[0]}" ]]
FILENAME="$(plutil -extract file raw -o - "${MANIFESTS[0]}")"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/snip2pin-distribution.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT
ditto -x -k "$CANDIDATE/$FILENAME" "$WORK"
APP="$WORK/Snip2Pin.app"
codesign --force --options runtime --timestamp --sign "$SNIP2PIN_SIGN_IDENTITY" "$APP"
codesign --verify --strict \
  -R '=anchor apple generic and certificate leaf[field.1.2.840.113635.100.6.1.13] exists' "$APP"
ditto -c -k --sequesterRsrc --keepParent "$APP" "$WORK/notarize.zip"
xcrun notarytool submit "$WORK/notarize.zip" --keychain-profile "$SNIP2PIN_NOTARY_PROFILE" --wait
xcrun stapler staple "$APP"
xcrun stapler validate "$APP"
spctl --assess --type execute "$APP"
ARCHIVE="$OUTPUT/${FILENAME/-unsigned.zip/.zip}"
[[ ! -e "$ARCHIVE" ]] || { printf 'Refusing to replace an archive.\n' >&2; exit 1; }
ditto -c -k --sequesterRsrc --keepParent "$APP" "$ARCHIVE"
cargo xtask finalize "$CANDIDATE" "$ARCHIVE"
