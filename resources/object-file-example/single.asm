;; A simple NASM assembly program that defines a global variable,
;; a function to increment it, and a main entry point that calls the function and exits with the result.

global _start

section .data
    num dq 41           ;; read-write global variable with initial value 41

section .text

;; fn inc() -> int64_t
inc:
    mov rax, [rel num]  ;; read the value of num into rax
    add rax, 1          ;; increment rax by 1
    ret                 ;; return the incremented value in rax

;; fn _start() -> void
_start:
    call inc            ;; call the inc function, result is in rax

    ; syscall call `exit(status)`
    ; syscall number: 60

    mov rdi, rax        ;; move the result of inc (the incremented value) into rdi, which is the first argument to the syscall
    mov rax, 60         ;; syscall number for exit
    syscall
