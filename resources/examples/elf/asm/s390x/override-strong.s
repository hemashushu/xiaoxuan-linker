.extern foo
.extern bar
.globl _start

bar:
    lghi %r2, 42
    br %r14

_start:
    brasl %r14, foo
    lgr %r5, %r2
    brasl %r14, bar
    agr %r2, %r5
    lghi %r1, 1
    svc 0
