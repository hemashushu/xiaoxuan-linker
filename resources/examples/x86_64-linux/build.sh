#!/bin/env bash
set -e

rm ./*.o ./*.elf

nasm -f elf64 -o mini.o mini.asm
nasm -f elf64 -o simple-lib.o simple-lib.asm
nasm -f elf64 -o simple-app.o simple-app.asm
nasm -f elf64 -o hello.o hello.asm

ld -o mini.elf mini.o
ld -o simple.elf simple-lib.o simple-app.o
ld -o hello.elf hello.o
