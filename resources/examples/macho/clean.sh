#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

delete_outputs() {
    local output_dir="$1"
    if [[ -d "$output_dir" ]]; then
        rm -f "$output_dir"/*.o "$output_dir"/*.macho "$output_dir"/*.dylib || true
    fi
}

delete_outputs "$SCRIPT_DIR/asm/aarch64"
delete_outputs "$SCRIPT_DIR/clang/aarch64"
