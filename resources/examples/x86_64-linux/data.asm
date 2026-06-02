;; Program summary:
;; - Exit with status code 24.

default rel                 ;; use RIP-relative addressing by default for position-independent code

global _start

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
    foo dq 11               ;; read-only global variable with value 11
    bar dq 13               ;; read-only global variable with value 13

section .data
    a dq 17                 ;; read-write global variable with initial value 17
    b dq 19                 ;; read-write global variable with initial value 19

section .bss
    x resq 1                ;; uninitialized global variable (8 bytes)
    y resq 1                ;; uninitialized global variable (8 bytes)

section .text

;; fn _start() -> void
_start:
    ;; read `foo` and subtract 1, then store the result in `a`.
    ;; (`a` should be 10 after this)
    mov rax, [foo]
    sub rax, 1
    mov [a], rax

    ;; read `bar` and add 1, then store the result in `b`
    ;; (`b` should be 14 after this)
    mov rax, [bar]
    add rax, 1
    mov [b], rax

    ;; read `a` and `b`, add them together, and store the result in `x`
    ;; (`x` should be 24 after this)
    mov rax, [a]
    mov rbx, [b]
    add rax, rbx
    mov [x], rax

    ;; copy `x` to `y`
    ;; (`y` should be 24 after this)
    mov rax, [x]
    mov [y], rax

    ;; read `y` and exit with the value of `y` as the status code
    ;;
    ;; exit program using syscall `exit(status)`
    ;; syscall number: 60
    mov rdi, [y]
    mov rax, 60
    syscall

