// Program summary:
// - Exit with status code 42.

.section .text
.global _start

// fn _start() -> void
_start:
    mov x0, #42      // exit status code in x0
    mov x8, #93      // syscall number for exit
    svc #0
