#!/usr/bin/env bash
set -euxo pipefail

if [[ -x /usr/bin/riscv64-linux-gnu-as ]]; then
    AS=${AS:-/usr/bin/riscv64-linux-gnu-as}
else
    AS=${AS:-as}
fi

if [[ -x /usr/bin/riscv64-linux-gnu-ld ]]; then
    LD=${LD:-/usr/bin/riscv64-linux-gnu-ld}
else
    LD=${LD:-ld}
fi

if [[ -x /usr/bin/riscv64-linux-gnu-gcc ]]; then
    GCC=${GCC:-/usr/bin/riscv64-linux-gnu-gcc}
else
    GCC=${GCC:-gcc}
fi

# Clean up old object files and executables
rm ./asm/*.o ./asm/*.elf || true
rm ./gcc/*.o ./gcc/*.elf ./gcc/*.so || true

pushd asm

# Compile assembly files to object files
$AS -mno-relax -o minimal.o minimal.s
$AS -mno-relax -o function.o function.s
$AS -mno-relax -o data.o data.s
$AS -mno-relax -o symbol-export.o symbol-export.s
$AS -mno-relax -o symbol-import.o symbol-import.s
$AS -mno-relax -o override-weak.o override-weak.s
$AS -mno-relax -o override-strong.o override-strong.s
$AS -mno-relax -o relocate-within-data.o relocate-within-data.s

# Link object files to executables
$LD -no-pie -o minimal.elf minimal.o
$LD -no-pie -o function.elf function.o
$LD -no-pie -o data.elf data.o
$LD -no-pie -o symbol.elf symbol-export.o symbol-import.o
$LD -no-pie -o override.elf override-weak.o override-strong.o
$LD -no-pie -o relocate-within-data.elf relocate-within-data.o

popd
pushd gcc

# Compile C files to object files
$GCC -std=c23 -c -mno-relax -O0 -fno-pic -fno-pie -o minimal.o minimal.c
$GCC -std=c23 -c -mno-relax -O0 -fno-pic -fno-pie -o function.o function.c
$GCC -std=c23 -c -mno-relax -O0 -fno-pic -fno-pie -o data.o data.c
$GCC -std=c23 -c -mno-relax -O0 -fno-pic -fno-pie -o symbol-export.o symbol-export.c
$GCC -std=c23 -c -mno-relax -O0 -fno-pic -fno-pie -o symbol-import.o symbol-import.c
$GCC -std=c23 -c -mno-relax -O0 -fno-pic -fno-pie -o override-weak.o override-weak.c
$GCC -std=c23 -c -mno-relax -O0 -fno-pic -fno-pie -o override-strong.o override-strong.c
$GCC -std=c23 -c -mno-relax -O0 -fno-pic -fno-pie -o relocate-within-data.o relocate-within-data.c

# Link object files to executables
$GCC -Wl,--no-relax -nostdlib -static -no-pie -o minimal.elf minimal.o
$GCC -Wl,--no-relax -nostdlib -static -no-pie -o function.elf function.o
$GCC -Wl,--no-relax -nostdlib -static -no-pie -o data.elf data.o
$GCC -Wl,--no-relax -nostdlib -static -no-pie -o symbol.elf symbol-export.o symbol-import.o
$GCC -Wl,--no-relax -nostdlib -static -no-pie -o override.elf override-weak.o override-strong.o
$GCC -Wl,--no-relax -nostdlib -static -no-pie -o relocate-within-data.elf relocate-within-data.o

# Reference program: TLS
$GCC -std=c23 -c -mno-relax -O0 -fno-pic -fPIE -ftls-model=local-exec -o tls.o tls.c
$GCC -Wl,--no-relax -ftls-model=local-exec -o tls.elf tls.o

# Reference program: Global dynamic TLS
$GCC -std=c23 -c -mno-relax -O0 -ftls-model=global-dynamic -o tls-gd.o tls.c
$GCC -Wl,--no-relax -ftls-model=global-dynamic -o tls-gd.elf tls-gd.o

# Reference program: Relocate within TLS
$GCC -std=c23 -c -mno-relax -O0 -fno-pic -fPIE -o relocate-within-tls.o relocate-within-tls.c
$GCC -Wl,--no-relax -o relocate-within-tls.elf relocate-within-tls.o

# Reference program: Shared library
$GCC -std=c23 -c -mno-relax -O0 -fPIC -o share-export.o share-export.c
$GCC -std=c23 -c -mno-relax -O0 -fPIC -o share-import.o share-import.c
$GCC -Wl,--no-relax -shared -o share-export.so share-export.o
$GCC -Wl,--no-relax -o share.elf share-import.o -L. -Wl,-rpath,'$ORIGIN' -l:share-export.so

popd
