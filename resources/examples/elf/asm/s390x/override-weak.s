.globl foo
.weak foo
.globl bar
.weak bar

foo:
    lghi %r2, 11
    br %r14
bar:
    lghi %r2, 13
    br %r14
