;; Defines a global variable and a function to increment it.

global num
global left
global right
global inc
global dec

section .rodata
    num dq 100              ;; read-only global variable with value 100

section .data
    left dq 11              ;; read-write global variable with initial value 41
    right dq 17             ;; read-write global variable with initial value 42

section .text

;; fn inc() -> int64_t
inc:
    mov rax, [rel left]     ;; read the value of left into rax
    add rax, 1              ;; increment rax by 1
    ret                     ;; return the incremented value in rax

;; fn dec() -> int64_t
dec:
    mov rax, [rel right]    ;; read the value of right into rax
    sub rax, 1              ;; decrement rax by 1
    ret                     ;; return the decremented value in rax