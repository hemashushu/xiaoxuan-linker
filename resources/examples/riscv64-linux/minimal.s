# Program summary:
# - Exit with status code 42.

.option norvc                   # disable compressed instructions for clarity
.global _start

.section .text

# fn _start() -> void
_start:
    li a0, 42                   # exit status code in a0
    li a7, 93                   # syscall number for exit
    ecall
