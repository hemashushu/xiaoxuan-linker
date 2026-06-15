#!/bin/env bash
set -euxo pipefail

# Clean up old object files and executables
rm ./*.o ./*.elf || true

# Compile assembly files to object files
as -o minimal.o minimal.s
as -o function.o function.s
as -o data.o data.s
as -o symbol-export.o symbol-export.s
as -o symbol-import.o symbol-import.s
as -o override-weak.o override-weak.s
as -o override-strong.o override-strong.s
as -o relocate-within-data.o relocate-within-data.s

# Link object files to executables
ld -o minimal.elf minimal.o
ld -o function.elf function.o
ld -o data.elf data.o
ld -o symbol.elf symbol-export.o symbol-import.o
ld -o override.elf override-weak.o override-strong.o
ld -o relocate-within-data.elf relocate-within-data.o

# Compile C files to object files
gcc -c -O0 -fno-pie -o relocate-within-data-tls.o relocate-within-data-tls.c
gcc -c -O0 -ftls-model=local-exec -o tls.o tls.c
gcc -c -O0 -ftls-model=global-dynamic -o tls-gd.o tls.c
gcc -c -O0 -o pie-export.o pie-export.c
gcc -c -O0 -o pie-import.o pie-import.c

# Link object files to executables
gcc -O0 -o relocate-within-data-tls.elf relocate-within-data-tls.c
gcc -O0 -ftls-model=local-exec -o tls.elf tls.c
gcc -O0 -ftls-model=global-dynamic -o tls-gd.elf tls.c
gcc -O0 -o pie.elf pie-export.o pie-import.o
