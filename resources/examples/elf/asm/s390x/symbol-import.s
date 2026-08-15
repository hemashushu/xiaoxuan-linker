.globl _start
.extern foo
.extern bar
.extern a
.extern b
.extern x
.extern y
.extern dec
.extern inc

.text
_start:
    larl %r4, foo
    lg %r2, 0(%r4)
    brasl %r14, dec
    larl %r5, a
    stg %r2, 0(%r5)
    larl %r4, bar
    lg %r2, 0(%r4)
    brasl %r14, inc
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
