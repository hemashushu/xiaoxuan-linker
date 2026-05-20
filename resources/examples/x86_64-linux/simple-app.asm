default rel     ;; use RIP-relative addressing by default for position-independent code

global _start

extern msg
extern len
extern left
extern right
extern foo
extern bar
extern inc
extern dec

section .text

;; fn _start() -> void
_start:
    ;; read the values of global variables for testing purposes, but we will not use them
    mov rbx, [rel left]
    mov rbx, [rel right]
    mov ebx, [rel foo]
    mov ebx, [rel bar]

    ;; print msg string using syscall `write(fd, buf, count)`
    ;; syscall number: 1

    mov rdi, 1          ;; file descriptor for stdout
    lea rsi, [rel msg]  ;; pointer to the string to write
    mov rdx, [rel len]  ;; number of bytes to write (length of "Hello\0")
    mov rax, 1          ;; syscall number for write
    syscall

    ;; calculate inc() + dec() and exit with the result as status code

    xor rbx, rbx            ;; set rbx to 0

    call inc                ;; call inc(), result is in rax, value is 12
    add rax, rbx            ;; sum original num and inc() result, value is 12

    mov rbx, rax

    call dec                ;; call dec(), result is in rax, value is 16
    add rax, rbx            ;; sum previous result and dec() result, value is 28

    ;; exit program using syscall `exit(status)`
    ;; syscall number: 60

    mov rdi, rax        ;; move summed result into rdi (exit status)
    mov rax, 60         ;; syscall number for exit
    syscall