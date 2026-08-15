.abiversion 2
.globl foo
.weak foo
.globl bar
.weak bar

foo:
    li 3, 11
    blr
bar:
    li 3, 13
    blr
