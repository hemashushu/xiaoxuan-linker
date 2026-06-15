.weak foo
.weak bar

foo:
    mov x0, #11
    ret

bar:
    mov x0, #13
    ret
