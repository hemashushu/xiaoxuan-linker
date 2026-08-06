#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
HOST_ARCH="$(uname -m)"

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

fail_count=0

configure_runner() {
    local target_arch="$1"

    RUN_MODE="native"
    RUNNER=""
    RUNNER_ARGS=()

    if [[ "$HOST_ARCH" == "$target_arch" ]]; then
        return
    fi

    RUN_MODE="qemu"

    case "$target_arch" in
        aarch64)
            RUNNER=${QEMU_USER:-/usr/bin/qemu-aarch64}
            [[ -x "$RUNNER" ]] || RUNNER=qemu-aarch64
            if [[ -x /usr/bin/aarch64-linux-gnu-gcc ]]; then
                SYSROOT="$(/usr/bin/aarch64-linux-gnu-gcc -print-sysroot)"
                RUNNER_ARGS=(-L "$SYSROOT")
            fi
            ;;
        riscv64)
            RUNNER=${QEMU_USER:-/usr/bin/qemu-riscv64}
            [[ -x "$RUNNER" ]] || RUNNER=qemu-riscv64
            if [[ -x /usr/bin/riscv64-linux-gnu-gcc ]]; then
                SYSROOT="$(/usr/bin/riscv64-linux-gnu-gcc -print-sysroot)"
                RUNNER_ARGS=(-L "$SYSROOT")
            fi
            ;;
        x86_64)
            RUNNER=${QEMU_USER:-/usr/bin/qemu-x86_64}
            [[ -x "$RUNNER" ]] || RUNNER=qemu-x86_64
            if [[ -x /usr/bin/x86_64-linux-gnu-gcc ]]; then
                SYSROOT="$(/usr/bin/x86_64-linux-gnu-gcc -print-sysroot)"
                RUNNER_ARGS=(-L "$SYSROOT")
            fi
            ;;
    esac
}

run_case() {
    local arch="$1"
    local elf_name="$2"
    local expected_exit="$3"
    local expected_stdout="$4"
    local elf_path="$SCRIPT_DIR/$arch/$elf_name"

    if [[ ! -f "$elf_path" ]]; then
        echo "[FAIL][$arch] $elf_name: file not found"
        fail_count=$((fail_count + 1))
        return
    fi

    local stdout_file stderr_file
    stdout_file="$(mktemp)"
    stderr_file="$(mktemp)"

    local actual_exit
    set +e
    if [[ "$RUN_MODE" == "native" ]]; then
        "$elf_path" >"$stdout_file" 2>"$stderr_file"
        actual_exit=$?
    else
        "$RUNNER" "${RUNNER_ARGS[@]}" "$elf_path" >"$stdout_file" 2>"$stderr_file"
        actual_exit=$?
    fi
    set -e

    local actual_stdout
    actual_stdout="$(cat "$stdout_file")"

    if [[ "$actual_exit" != "$expected_exit" ]]; then
        echo "[FAIL][$arch] $elf_name: exit code mismatch"
        echo "  expected: $expected_exit"
        echo "  actual:   $actual_exit"
        if [[ -s "$stderr_file" ]]; then
            echo "  stderr:"
            sed 's/^/    /' "$stderr_file"
        fi
        fail_count=$((fail_count + 1))
    elif [[ "$actual_stdout" != "$expected_stdout" ]]; then
        echo "[FAIL][$arch] $elf_name: stdout mismatch"
        echo "  expected: [$expected_stdout]"
        echo "  actual:   [$actual_stdout]"
        if [[ -s "$stderr_file" ]]; then
            echo "  stderr:"
            sed 's/^/    /' "$stderr_file"
        fi
        fail_count=$((fail_count + 1))
    else
        echo "[PASS][$arch] $elf_name"
    fi

    rm -f "$stdout_file" "$stderr_file"
}

test_arch() {
    local arch="$1"
    configure_runner "$arch"

    run_case "$arch" minimal.elf 42 ""
    run_case "$arch" function.elf 0 "Hello, world!"
    run_case "$arch" data.elf 24 ""
    run_case "$arch" symbol.elf 24 ""
    run_case "$arch" override.elf 53 ""
    run_case "$arch" relocate-within-data.elf 24 ""
}

if [[ "$ARCH" == "all" ]]; then
    test_arch aarch64
    test_arch riscv64
    test_arch x86_64
else
    test_arch "$ARCH"
fi

if [[ $fail_count -ne 0 ]]; then
    echo
    echo "Total failures: $fail_count"
    exit 1
fi

echo
echo "All tests passed."
