.section .text
.global _start

.extern foo
.extern bar
.extern a
.extern b
.extern x
.extern y
.extern dec
.extern inc

_start:
    adrp x0, foo
    ldr x0, [x0, :lo12:foo]
    bl dec
    adrp x1, a
    str x0, [x1, :lo12:a]

    adrp x0, bar
    ldr x0, [x0, :lo12:bar]
    bl inc
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
