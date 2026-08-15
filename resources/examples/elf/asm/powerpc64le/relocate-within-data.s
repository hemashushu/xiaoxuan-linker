# Program summary:
# - Exit with status code 24.

.abiversion 2
.globl dec
.globl inc
.globl _start

.section .data
    .align 3
foo: .8byte 11
    .align 3
bar: .8byte 13
    .align 3
pdec: .8byte dec
    .align 3
pinc: .8byte inc
.section .rodata
    .align 3
pfoo: .8byte foo
    .align 3
pbar: .8byte bar

.text
dec:
    addi 3, 3, -1
    blr
inc:
    addi 3, 3, 1
    blr

_start:
    lis 4, pfoo@highest
    ori 4, 4, pfoo@higher
    sldi 4, 4, 32
    oris 4, 4, pfoo@h
    ori 4, 4, pfoo@l
    ld 4, 0(4)
    ld 3, 0(4)
    lis 5, pdec@highest
    ori 5, 5, pdec@higher
    sldi 5, 5, 32
    oris 5, 5, pdec@h
    ori 5, 5, pdec@l
    ld 5, 0(5)
    mtctr 5
    bctrl
    std 3, 0(4)

    lis 4, pbar@highest
    ori 4, 4, pbar@higher
    sldi 4, 4, 32
    oris 4, 4, pbar@h
    ori 4, 4, pbar@l
    ld 4, 0(4)
    ld 3, 0(4)
    lis 5, pinc@highest
    ori 5, 5, pinc@higher
    sldi 5, 5, 32
    oris 5, 5, pinc@h
    ori 5, 5, pinc@l
    ld 5, 0(5)
    mtctr 5
    bctrl
    std 3, 0(4)

    lis 4, foo@highest
    ori 4, 4, foo@higher
    sldi 4, 4, 32
    oris 4, 4, foo@h
    ori 4, 4, foo@l
    ld 3, 0(4)
    lis 4, bar@highest
    ori 4, 4, bar@higher
    sldi 4, 4, 32
    oris 4, 4, bar@h
    ori 4, 4, bar@l
    ld 4, 0(4)
    add 3, 3, 4
    li 0, 1
    sc
