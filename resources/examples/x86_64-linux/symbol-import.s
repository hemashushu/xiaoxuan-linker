# Program summary:
# - Exit with status code 24.

.intel_syntax noprefix

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
    # read `foo` and subtract 1 (by function `dec`), then store the result in `a`.
    # (`a` should be 10 after this)
    mov rdi, qword ptr [rip + foo]
    call dec
    mov qword ptr [rip + a], rax

    # read `bar` and add 1 (by function `inc`), then store the result in `b`
    # (`b` should be 14 after this)
    mov rdi, qword ptr [rip + bar]
    call inc
    mov qword ptr [rip + b], rax

    # read `a` and `b`, add them together, and store the result in `x`
    # (`x` should be 24 after this)
    mov rax, qword ptr [rip + a]
    mov rbx, qword ptr [rip + b]
    add rax, rbx
    mov qword ptr [rip + x], rax

    # copy `x` to `y`
    # (`y` should be 24 after this)
    mov rax, qword ptr [rip + x]
    mov qword ptr [rip + y], rax

    # read `y` and exit with the value of `y` as the status code
    #
    # exit program using syscall `exit(status)`
    # syscall number: 60

    mov rdi, qword ptr [rip + y] # move result into rdi (exit status)
    mov rax, 60                  # syscall number for exit
    syscall
