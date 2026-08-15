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
foo: .dword 11
    .align 3
bar: .dword 13

.section .data
    .align 3
a: .dword 17
    .align 3
b: .dword 19

.section .bss
    .align 3
x: .space 8
    .align 3
y: .space 8

.text
dec:
    addi.d $a0, $a0, -1
    ret

inc:
    addi.d $a0, $a0, 1
    ret
