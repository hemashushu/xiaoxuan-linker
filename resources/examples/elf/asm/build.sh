#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

ARCH="${1:-}"
if [[ -z "$ARCH" ]]; then
    read -r -p "Enter ARCH (aarch64, riscv64, x86_64, all): " ARCH
fi

case "$ARCH" in
    aarch64|riscv64|x86_64|all)
        ;;
    *)
        echo "Unsupported ARCH: $ARCH" >&2
        exit 1
        ;;
esac

select_tools() {
    local arch="$1"
    case "$arch" in
        aarch64)
            AS=${AS:-/usr/bin/aarch64-linux-gnu-as}
            LD=${LD:-/usr/bin/aarch64-linux-gnu-ld}
            [[ -x "$AS" ]] || AS=as
            [[ -x "$LD" ]] || LD=ld
            AS_ARGS=()
            ;;
        riscv64)
            AS=${AS:-/usr/bin/riscv64-linux-gnu-as}
            LD=${LD:-/usr/bin/riscv64-linux-gnu-ld}
            [[ -x "$AS" ]] || AS=as
            [[ -x "$LD" ]] || LD=ld
            AS_ARGS=(-mno-relax)
            ;;
        x86_64)
            AS=${AS:-/usr/bin/x86_64-linux-gnu-as}
            LD=${LD:-/usr/bin/x86_64-linux-gnu-ld}
            [[ -x "$AS" ]] || AS=as
            [[ -x "$LD" ]] || LD=ld
            AS_ARGS=(--64)
            ;;
    esac
}

build_arch() {
    local arch="$1"
    local dir="$SCRIPT_DIR/$arch"

    select_tools "$arch"

    rm -f "$dir"/*.o "$dir"/*.elf || true

    pushd "$dir" >/dev/null

    "$AS" "${AS_ARGS[@]}" -o minimal.o minimal.s
    "$AS" "${AS_ARGS[@]}" -o function.o function.s
    "$AS" "${AS_ARGS[@]}" -o data.o data.s
    "$AS" "${AS_ARGS[@]}" -o symbol-export.o symbol-export.s
    "$AS" "${AS_ARGS[@]}" -o symbol-import.o symbol-import.s
    "$AS" "${AS_ARGS[@]}" -o override-weak.o override-weak.s
    "$AS" "${AS_ARGS[@]}" -o override-strong.o override-strong.s
    "$AS" "${AS_ARGS[@]}" -o relocate-within-data.o relocate-within-data.s

    "$LD" -no-pie -o minimal.elf minimal.o
    "$LD" -no-pie -o function.elf function.o
    "$LD" -no-pie -o data.elf data.o
    "$LD" -no-pie -o symbol.elf symbol-export.o symbol-import.o
    "$LD" -no-pie -o override.elf override-weak.o override-strong.o
    "$LD" -no-pie -o relocate-within-data.elf relocate-within-data.o

    popd >/dev/null
}

if [[ "$ARCH" == "all" ]]; then
    build_arch aarch64
    build_arch riscv64
    build_arch x86_64
else
    build_arch "$ARCH"
fi
