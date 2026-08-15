.abiversion 2
.globl foo
.globl bar
.globl a
.globl b
.globl x
.globl y
.globl dec
.globl inc

.section .rodata
    .align 3
foo: .8byte 11
    .align 3
bar: .8byte 13
.section .data
    .align 3
a: .8byte 17
    .align 3
b: .8byte 19
.section .bss
    .align 3
x: .space 8
    .align 3
y: .space 8

.text
dec:
    addi 3, 3, -1
    blr
inc:
    addi 3, 3, 1
    blr
