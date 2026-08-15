# Program summary:
# - Exit with status code 24.

.globl dec
.globl inc
.globl _start

.section .data
    .align 8
foo: .8byte 11
    .align 8
bar: .8byte 13
    .align 8
pdec: .8byte dec
    .align 8
pinc: .8byte inc
.section .rodata
    .align 8
pfoo: .8byte foo
    .align 8
pbar: .8byte bar

.text
dec:
    aghi %r2, -1
    br %r14
inc:
    aghi %r2, 1
    br %r14

_start:
    larl %r4, pfoo
    lg %r4, 0(%r4)
    lg %r2, 0(%r4)
    larl %r5, pdec
    lg %r5, 0(%r5)
    basr %r14, %r5
    stg %r2, 0(%r4)
    larl %r4, pbar
    lg %r4, 0(%r4)
    lg %r2, 0(%r4)
    larl %r5, pinc
    lg %r5, 0(%r5)
    basr %r14, %r5
    stg %r2, 0(%r4)
    larl %r4, foo
    lg %r2, 0(%r4)
    larl %r4, bar
    lg %r3, 0(%r4)
    agr %r2, %r3
    lghi %r1, 1
    svc 0
