default rel                         ;; use RIP-relative addressing by default for position-independent code

global my_var
global my_func
global my_array
global _start

section .data
    my_var dq 42                    ;; read-write global variable with initial value 42

    my_array:
        dq my_var                   ;; element 0: pointer to my_var (data pointer)
        dq my_func                  ;; element 1: pointer to my_func (function pointer)

section .text

;; fn my_func(n: int64_t) -> int64_t
my_func:
    mov rax, rdi                    ;; copy the first argument into rax
    add rax, 1                      ;; increment by 1
    ret                             ;; return rax

;; fn _start() -> void
_start:
    sub rsp, 16                     ;; allocate 16 bytes for two local pointer variables

    ;; load pointers from my_array and store them in local variables on the stack
    mov rax, [rel my_array]         ;; load my_array[0] (pointer to my_var) into rax
    mov [rsp], rax                  ;; store pointer to my_var at local[0]
    mov rax, [rel my_array + 8]     ;; load my_array[1] (pointer to my_func) into rax
    mov [rsp + 8], rax              ;; store pointer to my_func at local[1]

    ;; dereference local[0] to get the value of my_var
    mov rcx, [rsp]                  ;; load the pointer to my_var from local[0]
    mov rdi, [rcx]                  ;; dereference: load the value of my_var (42) into rdi

    ;; call my_func through the function pointer stored in local[1]
    mov rax, [rsp + 8]              ;; load the function pointer from local[1]
    call rax                        ;; call my_func(42), result (43) returned in rax

    add rsp, 16                     ;; restore stack pointer

    ;; exit program using syscall `exit(status)`
    ;; syscall number: 60

    mov rdi, rax                    ;; move the return value of my_func into rdi (exit status)
    mov rax, 60                     ;; syscall number for exit
    syscall
