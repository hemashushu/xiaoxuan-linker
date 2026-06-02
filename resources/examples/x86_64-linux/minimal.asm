;; Program summary:
;; - Exit with status code 42.

default rel             ;; use RIP-relative addressing by default for position-independent code

global _start

section .text

;; fn _start() -> void
_start:
    mov rdi, 42         ;; exit status code
    mov rax, 60         ;; syscall number for exit
    syscall
