#!/bin/env bash
set -e

rm ./*.o ./*.elf || true

nasm -f elf64 -o mini.o mini.asm
nasm -f elf64 -o simple-lib.o simple-lib.asm
nasm -f elf64 -o simple-app.o simple-app.asm
nasm -f elf64 -o hello.o hello.asm
nasm -f elf64 -o weak-lib.o weak-lib.asm
nasm -f elf64 -o weak-app.o weak-app.asm
nasm -f elf64 -o pointer.o pointer.asm
gcc -c -fpic -o dyn-lib.o dyn-lib.c
gcc -c -fpic -o dyn-app.o dyn-app.c
gcc -c -O1 -ftls-model=local-exec -o tls.o tls.c

ld -o mini.elf mini.o
ld -o simple.elf simple-lib.o simple-app.o
ld -o hello.elf hello.o
ld -o weak.elf weak-lib.o weak-app.o
ld -o pointer.elf pointer.o
gcc -static -pie -O1 -ftls-model=local-exec -o tls.elf tls.c
gcc -static -o dyn.elf dyn-lib.o dyn-app.o
