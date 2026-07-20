.option norvc                   # disable compressed instructions for clarity
.global foo
.global bar
.global a
.global b
.global x
.global y
.global dec
.global inc

.section .rodata
    .align 3
    foo: .quad 11               # read-only global variable with value 11
    .align 3
    bar: .quad 13               # read-only global variable with value 13

.section .data
    .align 3
    a: .quad 17                 # read-write global variable with initial value 17
    .align 3
    b: .quad 19                 # read-write global variable with initial value 19

.section .bss
    .align 3
    x: .space 8                 # uninitialized global variable (8 bytes)
    .align 3
    y: .space 8                 # uninitialized global variable (8 bytes)

.section .text

# fn dec(int64_t) -> int64_t
dec:
    # decrement the argument by 1 and return the result
    addi a0, a0, -1
    ret

# fn inc(int64_t) -> int64_t
inc:
    # increment the argument by 1 and return the result
    addi a0, a0, 1
    ret
