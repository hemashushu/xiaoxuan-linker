#!/usr/bin/env bash
set -euo pipefail

# Ported from resources/examples/elf/asm/build.sh.
#
# Only aarch64 (arm64) is supported, since macOS on Apple Silicon only runs
# native arm64 code (there is no Linux-style cross-compiler toolchain).

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SDK="$(xcrun --sdk macosx --show-sdk-path)"
# Match the deployment target to the host SDK to avoid ld's version-mismatch warning.
MIN_VERSION="$(xcrun --sdk macosx --show-sdk-version)"

ARCH="${1:-aarch64}"
if [[ "$ARCH" != "aarch64" ]]; then
    echo "Unsupported ARCH: $ARCH (only aarch64/Apple Silicon is supported)" >&2
    exit 1
fi

DIR="$SCRIPT_DIR/aarch64"

rm -f "$DIR"/*.o "$DIR"/*.macho || true

pushd "$DIR" >/dev/null

CFLAGS_COMMON=(-arch arm64 -isysroot "$SDK" -mmacosx-version-min="$MIN_VERSION")

as -arch arm64 -o minimal.o minimal.s
as -arch arm64 -o function.o function.s
as -arch arm64 -o data.o data.s
as -arch arm64 -o symbol-export.o symbol-export.s
as -arch arm64 -o symbol-import.o symbol-import.s
as -arch arm64 -o override-weak.o override-weak.s
as -arch arm64 -o override-strong.o override-strong.s
as -arch arm64 -o relocate-within-data.o relocate-within-data.s

clang "${CFLAGS_COMMON[@]}" -o minimal.macho minimal.o
clang "${CFLAGS_COMMON[@]}" -o function.macho function.o
clang "${CFLAGS_COMMON[@]}" -o data.macho data.o
clang "${CFLAGS_COMMON[@]}" -o symbol.macho symbol-export.o symbol-import.o
clang "${CFLAGS_COMMON[@]}" -o override.macho override-weak.o override-strong.o
clang "${CFLAGS_COMMON[@]}" -o relocate-within-data.macho relocate-within-data.o

popd >/dev/null
