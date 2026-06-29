# Program summary:
# - Exit with status code 53.

.intel_syntax noprefix

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
    mov eax, 42
    ret

_start:
    # Call `foo`
    # Now `eax` should be 11 (the value returned by `foo`).
    call foo
    mov ebx, eax

    # Call `bar`
    # Now `eax` should be 42 (the value returned by `bar`).
    call bar

    # Sum their results.
    # Now `eax` should be 53 (11 + 42).
    add eax, ebx

    # Exit with the sum as the status code
    #
    # Exit program using syscall `exit(status)`
    # syscall number: 60
    mov edi, eax
    mov eax, 60
    syscall
