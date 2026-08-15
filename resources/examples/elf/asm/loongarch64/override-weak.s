.globl foo
.weak foo
.globl bar
.weak bar

foo:
    li.w $a0, 11
    ret

bar:
    li.w $a0, 13
    ret
