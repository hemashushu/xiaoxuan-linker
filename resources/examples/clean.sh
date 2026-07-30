#!/usr/bin/env bash
set -euxo pipefail

delete_outputs() {
    local output_dir="$1"
    if [[ -d "$output_dir" ]]; then
        rm -rf "$output_dir"/*.o "$output_dir"/*.elf "$output_dir"/*.so || true
    fi
}

delete_arch() {
    local arch="$1"
    delete_outputs "./$1/asm"
    delete_outputs "./$1/gcc"
}

delete_arch "x86_64-linux"
delete_arch "aarch64-linux"
delete_arch "riscv64-linux"