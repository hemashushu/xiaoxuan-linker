#!/usr/bin/env bash

set -euo pipefail

HOST_ARCH="$(uname -m)"

RUN_MODE="native"
RUNNER=""
RUNNER_ARGS=()

if [[ "$HOST_ARCH" != "x86_64" ]]; then
    RUN_MODE="qemu"
    RUNNER=${QEMU_USER:-/usr/bin/qemu-x86_64}

    if [[ ! -x "$RUNNER" ]]; then
        RUNNER=qemu-x86_64
    fi

    if [[ -x /usr/bin/x86_64-linux-gnu-gcc ]]; then
        GCC=/usr/bin/x86_64-linux-gnu-gcc
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

run_case "minimal.elf" 42 ""
run_case "function.elf" 0 "Hello, world!"
run_case "data.elf" 24 ""
run_case "symbol.elf" 24 ""
run_case "override.elf" 53 ""
run_case "relocate-within-data.elf" 24 ""

run_case "pie.elf" 199 ""
run_case "tls.elf" 66 ""
run_case "tls-gd.elf" 66 ""
run_case "relocate-within-data-tls.elf" 126 ""

if [[ $fail_count -ne 0 ]]; then
    echo ""
    echo "Total failures: $fail_count"
    exit 1
fi

echo ""
echo "All tests passed."