.globl foo
.globl bar
.globl a
.globl b
.globl x
.globl y
.globl dec
.globl inc

.section .rodata
    .align 8
foo: .8byte 11
    .align 8
bar: .8byte 13
.section .data
    .align 8
a: .8byte 17
    .align 8
b: .8byte 19
.section .bss
    .align 8
x: .space 8
    .align 8
y: .space 8

.text
dec:
    aghi %r2, -1
    br %r14
inc:
    aghi %r2, 1
    br %r14
