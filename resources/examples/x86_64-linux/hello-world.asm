default rel                 ;; use RIP-relative addressing by default for position-independent code

global _start

section .rodata
    msg db "Hello", 10, 0   ;; read-only global variable with a string value
    len dq 6                ;; read-only global variable with value 6

section .text

;; fn _start() -> void
_start:
    ;; print msg string using syscall `write(fd, buf, count)`
    ;; syscall number: 1

    mov rdi, 1              ;; file descriptor for stdout
    lea rsi, [rel msg]      ;; pointer to the string to write
    mov rdx, [rel len]      ;; number of bytes to write (length of "Hello\0")
    mov rax, 1              ;; syscall number for write
    syscall

    ;; exit program using syscall `exit(status)`
    ;; syscall number: 60

    xor rdi, rdi            ;; set exit status to 0
    mov rax, 60             ;; syscall number for exit
    syscall