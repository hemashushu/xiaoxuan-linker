.option norvc                   # disable compressed instructions for clarity
.global foo
.global bar
.global dec
.global inc
.global _start

.section .data
    .align 3
    foo: .quad 11               # read-write global variable with initial value 11
    .align 3
    bar: .quad 13               # read-write global variable with initial value 13
    .align 3
    pdec: .quad dec             # pointer to dec (function pointer)
    .align 3
    pinc: .quad inc             # pointer to inc (function pointer)

.section .rodata
    .align 3
    pfoo: .quad foo             # pointer to foo (data pointer)
    .align 3
    pbar: .quad bar             # pointer to bar (data pointer)

.section .text

# fn dec(n: int64_t) -> int64_t
dec:
    addi a0, a0, -1             # decrement by 1
    ret                         # return a0

# fn inc(n: int64_t) -> int64_t
inc:
    addi a0, a0, 1              # increment by 1
    ret                         # return a0

# fn _start() -> void
_start:
    la gp, __global_pointer$

    # Read the value of `foo` by dereferencing the pointer `pfoo` (in .rodata).
    la a4, pfoo
    ld s0, 0(a4)                # s0 now contains the address of foo (use callee-saved register)
    ld a0, 0(s0)                # a0 now contains the value of foo

    # Invoke `dec` via the function pointer `pdec` (in .data) with the value of `foo` as argument.
    la a4, pdec
    ld a4, 0(a4)
    jalr ra, 0(a4)

    # Store the result of `dec(foo)` into `foo` (via pointer `pfoo`).
    # After this, `foo` should be 10 (11 - 1).
    sd a0, 0(s0)

    # Read the value of `bar` by dereferencing the pointer `pbar` (in .rodata).
    la a4, pbar
    ld s1, 0(a4)                # s1 now contains the address of bar (use callee-saved register)
    ld a0, 0(s1)                # a0 now contains the value of bar

    # Invoke `inc` via the function pointer `pinc` (in .data) with the value of `bar` as argument.
    # Call `inc(bar)` via dereferencing the pointer `pinc` directly.
    la a4, pinc
    ld a4, 0(a4)
    jalr ra, 0(a4)

    # Store the result of `inc(bar)` into `bar` (via pointer `pbar`).
    # After this, `bar` should be 14 (13 + 1).
    sd a0, 0(s1)

    # Read the updated values of `foo` and `bar` directly from memory (not via pointers), add them together.
    # The result should be 24 (10 + 14).
    la a4, foo
    ld a0, 0(a4)
    la a4, bar
    ld a1, 0(a4)
    add a0, a0, a1

    # Exit with the sum as the status code
    #
    # exit program using syscall `exit(status)`
    # syscall number: 93
    li a7, 93
    ecall
