.intel_syntax noprefix

.global foo
.global bar
.global a
.global b
.global x
.global y
.global dec
.global inc

.section .rodata
    foo: .quad 11           # read-only global variable with value 11
    bar: .quad 13           # read-only global variable with value 13

.section .data
    a: .quad 17             # read-write global variable with initial value 17
    b: .quad 19             # read-write global variable with initial value 19

.section .bss
    .align 8
    x: .zero 8              # uninitialized global variable (8 bytes)
    y: .zero 8              # uninitialized global variable (8 bytes)

.section .text

# fn dec(int64_t) -> int64_t
dec:
    # decrement the argument by 1 and return the result
    mov rax, rdi
    sub rax, 1
    ret

# fn inc(int64_t) -> int64_t
inc:
    # increment the argument by 1 and return the result
    mov rax, rdi
    add rax, 1
    ret
