.section .rodata
.align 3
.global foo
.global bar
foo:
    .quad 11
.align 3
bar:
    .quad 13

.section .data
.align 3
.global a
.global b
a:
    .quad 17
.align 3
b:
    .quad 19

.section .bss
.align 3
.global x
.global y
x:
    .skip 8
.align 3
y:
    .skip 8

.section .text
.global dec
.global inc

dec:
    sub x0, x0, #1
    ret

inc:
    add x0, x0, #1
    ret
