.section .rodata
.align 3
foo:
    .quad 11
.align 3
bar:
    .quad 13
.align 3
pfoo:
    .quad foo
.align 3
pbar:
    .quad bar

.section .data
.align 3
a:
    .quad 17
.align 3
b:
    .quad 19
.align 3
pdec:
    .quad dec
.align 3
pinc:
    .quad inc

.section .bss
.align 3
x:
    .skip 8
.align 3
y:
    .skip 8

.section .text
.global dec
.global inc
.global _start

dec:
    sub x0, x0, #1
    ret

inc:
    add x0, x0, #1
    ret

_start:
    adrp x2, pfoo
    ldr x2, [x2, :lo12:pfoo]
    ldr x0, [x2]

    adrp x3, pdec
    ldr x3, [x3, :lo12:pdec]
    blr x3
    str x0, [x2]

    adrp x4, pbar
    ldr x4, [x4, :lo12:pbar]
    ldr x0, [x4]

    adrp x5, pinc
    ldr x5, [x5, :lo12:pinc]
    blr x5
    str x0, [x4]

    adrp x0, foo
    ldr x0, [x0, :lo12:foo]
    adrp x1, bar
    ldr x1, [x1, :lo12:bar]
    add x0, x0, x1
    mov x8, #93
    svc #0
