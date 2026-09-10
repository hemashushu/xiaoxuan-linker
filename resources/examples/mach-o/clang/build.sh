#!/usr/bin/env bash
set -euo pipefail

# Ported from resources/examples/elf/gcc/build.sh.
#
# Only aarch64 (arm64) is supported, since macOS on Apple Silicon only runs
# native arm64 code. `clang` (Xcode command line tools) replaces `gcc`, and
# `ld`/dyld replace the ELF linker/dynamic loader.

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SDK="$(xcrun --sdk macosx --show-sdk-path)"
# Match the deployment target to the host SDK to avoid ld's version-mismatch warning.
MIN_VERSION="$(xcrun --sdk macosx --show-sdk-version)"

ARCH="${1:-aarch64}"
if [[ "$ARCH" != "aarch64" ]]; then
    echo "Unsupported ARCH: $ARCH (only aarch64/Apple Silicon is supported)" >&2
    exit 1
fi

OUT_DIR="$SCRIPT_DIR/aarch64"

CFLAGS_COMMON=(-arch arm64 -isysroot "$SDK" -mmacosx-version-min="$MIN_VERSION" -std=c23 -O0)
LD_COMMON=(-arch arm64 -platform_version macos "$MIN_VERSION" "$MIN_VERSION" -syslibroot "$SDK" -lSystem)

mkdir -p "$OUT_DIR"
rm -f "$OUT_DIR"/*.o "$OUT_DIR"/*.macho "$OUT_DIR"/*.dylib || true

# Freestanding programs with a custom `start` entry point (mangled to
# `_start` by the compiler), built without the standard C runtime/libc
# startup files -- analogous to the ELF version's `-nostdlib -static -no-pie`.
clang "${CFLAGS_COMMON[@]}" -c -fno-pic -fno-pie -nostdlib -o "$OUT_DIR/minimal.o" "$SCRIPT_DIR/minimal.c"
clang "${CFLAGS_COMMON[@]}" -c -fno-pic -fno-pie -nostdlib -o "$OUT_DIR/function.o" "$SCRIPT_DIR/function.c"
clang "${CFLAGS_COMMON[@]}" -c -fno-pic -fno-pie -nostdlib -o "$OUT_DIR/data.o" "$SCRIPT_DIR/data.c"
clang "${CFLAGS_COMMON[@]}" -c -fno-pic -fno-pie -nostdlib -o "$OUT_DIR/symbol-export.o" "$SCRIPT_DIR/symbol-export.c"
clang "${CFLAGS_COMMON[@]}" -c -fno-pic -fno-pie -nostdlib -o "$OUT_DIR/symbol-import.o" "$SCRIPT_DIR/symbol-import.c"
clang "${CFLAGS_COMMON[@]}" -c -fno-pic -fno-pie -nostdlib -o "$OUT_DIR/override-weak.o" "$SCRIPT_DIR/override-weak.c"
clang "${CFLAGS_COMMON[@]}" -c -fno-pic -fno-pie -nostdlib -o "$OUT_DIR/override-strong.o" "$SCRIPT_DIR/override-strong.c"
clang "${CFLAGS_COMMON[@]}" -c -fno-pic -fno-pie -nostdlib -o "$OUT_DIR/relocate-within-data.o" "$SCRIPT_DIR/relocate-within-data.c"

ld "${LD_COMMON[@]}" -e _start -o "$OUT_DIR/minimal.macho" "$OUT_DIR/minimal.o"
ld "${LD_COMMON[@]}" -e _start -o "$OUT_DIR/function.macho" "$OUT_DIR/function.o"
ld "${LD_COMMON[@]}" -e _start -o "$OUT_DIR/data.macho" "$OUT_DIR/data.o"
ld "${LD_COMMON[@]}" -e _start -o "$OUT_DIR/symbol.macho" "$OUT_DIR/symbol-export.o" "$OUT_DIR/symbol-import.o"
ld "${LD_COMMON[@]}" -e _start -o "$OUT_DIR/override.macho" "$OUT_DIR/override-weak.o" "$OUT_DIR/override-strong.o"
ld "${LD_COMMON[@]}" -e _start -o "$OUT_DIR/relocate-within-data.macho" "$OUT_DIR/relocate-within-data.o"

# Thread-local storage examples rely on the TLV (Thread-Local Variable)
# runtime mechanism, which needs the normal dyld/libSystem process startup.
# Link them as regular `main`-based executables instead of using a custom
# `_start` entry point.
clang "${CFLAGS_COMMON[@]}" -o "$OUT_DIR/tls.macho" "$SCRIPT_DIR/tls.c"
clang "${CFLAGS_COMMON[@]}" -o "$OUT_DIR/relocate-within-tls.macho" "$SCRIPT_DIR/relocate-within-tls.c"

# Shared library example (dylib instead of ELF .so).
clang "${CFLAGS_COMMON[@]}" -c -fPIC -o "$OUT_DIR/share-export.o" "$SCRIPT_DIR/share-export.c"
clang "${CFLAGS_COMMON[@]}" -dynamiclib -install_name @rpath/libshare-export.dylib -o "$OUT_DIR/libshare-export.dylib" "$OUT_DIR/share-export.o"
clang "${CFLAGS_COMMON[@]}" -o "$OUT_DIR/share.macho" "$SCRIPT_DIR/share-import.c" -L"$OUT_DIR" -Wl,-rpath,@executable_path -lshare-export
