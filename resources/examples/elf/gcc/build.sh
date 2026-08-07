#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

ARCH="${1:-}"
if [[ -z "$ARCH" ]]; then
    echo "Usage: $0 <ARCH>"
    echo "ARCH: aarch64 | riscv64 | x86_64 | all"
    exit 1
fi

case "$ARCH" in
    aarch64|riscv64|x86_64|all)
        ;;
    *)
        echo "Unsupported ARCH: $ARCH" >&2
        exit 1
        ;;
esac

select_gcc() {
    local arch="$1"
    case "$arch" in
        aarch64)
            GCC=/usr/bin/aarch64-linux-gnu-gcc
            [[ -x "$GCC" ]] || GCC=gcc
            CFLAGS_ARCH=()
            LDFLAGS_ARCH=()
            ;;
        riscv64)
            GCC=/usr/bin/riscv64-linux-gnu-gcc
            [[ -x "$GCC" ]] || GCC=gcc
            CFLAGS_ARCH=(-mno-relax)
            LDFLAGS_ARCH=(-Wl,--no-relax)
            ;;
        x86_64)
            GCC=/usr/bin/x86_64-linux-gnu-gcc
            [[ -x "$GCC" ]] || GCC=gcc
            CFLAGS_ARCH=()
            LDFLAGS_ARCH=()
            ;;
    esac
}

build_arch() {
    local arch="$1"
    local out_dir="$SCRIPT_DIR/$arch"

    select_gcc "$arch"

    mkdir -p "$out_dir"
    rm -f "$out_dir"/*.o "$out_dir"/*.elf "$out_dir"/*.so || true

    "$GCC" -std=c23 -c -O0 -fno-pic -fno-pie "${CFLAGS_ARCH[@]}" -o "$out_dir/minimal.o" "$SCRIPT_DIR/minimal.c"
    "$GCC" -std=c23 -c -O0 -fno-pic -fno-pie "${CFLAGS_ARCH[@]}" -o "$out_dir/function.o" "$SCRIPT_DIR/function.c"
    "$GCC" -std=c23 -c -O0 -fno-pic -fno-pie "${CFLAGS_ARCH[@]}" -o "$out_dir/data.o" "$SCRIPT_DIR/data.c"
    "$GCC" -std=c23 -c -O0 -fno-pic -fno-pie "${CFLAGS_ARCH[@]}" -o "$out_dir/symbol-export.o" "$SCRIPT_DIR/symbol-export.c"
    "$GCC" -std=c23 -c -O0 -fno-pic -fno-pie "${CFLAGS_ARCH[@]}" -o "$out_dir/symbol-import.o" "$SCRIPT_DIR/symbol-import.c"
    "$GCC" -std=c23 -c -O0 -fno-pic -fno-pie "${CFLAGS_ARCH[@]}" -o "$out_dir/override-weak.o" "$SCRIPT_DIR/override-weak.c"
    "$GCC" -std=c23 -c -O0 -fno-pic -fno-pie "${CFLAGS_ARCH[@]}" -o "$out_dir/override-strong.o" "$SCRIPT_DIR/override-strong.c"
    "$GCC" -std=c23 -c -O0 -fno-pic -fno-pie "${CFLAGS_ARCH[@]}" -o "$out_dir/relocate-within-data.o" "$SCRIPT_DIR/relocate-within-data.c"

    "$GCC" "${LDFLAGS_ARCH[@]}" -nostdlib -static -no-pie -o "$out_dir/minimal.elf" "$out_dir/minimal.o"
    "$GCC" "${LDFLAGS_ARCH[@]}" -nostdlib -static -no-pie -o "$out_dir/function.elf" "$out_dir/function.o"
    "$GCC" "${LDFLAGS_ARCH[@]}" -nostdlib -static -no-pie -o "$out_dir/data.elf" "$out_dir/data.o"
    "$GCC" "${LDFLAGS_ARCH[@]}" -nostdlib -static -no-pie -o "$out_dir/symbol.elf" "$out_dir/symbol-export.o" "$out_dir/symbol-import.o"
    "$GCC" "${LDFLAGS_ARCH[@]}" -nostdlib -static -no-pie -o "$out_dir/override.elf" "$out_dir/override-weak.o" "$out_dir/override-strong.o"
    "$GCC" "${LDFLAGS_ARCH[@]}" -nostdlib -static -no-pie -o "$out_dir/relocate-within-data.elf" "$out_dir/relocate-within-data.o"

    "$GCC" -std=c23 -c -O0 -fno-pic -fPIE -ftls-model=local-exec "${CFLAGS_ARCH[@]}" -o "$out_dir/tls.o" "$SCRIPT_DIR/tls.c"
    "$GCC" "${LDFLAGS_ARCH[@]}" -ftls-model=local-exec -o "$out_dir/tls.elf" "$out_dir/tls.o"

    "$GCC" -std=c23 -c -O0 -ftls-model=global-dynamic "${CFLAGS_ARCH[@]}" -o "$out_dir/tls-gd.o" "$SCRIPT_DIR/tls.c"
    "$GCC" "${LDFLAGS_ARCH[@]}" -ftls-model=global-dynamic -o "$out_dir/tls-gd.elf" "$out_dir/tls-gd.o"

    "$GCC" -std=c23 -c -O0 -fno-pic -fPIE "${CFLAGS_ARCH[@]}" -o "$out_dir/relocate-within-tls.o" "$SCRIPT_DIR/relocate-within-tls.c"
    "$GCC" "${LDFLAGS_ARCH[@]}" -o "$out_dir/relocate-within-tls.elf" "$out_dir/relocate-within-tls.o"

    "$GCC" -std=c23 -c -O0 -fPIC "${CFLAGS_ARCH[@]}" -o "$out_dir/share-export.o" "$SCRIPT_DIR/share-export.c"
    "$GCC" -std=c23 -c -O0 -fPIC "${CFLAGS_ARCH[@]}" -o "$out_dir/share-import.o" "$SCRIPT_DIR/share-import.c"
    "$GCC" "${LDFLAGS_ARCH[@]}" -shared -o "$out_dir/share-export.so" "$out_dir/share-export.o"
    "$GCC" "${LDFLAGS_ARCH[@]}" -o "$out_dir/share.elf" "$out_dir/share-import.o" -L"$out_dir" -Wl,-rpath,'$ORIGIN' -l:share-export.so
}

if [[ "$ARCH" == "all" ]]; then
    build_arch aarch64
    build_arch riscv64
    build_arch x86_64
else
    build_arch "$ARCH"
fi
