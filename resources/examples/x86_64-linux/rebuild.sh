#!/usr/bin/env bash
set -euxo pipefail

NASM=${NASM:-/usr/bin/nasm}
if [[ ! -x "$NASM" ]]; then
	NASM=${NASM:-nasm}
fi

if [[ -x /usr/bin/x86_64-linux-gnu-ld ]]; then
	LD=${LD:-/usr/bin/x86_64-linux-gnu-ld}
else
	LD=${LD:-ld}
fi

if [[ -x /usr/bin/x86_64-linux-gnu-gcc ]]; then
	GCC=${GCC:-/usr/bin/x86_64-linux-gnu-gcc}
else
	GCC=${GCC:-gcc}
fi

# Clean up old object files and executables
rm ./*.o ./*.elf || true

# Compile assembly files to object files
$NASM -f elf64 -o minimal.o minimal.asm
$NASM -f elf64 -o function.o function.asm
$NASM -f elf64 -o data.o data.asm
$NASM -f elf64 -o symbol-export.o symbol-export.asm
$NASM -f elf64 -o symbol-import.o symbol-import.asm
$NASM -f elf64 -o override-weak.o override-weak.asm
$NASM -f elf64 -o override-strong.o override-strong.asm
$NASM -f elf64 -o relocate-within-data.o relocate-within-data.asm

# Link object files to executables
$LD -o minimal.elf minimal.o
$LD -o function.elf function.o
$LD -o data.elf data.o
$LD -o symbol.elf symbol-export.o symbol-import.o
$LD -o override.elf override-weak.o override-strong.o
$LD -o relocate-within-data.elf relocate-within-data.o

# Compile C files to object files
$GCC -c -O0 -o gcc.o gcc.c
$GCC -c -O0 -o relocate-within-data-tls.o relocate-within-data-tls.c
$GCC -c -O0 -fno-pie -o relocate-within-data-tls-no-pie.o relocate-within-data-tls.c
$GCC -c -O0 -ftls-model=local-exec -o tls.o tls.c
$GCC -c -O0 -ftls-model=global-dynamic -o tls-gd.o tls.c
$GCC -c -O0 -o pie-export.o pie-export.c
$GCC -c -O0 -o pie-import.o pie-import.c

# Link object files to executables
$GCC -O0 -o gcc.elf gcc.o
$GCC -O0 -o relocate-within-data-tls.elf relocate-within-data-tls.o
$GCC -O0 -ftls-model=local-exec -o tls.elf tls.o
$GCC -O0 -ftls-model=global-dynamic -o tls-gd.elf tls-gd.o
$GCC -O0 -o pie.elf pie-export.o pie-import.o
