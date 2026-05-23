#!/bin/env bash
set -euxo pipefail

rm ./*.o ./*.elf || true

nasm -f elf64 -o mini.o mini.asm
nasm -f elf64 -o simple-lib.o simple-lib.asm
nasm -f elf64 -o simple-app.o simple-app.asm
nasm -f elf64 -o hello-world.o hello-world.asm
nasm -f elf64 -o weak-symbol-lib.o weak-symbol-lib.asm
nasm -f elf64 -o weak-symbol-app.o weak-symbol-app.asm
nasm -f elf64 -o pointer-in-data.o pointer-in-data.asm
nasm -f elf64 -o custom-tls.o custom-tls.asm
gcc -c -O0 -fno-pie -o pointer-in-tls.o pointer-in-tls.c
gcc -c -O0 -ftls-model=local-exec -o tls.o tls.c
gcc -c -O0 -ftls-model=global-dynamic -o tls-gd.o tls.c
gcc -c -O0 -o dyn-external-lib.o dyn-external-lib.c
gcc -c -O0 -o dyn-external-app.o dyn-external-app.c

ld -o mini.elf mini.o
ld -o simple.elf simple-lib.o simple-app.o
ld -o hello-world.elf hello-world.o
ld -o weak-symbol.elf weak-symbol-lib.o weak-symbol-app.o
ld -o pointer-in-data.elf pointer-in-data.o
ld -o custom-tls.elf custom-tls.o
gcc -O0 -o pointer-in-tls.elf pointer-in-tls.c
gcc -O0 -ftls-model=local-exec -o tls.elf tls.c
gcc -O0 -ftls-model=global-dynamic -o tls-gd.elf tls.c
gcc -O0 -o dyn-external.elf dyn-external-lib.o dyn-external-app.o
