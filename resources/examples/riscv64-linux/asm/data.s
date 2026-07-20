# Program summary:
# - Exit with status code 24.

.option norvc                   # disable compressed instructions for clarity
.global _start

# data types:
# - .quad: data quadword (8 bytes)
# - .long: data longword (4 bytes)
# - .short: data halfword (2 bytes)
# - .byte: data byte (1 byte)
#
# uninitialized global variables (BSS section) can be defined with:
# - .bss / .lcomm: block started by symbol

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

# fn _start() -> void
_start:
    # Setup gp (global pointer) for accessing data
    la gp, __global_pointer$

    # read `foo` and subtract 1, then store the result in `a`.
    # (`a` should be 10 after this)
    la a4, foo
    ld a0, 0(a4)
    addi a0, a0, -1
    la a5, a
    sd a0, 0(a5)

    # read `bar` and add 1, then store the result in `b`
    # (`b` should be 14 after this)
    la a4, bar
    ld a0, 0(a4)
    addi a0, a0, 1
    la a5, b
    sd a0, 0(a5)

    # read `a` and `b`, add them together, and store the result in `x`
    # (`x` should be 24 after this)
    la a4, a
    ld a0, 0(a4)
    la a5, b
    ld a1, 0(a5)
    add a0, a0, a1
    la a4, x
    sd a0, 0(a4)

    # copy `x` to `y`
    # (`y` should be 24 after this)
    la a4, x
    ld a0, 0(a4)
    la a4, y
    sd a0, 0(a4)

    # read `y` and exit with the value of `y` as the status code
    #
    # exit program using syscall `exit(status)`
    # syscall number: 93
    la a4, y
    ld a0, 0(a4)
    li a7, 93
    ecall
