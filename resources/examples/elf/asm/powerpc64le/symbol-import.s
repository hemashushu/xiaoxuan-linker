.abiversion 2
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
    lis 4, foo@highest
    ori 4, 4, foo@higher
    sldi 4, 4, 32
    oris 4, 4, foo@h
    ori 4, 4, foo@l
    ld 3, 0(4)
    bl dec
    lis 5, a@highest
    ori 5, 5, a@higher
    sldi 5, 5, 32
    oris 5, 5, a@h
    ori 5, 5, a@l
    std 3, 0(5)

    lis 4, bar@highest
    ori 4, 4, bar@higher
    sldi 4, 4, 32
    oris 4, 4, bar@h
    ori 4, 4, bar@l
    ld 3, 0(4)
    bl inc
    lis 5, b@highest
    ori 5, 5, b@higher
    sldi 5, 5, 32
    oris 5, 5, b@h
    ori 5, 5, b@l
    std 3, 0(5)

    lis 4, a@highest
    ori 4, 4, a@higher
    sldi 4, 4, 32
    oris 4, 4, a@h
    ori 4, 4, a@l
    ld 3, 0(4)
    lis 5, b@highest
    ori 5, 5, b@higher
    sldi 5, 5, 32
    oris 5, 5, b@h
    ori 5, 5, b@l
    ld 6, 0(5)
    add 3, 3, 6
    lis 4, x@highest
    ori 4, 4, x@higher
    sldi 4, 4, 32
    oris 4, 4, x@h
    ori 4, 4, x@l
    std 3, 0(4)
    lis 4, x@highest
    ori 4, 4, x@higher
    sldi 4, 4, 32
    oris 4, 4, x@h
    ori 4, 4, x@l
    ld 3, 0(4)
    lis 5, y@highest
    ori 5, 5, y@higher
    sldi 5, 5, 32
    oris 5, 5, y@h
    ori 5, 5, y@l
    std 3, 0(5)
    lis 4, y@highest
    ori 4, 4, y@higher
    sldi 4, 4, 32
    oris 4, 4, y@h
    ori 4, 4, y@l
    ld 3, 0(4)
    li 0, 1
    sc
