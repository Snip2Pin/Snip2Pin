#!/usr/bin/env bash
set -euo pipefail

PROJECT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$PROJECT_DIR"
SWIFT_BIN="$(xcrun --find swift)"
TESTING_PLUGIN="$(dirname "$SWIFT_BIN")/../lib/swift/host/plugins/testing/libTestingMacros.dylib"

# Some Command Line Tools releases include Testing but omit its macro plugin
# from SwiftPM's plugin search path. Use the toolchain's own plugin if present.
if [ -f "$TESTING_PLUGIN" ]; then
    swift test "$@" -Xswiftc -load-plugin-library -Xswiftc "$TESTING_PLUGIN"
else
    swift test "$@"
fi
