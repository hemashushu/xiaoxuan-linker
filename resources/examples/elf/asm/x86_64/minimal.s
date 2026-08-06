# Program summary:
# - Exit with status code 42.

.intel_syntax noprefix

.global _start

.section .text

# fn _start() -> void
_start:
    mov rdi, 42         # exit status code
    mov rax, 60         # syscall number for exit
    syscall
