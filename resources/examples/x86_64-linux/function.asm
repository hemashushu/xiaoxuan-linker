;; Program summary:
;; - Prints "Hello, world!\n" to stdout.
;; - Exit with status code 0.

default rel                     ;; use RIP-relative addressing by default for position-independent code

global _start

section .rodata
    hello db "Hello"            ;; read-only global variable with a string value
    hello_len dq 5              ;; read-only global variable with value 5
    world db ", world!", 10     ;; read-only global variable with a string value
    world_len dq 9              ;; read-only global variable with value 9

section .text

;; fn print_hello() -> void
print_hello:
    ;; print string using syscall `write(fd, buf, count)`
    ;; syscall number: 1

    mov rdi, 1                  ;; file descriptor for stdout
    lea rsi, [rel hello]        ;; pointer to the string to write
    mov rdx, [rel hello_len]    ;; number of bytes to write (length of "Hello,")
    mov rax, 1                  ;; syscall number for write
    syscall
    ret

;; fn print_world() -> void
print_world:
    ;; print string using syscall `write(fd, buf, count)`
    ;; syscall number: 1

    mov rdi, 1                  ;; file descriptor for stdout
    lea rsi, [rel world]        ;; pointer to the string to write
    mov rdx, [rel world_len]    ;; number of bytes to write (length of " World!\n")
    mov rax, 1                  ;; syscall number for write
    syscall
    ret

;; fn _start() -> void
_start:
    call print_hello
    call print_world

    ;; exit program using syscall `exit(status)`
    ;; syscall number: 60

    xor rdi, rdi            ;; set exit status to 0
    mov rax, 60             ;; syscall number for exit
    syscall