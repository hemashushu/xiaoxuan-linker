.section .rodata
.align 3
.global foo
.global bar
foo:
    .quad 11                   // read-only global variable with value 11
.align 3
bar:
    .quad 13                   // read-only global variable with value 13

.section .data
.align 3
.global a
.global b
a:
    .quad 17                   // read-write global variable with initial value 17
.align 3
b:
    .quad 19                   // read-write global variable with initial value 19

.section .bss
.align 3
.global x
.global y
x:
    .skip 8                    // uninitialized global variable (8 bytes)
.align 3
y:
    .skip 8                    // uninitialized global variable (8 bytes)

.section .text
.global dec
.global inc

// fn dec(int64_t) -> int64_t
dec:
    sub x0, x0, #1             // decrement the argument by 1 and return the result
    ret

// fn inc(int64_t) -> int64_t
inc:
    add x0, x0, #1             // increment the argument by 1 and return the result
    ret
