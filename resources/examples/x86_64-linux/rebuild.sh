#!/bin/env bash
set -euxo pipefail

# Clean up old object files and executables
rm ./*.o ./*.elf || true

# Compile assembly files to object files
nasm -f elf64 -o minimal.o minimal.asm
nasm -f elf64 -o function.o function.asm
nasm -f elf64 -o data.o data.asm
nasm -f elf64 -o symbol-export.o symbol-export.asm
nasm -f elf64 -o symbol-import.o symbol-import.asm
nasm -f elf64 -o override-weak.o override-weak.asm
nasm -f elf64 -o override-strong.o override-strong.asm
nasm -f elf64 -o relocate-data.o relocate-data.asm

# Link object files to executables
ld -o minimal.elf minimal.o
ld -o function.elf function.o
ld -o data.elf data.o
ld -o symbol.elf symbol-export.o symbol-import.o
ld -o override.elf override-weak.o override-strong.o
ld -o relocate-data.elf relocate-data.o

# Compile C files to object files
gcc -c -O0 -fno-pie -o relocate-data-tls.o relocate-data-tls.c
gcc -c -O0 -ftls-model=local-exec -o tls.o tls.c
gcc -c -O0 -ftls-model=global-dynamic -o tls-gd.o tls.c
gcc -c -O0 -o pie-export.o pie-export.c
gcc -c -O0 -o pie-import.o pie-import.c

# Link object files to executables
gcc -O0 -o relocate-data-tls.elf relocate-data-tls.c
gcc -O0 -ftls-model=local-exec -o tls.elf tls.c
gcc -O0 -ftls-model=global-dynamic -o tls-gd.elf tls.c
gcc -O0 -o pie.elf pie-export.o pie-import.o
