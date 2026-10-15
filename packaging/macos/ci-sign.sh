#!/usr/bin/env bash
# GitHub-hosted runners only. No local credential discovery or persistent imports.
set -euo pipefail
[[ $# -eq 2 && "${GITHUB_ACTIONS:-}" == true && "${RUNNER_ENVIRONMENT:-}" == github-hosted ]] || {
  printf 'CI signing requires a GitHub-hosted runner and candidate/output paths.\n' >&2
  exit 2
}
for name in MACOS_CERTIFICATE_P12_BASE64 MACOS_CERTIFICATE_PASSWORD MACOS_NOTARY_KEY_P8 MACOS_NOTARY_KEY_ID MACOS_NOTARY_ISSUER_ID MACOS_CERTIFICATE_SHA1; do
  [[ -n "${!name:-}" ]] || { printf 'Missing signing configuration: %s\n' "$name" >&2; exit 1; }
done
[[ "$MACOS_CERTIFICATE_SHA1" =~ ^[a-fA-F0-9]{40}$ ]] || { printf 'Invalid certificate fingerprint.\n' >&2; exit 1; }
umask 077
WORK="$(mktemp -d "$RUNNER_TEMP/snip2pin-signing.XXXXXX")"
KEYCHAIN="$WORK/signing.keychain-db"
cleanup() {
  status=$?
  if [[ -f "$KEYCHAIN" ]]; then security delete-keychain "$KEYCHAIN" || status=1; fi
  rm -rf "$WORK" || status=1
  exit "$status"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
KEYCHAIN_PASSWORD="$(openssl rand -hex 32)"
printf '::add-mask::%s\n' "$KEYCHAIN_PASSWORD"
printf '%s' "$MACOS_CERTIFICATE_P12_BASE64" | base64 --decode -o "$WORK/certificate.p12"
# notarytool reads the key file directly; never print its contents.
printf '%s' "$MACOS_NOTARY_KEY_P8" | /usr/bin/tee "$WORK/notary.p8" >/dev/null
security create-keychain -p "$KEYCHAIN_PASSWORD" "$KEYCHAIN"
security set-keychain-settings -lut 3600 "$KEYCHAIN"
security unlock-keychain -p "$KEYCHAIN_PASSWORD" "$KEYCHAIN"
security import "$WORK/certificate.p12" -P "$MACOS_CERTIFICATE_PASSWORD" -k "$KEYCHAIN" -T /usr/bin/codesign -T /usr/bin/security
security set-key-partition-list -S apple-tool:,apple:,codesign: -k "$KEYCHAIN_PASSWORD" "$KEYCHAIN" >/dev/null
security find-identity -v -p codesigning "$KEYCHAIN" | /usr/bin/grep -Fqi "$MACOS_CERTIFICATE_SHA1" || {
  printf 'Expected signing identity is not present.\n' >&2; exit 1;
}
xcrun notarytool store-credentials snip2pin-ci --key "$WORK/notary.p8" \
  --key-id "$MACOS_NOTARY_KEY_ID" --issuer "$MACOS_NOTARY_ISSUER_ID" --keychain "$KEYCHAIN"
export SNIP2PIN_SIGN_IDENTITY="$MACOS_CERTIFICATE_SHA1"
export SNIP2PIN_SIGN_KEYCHAIN="$KEYCHAIN"
export SNIP2PIN_NOTARY_PROFILE=snip2pin-ci
bash packaging/macos/distribute.sh "$1" "$2"
