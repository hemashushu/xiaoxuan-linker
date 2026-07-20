.option norvc                   # disable compressed instructions for clarity
.extern foo
.extern bar
.global _start

.section .text

# Override the weak symbol `bar` with a strong symbol.
#
# ```c
# int bar() {
#    return 42;
# }
# ```
bar:
    li a0, 42
    ret

_start:
    la gp, __global_pointer$

    # Call `foo`
    # Now `a0` should be 11 (the value returned by `foo`).
    call foo
    mv s0, a0                   # save result in s0 (callee-saved register)

    # Call `bar`
    # Now `a0` should be 42 (the value returned by `bar`).
    call bar

    # Sum their results.
    # Now `a0` should be 53 (11 + 42).
    add a0, a0, s0

    # Exit with the sum as the status code
    #
    # Exit program using syscall `exit(status)`
    # syscall number: 93
    li a7, 93
    ecall
