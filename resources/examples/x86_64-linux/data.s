# Program summary:
# - Exit with status code 24.

.intel_syntax noprefix

.global _start

# data types:
# - .quad: data quadword (8 bytes)
# - .long: data doubleword (4 bytes)
# - .word: data word (2 bytes)
# - .byte: data byte (1 byte)

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

# fn _start() -> void
_start:
    # read `foo` and subtract 1, then store the result in `a`.
    # (`a` should be 10 after this)
    mov rax, qword ptr [rip + foo]
    sub rax, 1
    mov qword ptr [rip + a], rax

    # read `bar` and add 1, then store the result in `b`
    # (`b` should be 14 after this)
    mov rax, qword ptr [rip + bar]
    add rax, 1
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
    mov rdi, qword ptr [rip + y]
    mov rax, 60
    syscall

