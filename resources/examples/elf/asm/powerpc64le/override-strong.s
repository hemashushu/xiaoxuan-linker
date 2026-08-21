.abiversion 2
.extern foo

.globl _start
.globl bar

bar:
    li 3, 42
    blr

_start:
    bl foo
    mr 31, 3
    bl bar
    add 3, 3, 31
    li 0, 1
    sc
