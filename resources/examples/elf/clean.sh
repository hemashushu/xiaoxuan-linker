#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

delete_outputs() {
    local output_dir="$1"
    if [[ -d "$output_dir" ]]; then
        rm -f "$output_dir"/*.o "$output_dir"/*.elf "$output_dir"/*.so || true
    fi
}

delete_outputs "$SCRIPT_DIR/asm/aarch64"
delete_outputs "$SCRIPT_DIR/asm/loongarch64"
delete_outputs "$SCRIPT_DIR/asm/powerpc64le"
delete_outputs "$SCRIPT_DIR/asm/riscv64"
delete_outputs "$SCRIPT_DIR/asm/s390x"
delete_outputs "$SCRIPT_DIR/asm/x86_64"

delete_outputs "$SCRIPT_DIR/gcc/aarch64"
delete_outputs "$SCRIPT_DIR/gcc/loongarch64"
delete_outputs "$SCRIPT_DIR/gcc/powerpc64le"
delete_outputs "$SCRIPT_DIR/gcc/riscv64"
delete_outputs "$SCRIPT_DIR/gcc/s390x"
delete_outputs "$SCRIPT_DIR/gcc/x86_64"
