#!/usr/bin/env bash
set -euxo pipefail

if [[ -x /usr/bin/x86_64-linux-gnu-as ]]; then
    AS=${AS:-/usr/bin/x86_64-linux-gnu-as}
else
    AS=${AS:-as}
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
rm ./asm/*.o ./asm/*.elf || true
rm ./clang/*.o ./clang/*.so ./clang/*.elf || true

pushd asm

# Compile assembly files to object files
$AS --64 -o minimal.o minimal.s
$AS --64 -o function.o function.s
$AS --64 -o data.o data.s
$AS --64 -o symbol-export.o symbol-export.s
$AS --64 -o symbol-import.o symbol-import.s
$AS --64 -o override-weak.o override-weak.s
$AS --64 -o override-strong.o override-strong.s
$AS --64 -o relocate-within-data.o relocate-within-data.s

# Link object files to executables
$LD -o minimal.elf minimal.o
$LD -o function.elf function.o
$LD -o data.elf data.o
$LD -o symbol.elf symbol-export.o symbol-import.o
$LD -o override.elf override-weak.o override-strong.o
$LD -o relocate-within-data.elf relocate-within-data.o

popd
pushd gcc

# Compile C files to object files
$GCC -std=c23 -c -O0 -o minimal.o minimal.c
$GCC -std=c23 -c -O0 -o function.o function.c
$GCC -std=c23 -c -O0 -o data.o data.c
$GCC -std=c23 -c -O0 -o symbol-export.o symbol-export.c
$GCC -std=c23 -c -O0 -o symbol-import.o symbol-import.c
$GCC -std=c23 -c -O0 -o override-weak.o override-weak.c
$GCC -std=c23 -c -O0 -o override-strong.o override-strong.c
$GCC -std=c23 -c -O0 -o relocate-within-data.o relocate-within-data.c
$GCC -std=c23 -c -O0 -fno-pie -o relocate-within-data-no-pie.o relocate-within-data.c

# Link object files to executables
$GCC -O0 -nostdlib -static -o minimal.elf minimal.o
$GCC -O0 -nostdlib -static -o function.elf function.o
$GCC -O0 -nostdlib -static -o data.elf data.o
$GCC -O0 -nostdlib -static -o symbol.elf symbol-export.o symbol-import.o
$GCC -O0 -nostdlib -static -o override.elf override-weak.o override-strong.o
$GCC -O0 -nostdlib -static -o relocate-within-data.elf relocate-within-data.o
$GCC -O0 -nostdlib -static -no-pie -o relocate-within-data-no-pie.elf relocate-within-data-no-pie.o

# Other C programs that require stdlib

$GCC -c -O0 -o relocate-within-tls.o relocate-within-tls.c
$GCC -c -O0 -fno-pie -o relocate-within-tls-no-pie.o relocate-within-tls.c
$GCC -c -O0 -ftls-model=local-exec -o tls.o tls.c
$GCC -c -O0 -ftls-model=global-dynamic -o tls-gd.o tls.c
$GCC -c -O0 -fPIC -o share-export.o share-export.c # -fPIC generates a shared library with PLT/GOT
$GCC -c -O0 -fPIC -o share-import.o share-import.c # -fPIC generates a shared library with PLT/GOT

$GCC -O0 -o relocate-within-tls.elf relocate-within-tls.o
$GCC -O0 -no-pie -o relocate-within-tls-no-pie.elf relocate-within-tls-no-pie.o
$GCC -O0 -ftls-model=local-exec -o tls.elf tls.o
$GCC -O0 -ftls-model=global-dynamic -o tls-gd.elf tls-gd.o
$GCC -O0 -shared -o share-export.so share-export.o
$GCC -O0 -o share.elf share-import.o -L. -Wl,-rpath,'$ORIGIN' -l:share-export.so

popd