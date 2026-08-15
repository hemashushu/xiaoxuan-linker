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
    la.local $t0, foo
    ld.d $a0, $t0, 0
    bl dec
    la.local $t1, a
    st.d $a0, $t1, 0

    la.local $t0, bar
    ld.d $a0, $t0, 0
    bl inc
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
