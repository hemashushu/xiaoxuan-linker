# Program summary:
# - Exit with status code 24.

.globl dec
.globl inc
.globl _start

.section .data
    .align 3
foo: .dword 11
    .align 3
bar: .dword 13
    .align 3
pdec: .dword dec
    .align 3
pinc: .dword inc

.section .rodata
    .align 3
pfoo: .dword foo
    .align 3
pbar: .dword bar

.text
dec:
    addi.d $a0, $a0, -1
    ret

inc:
    addi.d $a0, $a0, 1
    ret

_start:
    la.local $t0, pfoo
    ld.d $t0, $t0, 0
    ld.d $a0, $t0, 0
    la.local $t1, pdec
    ld.d $t1, $t1, 0
    jirl $ra, $t1, 0
    st.d $a0, $t0, 0

    la.local $t0, pbar
    ld.d $t0, $t0, 0
    ld.d $a0, $t0, 0
    la.local $t1, pinc
    ld.d $t1, $t1, 0
    jirl $ra, $t1, 0
    st.d $a0, $t0, 0

    la.local $t0, foo
    ld.d $a0, $t0, 0
    la.local $t0, bar
    ld.d $a1, $t0, 0
    add.d $a0, $a0, $a1
    li.w $a7, 93
    syscall 0
