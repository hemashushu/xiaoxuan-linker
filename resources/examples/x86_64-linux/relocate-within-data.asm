;; Program summary:
;; - Exit with status code 24.

default rel                         ;; use RIP-relative addressing by default for position-independent code

global foo
global bar
global dec
global inc
global _start

section .data
    foo dq 11                   ;; read-write global variable with initial value 11
    bar dq 13                   ;; read-write global variable with initial value 13
    pdec dq dec                 ;; pointer to dec (function pointer)
    pinc dq inc                 ;; pointer to inc (function pointer)

section .rodata
    pfoo dq foo                 ;; pointer to foo (data pointer)
    pbar dq bar                 ;; pointer to bar (data pointer)

section .text

;; fn dec(n: int64_t) -> int64_t
dec:
    mov rax, rdi                    ;; copy the first argument into rax
    sub rax, 1                      ;; decrement by 1
    ret                             ;; return rax

;; fn inc(n: int64_t) -> int64_t
inc:
    mov rax, rdi                    ;; copy the first argument into rax
    add rax, 1                      ;; increment by 1
    ret                             ;; return rax

;; fn _start() -> void
_start:

    ;; Read the value of `foo` by deferencing the pointer `pfoo` (in .rodata).
    mov rbx, [rel pfoo]
    mov rdi, [rbx]

    ;; Invoke `dec` via the function pointer `pdec` (in .data) with the value of `foo` as argument.
    mov rsi, [rel pdec]
    call rsi

    ;; Store the result of `dec(foo)` into `foo` (via pointer `pfoo`).
    ;; After this, `foo` should be 10 (11 - 1).
    mov [rbx], rax

    ;; Read the value of `bar` by dereferencing the pointer `pbar` (in .rodata).
    mov rbx, [rel pbar]
    mov rdi, [rbx]

    ;; Invoke `inc` via the function pointer `pinc` (in .data) with the value of `bar` as argument.
    ;; Call `inc(bar)` via dereferencing the pointer `pinc` directly.
    call [rel pinc]

    ;; Store the result of `inc(bar)` into `bar` (via pointer `pbar`).
    ;; After this, `bar` should be 14 (13 + 1).
    mov [rbx], rax

    ;; Read the updated values of `foo` and `bar` directly from memory (not via pointers), add them together.
    ;; The result should be 24 (10 + 14).
    mov rax, [rel foo]
    add rax, [rel bar]

    ;; Exit with the sum as the status code
    ;;
    ;; exit program using syscall `exit(status)`
    ;; syscall number: 60
    mov rdi, rax                    ;; move the final sum into rdi (exit status)
    mov rax, 60                     ;; syscall number for exit
    syscall
