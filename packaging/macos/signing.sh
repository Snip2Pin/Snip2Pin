#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SIGNING_DIR="${SNIPSPRIG_SIGNING_DIR:-$HOME/Library/Application Support/io.github.rensilin.SnipSprig/signing}"
KEYCHAIN="$SIGNING_DIR/identity.keychain-db"
CERTIFICATE="$SIGNING_DIR/certificate.pem"
PASSWORD_FILE="$SIGNING_DIR/password.bin"

password() {
    od -An -v -tx1 "$PASSWORD_FILE" | tr -d ' \n'
}

case "${1:-}" in
    --setup)
        if [[ -e "$KEYCHAIN" || -e "$CERTIFICATE" || -e "$PASSWORD_FILE" ]]; then
            [[ -f "$KEYCHAIN" && -f "$CERTIFICATE" && -f "$PASSWORD_FILE" ]] || {
                printf 'Signing setup is incomplete. Restore it; do not regenerate an existing identity.\n' >&2
                exit 1
            }
            printf 'Reusing the existing local signing identity.\n'
            exit 0
        fi
        umask 077
        mkdir -p "$SIGNING_DIR"
        chmod 700 "$SIGNING_DIR"
        WORK_DIR="$(mktemp -d "$SIGNING_DIR/setup.XXXXXX")"
        trap 'rm -rf "$WORK_DIR"' EXIT
        openssl rand -out "$PASSWORD_FILE" 32
        SIGNING_PASSWORD="$(password)"
        export SIGNING_PASSWORD
        openssl req -x509 -newkey rsa:3072 -sha256 -nodes -days 3650 \
            -config "$SCRIPT_DIR/signing.cnf" \
            -keyout "$WORK_DIR/key.pem" -out "$CERTIFICATE" 2>/dev/null
        openssl pkcs12 -export -in "$CERTIFICATE" -inkey "$WORK_DIR/key.pem" \
            -out "$WORK_DIR/identity.p12" -passout env:SIGNING_PASSWORD
        KEYCHAINS=()
        while IFS= read -r line; do
            line="${line#*\"}"
            KEYCHAINS+=("${line%\"*}")
        done < <(security list-keychains -d user)
        security create-keychain -p "$SIGNING_PASSWORD" "$KEYCHAIN"
        # Keep this private keychain out of the user's normal search list.
        security list-keychains -d user -s "${KEYCHAINS[@]}"
        security import "$WORK_DIR/identity.p12" -k "$KEYCHAIN" -P "$SIGNING_PASSWORD" \
            -x -T /usr/bin/codesign
        security set-key-partition-list -S apple-tool:,apple: -s \
            -k "$SIGNING_PASSWORD" "$KEYCHAIN" >/dev/null
        printf 'Local signing identity created in %s\n' "$SIGNING_DIR"
        ;;
    --sign)
        [[ $# -eq 2 ]] || { printf 'Usage: signing.sh --sign APP\n' >&2; exit 2; }
        if [[ -n "${SNIPSPRIG_SIGN_IDENTITY:-}" ]]; then
            [[ "$SNIPSPRIG_SIGN_IDENTITY" != '-' ]] || {
                printf 'Ad-hoc signing loses privacy permissions on updates. Use a certificate.\n' >&2
                exit 1
            }
            SIGN_ARGS=(--sign "$SNIPSPRIG_SIGN_IDENTITY")
            if [[ -n "${SNIPSPRIG_SIGN_KEYCHAIN:-}" ]]; then
                SIGN_ARGS+=(--keychain "$SNIPSPRIG_SIGN_KEYCHAIN")
            fi
        else
            [[ -f "$KEYCHAIN" && -f "$CERTIFICATE" && -f "$PASSWORD_FILE" ]] || {
                printf 'Create a stable local identity with: bash packaging/macos/signing.sh --setup\n' >&2
                exit 1
            }
            security unlock-keychain -p "$(password)" "$KEYCHAIN"
            IDENTITY="$(openssl x509 -in "$CERTIFICATE" -noout -fingerprint -sha1)"
            IDENTITY="${IDENTITY#*=}"
            SIGN_ARGS=(--sign "${IDENTITY//:/}" --keychain "$KEYCHAIN")
        fi
        codesign --force "${SIGN_ARGS[@]}" --identifier io.github.rensilin.snipsprig "$2"
        codesign --verify --strict "$2"
        ;;
    *)
        printf 'Usage: signing.sh --setup | --sign APP\n' >&2
        exit 2
        ;;
esac
