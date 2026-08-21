# Program summary:
# - Exit with status code 24.

.globl _start
.globl foo
.globl bar
.globl a
.globl b
.globl x
.globl y

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
_start:
    larl %r4, foo
    lg %r2, 0(%r4)
    aghi %r2, -1
    larl %r5, a
    stg %r2, 0(%r5)
    larl %r4, bar
    lg %r2, 0(%r4)
    aghi %r2, 1
    larl %r5, b
    stg %r2, 0(%r5)
    larl %r4, a
    lg %r2, 0(%r4)
    larl %r5, b
    lg %r3, 0(%r5)
    agr %r2, %r3
    larl %r4, x
    stg %r2, 0(%r4)
    larl %r4, x
    lg %r2, 0(%r4)
    larl %r5, y
    stg %r2, 0(%r5)
    larl %r4, y
    lg %r2, 0(%r4)
    lghi %r1, 1
    svc 0
