.section .rodata
.align 3
foo:
    .quad 11
.align 3
bar:
    .quad 13

.section .data
.align 3
a:
    .quad 17
.align 3
b:
    .quad 19

.section .bss
.align 3
x:
    .skip 8
.align 3
y:
    .skip 8

.section .text
.global _start

_start:
    adrp x0, foo
    ldr x0, [x0, :lo12:foo]
    sub x0, x0, #1
    adrp x1, a
    str x0, [x1, :lo12:a]

    adrp x0, bar
    ldr x0, [x0, :lo12:bar]
    add x0, x0, #1
    adrp x1, b
    str x0, [x1, :lo12:b]

    adrp x0, a
    ldr x0, [x0, :lo12:a]
    adrp x1, b
    ldr x1, [x1, :lo12:b]
    add x0, x0, x1
    adrp x2, x
    str x0, [x2, :lo12:x]

    adrp x0, x
    ldr x0, [x0, :lo12:x]
    adrp x1, y
    str x0, [x1, :lo12:y]

    adrp x0, y
    ldr x0, [x0, :lo12:y]
    mov x8, #93
    svc #0
