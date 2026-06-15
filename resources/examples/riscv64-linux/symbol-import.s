.option norvc                   # disable compressed instructions for clarity
.global _start

.extern foo
.extern bar
.extern a
.extern b
.extern x
.extern y
.extern dec
.extern inc

.section .text

# fn _start() -> void
_start:
    la gp, __global_pointer$

    # read `foo` and subtract 1 (by function `dec`), then store the result in `a`.
    # (`a` should be 10 after this)
    la a4, foo
    ld a0, 0(a4)
    call dec
    la a5, a
    sd a0, 0(a5)

    # read `bar` and add 1 (by function `inc`), then store the result in `b`
    # (`b` should be 14 after this)
    la a4, bar
    ld a0, 0(a4)
    call inc
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
