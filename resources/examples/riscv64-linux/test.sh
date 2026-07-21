#!/usr/bin/env bash

set -euo pipefail

HOST_ARCH="$(uname -m)"

RUN_MODE="native"
RUNNER=""
RUNNER_ARGS=()

if [[ "$HOST_ARCH" != "riscv64" ]]; then
    RUN_MODE="qemu"
    RUNNER=${QEMU_USER:-/usr/bin/qemu-riscv64}

    if [[ ! -x "$RUNNER" ]]; then
        RUNNER=qemu-riscv64
    fi

    if [[ -x /usr/bin/riscv64-linux-gnu-gcc ]]; then
        GCC=/usr/bin/riscv64-linux-gnu-gcc
        SYSROOT="$($GCC -print-sysroot)"
        RUNNER_ARGS=(-L "$SYSROOT")
    fi
fi

fail_count=0

run_case() {
    local elf="$1"
    local expected_exit="$2"
    local expected_stdout="$3"

    if [[ ! -f "$elf" ]]; then
        echo "[FAIL] $elf: file not found"
        fail_count=$((fail_count + 1))
        return
    fi

    local stdout_file stderr_file
    stdout_file="$(mktemp)"
    stderr_file="$(mktemp)"

    local actual_exit
    set +e
    if [[ "$RUN_MODE" == "native" ]]; then
        "./$elf" >"$stdout_file" 2>"$stderr_file"
        actual_exit=$?
    else
        "$RUNNER" "${RUNNER_ARGS[@]}" "./$elf" >"$stdout_file" 2>"$stderr_file"
        actual_exit=$?
    fi
    set -e

    local actual_stdout
    actual_stdout="$(cat "$stdout_file")"

    if [[ "$actual_exit" != "$expected_exit" ]]; then
        echo "[FAIL] $elf: exit code mismatch"
        echo "       expected: $expected_exit"
        echo "       actual:   $actual_exit"
        if [[ -s "$stderr_file" ]]; then
            echo "       stderr:"
            sed 's/^/         /' "$stderr_file"
        fi
        fail_count=$((fail_count + 1))
    elif [[ "$actual_stdout" != "$expected_stdout" ]]; then
        echo "[FAIL] $elf: stdout mismatch"
        echo "       expected: [$expected_stdout]"
        echo "       actual:   [$actual_stdout]"
        if [[ -s "$stderr_file" ]]; then
            echo "       stderr:"
            sed 's/^/         /' "$stderr_file"
        fi
        fail_count=$((fail_count + 1))
    else
        echo "[PASS] $elf"
    fi

    rm -f "$stdout_file" "$stderr_file"
}

run_case "asm/minimal.elf" 42 ""
run_case "asm/function.elf" 0 "Hello, world!"
run_case "asm/data.elf" 24 ""
run_case "asm/symbol.elf" 24 ""
run_case "asm/override.elf" 53 ""
run_case "asm/relocate-within-data.elf" 24 ""

run_case "gcc/minimal.elf" 42 ""
run_case "gcc/function.elf" 0 "Hello, world!"
run_case "gcc/data.elf" 24 ""
run_case "gcc/symbol.elf" 24 ""
run_case "gcc/override.elf" 53 ""
run_case "gcc/relocate-within-data.elf" 24 ""
run_case "gcc/relocate-within-data-no-pie.elf" 24 ""

run_case "gcc/relocate-within-tls.elf" 126 ""
run_case "gcc/relocate-within-tls-no-pie.elf" 126 ""
run_case "gcc/tls.elf" 66 ""
run_case "gcc/tls-gd.elf" 66 ""
run_case "gcc/share.elf" 199 ""

if [[ $fail_count -ne 0 ]]; then
    echo ""
    echo "Total failures: $fail_count"
    exit 1
fi

echo ""
echo "All tests passed."