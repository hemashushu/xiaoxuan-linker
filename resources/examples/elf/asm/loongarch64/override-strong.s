.extern foo

.globl _start
.globl bar

bar:
    li.w $a0, 42
    ret

_start:
    bl foo
    move $s0, $a0
    bl bar
    add.d $a0, $a0, $s0
    li.w $a7, 93
    syscall 0
