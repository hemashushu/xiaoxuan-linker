#!/usr/bin/env bash

# initialize the build environment
AS=/usr/bin/riscv64-linux-gnu-as
LD=/usr/bin/riscv64-linux-gnu-ld
QEMU_USER=/usr/bin/qemu-riscv64
GCC=/usr/bin/riscv64-linux-gnu-gcc

set -euxo pipefail

# Clean up old object files and executables
rm ./*.o ./*.elf || true

# Compile assembly files to object files
$AS -o minimal.o minimal.s
$AS -o function.o function.s
$AS -o data.o data.s
$AS -o symbol-export.o symbol-export.s
$AS -o symbol-import.o symbol-import.s
$AS -o override-weak.o override-weak.s
$AS -o override-strong.o override-strong.s
$AS -o relocate-within-data.o relocate-within-data.s

# Link object files to executables
$LD -o minimal.elf minimal.o
$LD -o function.elf function.o
$LD -o data.elf data.o
$LD -o symbol.elf symbol-export.o symbol-import.o
$LD -o override.elf override-weak.o override-strong.o
$LD -o relocate-within-data.elf relocate-within-data.o

# Compile C files to object files
$GCC -c -O0 -fno-pie -o relocate-within-data-tls.o relocate-within-data-tls.c
$GCC -c -O0 -ftls-model=local-exec -o tls.o tls.c
$GCC -c -O0 -ftls-model=global-dynamic -o tls-gd.o tls.c
$GCC -c -O0 -o pie-export.o pie-export.c
$GCC -c -O0 -o pie-import.o pie-import.c

# Link object files to executables
$GCC -O0 -o relocate-within-data-tls.elf relocate-within-data-tls.c
$GCC -O0 -ftls-model=local-exec -o tls.elf tls.c
$GCC -O0 -ftls-model=global-dynamic -o tls-gd.elf tls.c
$GCC -O0 -o pie.elf pie-export.o pie-import.o