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
    .align 3
foo: .dword 11
    .align 3
bar: .dword 13

.section .data
    .align 3
a: .dword 17
    .align 3
b: .dword 19

.section .bss
    .align 3
x: .space 8
    .align 3
y: .space 8

.text

_start:
    la.local $t0, foo
    ld.d $a0, $t0, 0
    addi.d $a0, $a0, -1
    la.local $t1, a
    st.d $a0, $t1, 0

    la.local $t0, bar
    ld.d $a0, $t0, 0
    addi.d $a0, $a0, 1
    la.local $t1, b
    st.d $a0, $t1, 0

    la.local $t0, a
    ld.d $a0, $t0, 0
    la.local $t1, b
    ld.d $a1, $t1, 0
    add.d $a0, $a0, $a1
    la.local $t0, x
    st.d $a0, $t0, 0

    la.local $t0, x
    ld.d $a0, $t0, 0
    la.local $t0, y
    st.d $a0, $t0, 0

    la.local $t0, y
    ld.d $a0, $t0, 0
    li.w $a7, 93
    syscall 0
