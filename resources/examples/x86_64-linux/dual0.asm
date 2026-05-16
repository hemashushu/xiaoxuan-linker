default rel     ;; use RIP-relative addressing by default for position-independent code

global msg
global len
global left
global right
global foo
global bar
global inc
global dec

;; data types:
;; - dq: data quadword (8 bytes)
;; - dd: data doubleword (4 bytes)
;; - dw: data word (2 bytes)
;; - db: data byte (1 byte)
;;
;; uninitialized global variables (BSS section) can be defined with:
;; - `resq`: reserve quadword (8 bytes)
;; - `resd`: reserve doubleword (4 bytes)
;; - `resw`: reserve word (2 bytes)
;; - `resb`: reserve byte (1 byte)

section .rodata
    msg db "Hello", 10, 0   ;; read-only global variable with a string value
    len dq 6                ;; read-only global variable with value 6

section .data
    left dq 11              ;; read-write global variable with initial value 11
    right dq 17             ;; read-write global variable with initial value 17

section .bss
    foo resd 1              ;; uninitialized global variable (4 bytes)
    bar resd 1              ;; uninitialized global variable (4 bytes)

section .text

;; fn inc() -> int64_t
inc:
    mov rax, [rel left]     ;; read the value of left into rax
    add rax, 1              ;; increment rax by 1
    mov [rel foo], eax      ;; store the incremented value in foo (lower 32 bits of rax)
    ret                     ;; return the incremented value in rax

;; fn dec() -> int64_t
dec:
    mov rax, [rel right]    ;; read the value of right into rax
    sub rax, 1              ;; decrement rax by 1
    mov [rel bar], eax      ;; store the decremented value in bar (lower 32 bits of rax)
    ret                     ;; return the decremented value in rax

;; private fn test() -> int64_t
test:
    lea rdi, [rel msg]      ;; load the address of msg into rdi
    mov rsi, [rel len]      ;; load the value of len into rsi
    xor rax, rax            ;; clear rax (set to 0)
    ret