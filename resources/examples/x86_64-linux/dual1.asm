;; Entry point that imports symbols and exits with num + inc().

global _start
extern num
extern left
extern right
extern inc
extern dec

section .text

;; fn _start() -> void
_start:
    mov rbx, [rel left]     ;; read original num value into rbx, value is 11, but we will not use it
    mov rbx, [rel right]    ;; read original num value into rbx, value is 17, but we will not use it

    mov rbx, [rel num]      ;; read original num value into rbx, value is 100

    call inc                ;; call inc(), result is in rax, value is 12
    add rax, rbx            ;; sum original num and inc() result, value is 112

    mov rbx, rax
    call dec                ;; call dec(), result is in rax, value is 16
    add rax, rbx            ;; sum previous result and dec() result, value is 128

    ; syscall call `exit(status)`
    ; syscall number: 60

    mov rdi, rax        ;; move summed result into rdi (exit status)
    mov rax, 60         ;; syscall number for exit
    syscall