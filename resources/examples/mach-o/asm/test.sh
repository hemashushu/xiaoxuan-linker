#!/usr/bin/env bash
set -euo pipefail

# Ported from resources/examples/elf/asm/test.sh.
#
# Only aarch64 (arm64) is supported, since macOS on Apple Silicon only runs
# native arm64 code.

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

ARCH="${1:-aarch64}"
if [[ "$ARCH" != "aarch64" ]]; then
    echo "Unsupported ARCH: $ARCH (only aarch64/Apple Silicon is supported)" >&2
    exit 1
fi

fail_count=0

run_case() {
    local name="$1"
    local expected_exit="$2"
    local expected_stdout="$3"
    local bin_path="$SCRIPT_DIR/aarch64/$name"

    if [[ ! -f "$bin_path" ]]; then
        echo "[FAIL] $name: file not found"
        fail_count=$((fail_count + 1))
        return
    fi

    local stdout_file stderr_file
    stdout_file="$(mktemp)"
    stderr_file="$(mktemp)"

    local actual_exit
    set +e
    "$bin_path" >"$stdout_file" 2>"$stderr_file"
    actual_exit=$?
    set -e

    local actual_stdout
    actual_stdout="$(cat "$stdout_file")"

    if [[ "$actual_exit" != "$expected_exit" ]]; then
        echo "[FAIL] $name: exit code mismatch"
        echo "  expected: $expected_exit"
        echo "  actual:   $actual_exit"
        if [[ -s "$stderr_file" ]]; then
            echo "  stderr:"
            sed 's/^/    /' "$stderr_file"
        fi
        fail_count=$((fail_count + 1))
    elif [[ "$actual_stdout" != "$expected_stdout" ]]; then
        echo "[FAIL] $name: stdout mismatch"
        echo "  expected: [$expected_stdout]"
        echo "  actual:   [$actual_stdout]"
        fail_count=$((fail_count + 1))
    else
        echo "[PASS] $name"
    fi

    rm -f "$stdout_file" "$stderr_file"
}

run_case minimal.macho 42 ""
run_case function.macho 0 "Hello, world!"
run_case data.macho 24 ""
run_case symbol.macho 24 ""
run_case override.macho 53 ""
run_case relocate-within-data.macho 24 ""

if [[ $fail_count -ne 0 ]]; then
    echo
    echo "Total failures: $fail_count"
    exit 1
fi

echo
echo "All tests passed."
