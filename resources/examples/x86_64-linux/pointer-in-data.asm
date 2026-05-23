default rel                         ;; use RIP-relative addressing by default for position-independent code

global my_var
global my_func
global _start

section .data
    my_var dq 42                    ;; read-write global variable with initial value 42

    var_ptr dq my_var               ;; pointer to my_var (data pointer)
    func_ptr dq my_func             ;; pointer to my_func (function pointer)

section .rodata
    var_ptr_const dq my_var         ;; pointer to my_var (data pointer, read-only)

section .text

;; fn my_func(n: int64_t) -> int64_t
my_func:
    mov rax, rdi                    ;; copy the first argument into rax
    add rax, 1                      ;; increment by 1
    ret                             ;; return rax

;; fn _start() -> void
_start:

    ;; Dereference var_ptr (in .data) to get the value it points to
    mov rax, [var_ptr]              ;; rax = *var_ptr = &my_var  (RIP-relative → .rela.text entry)
    mov rbx, [rax]                  ;; rbx = my_var = 42

    ;; Dereference var_ptr_const (in .rodata) to get the value it points to
    mov rax, [var_ptr_const]        ;; rax = *var_ptr_const = &my_var  (RIP-relative → .rela.text entry)
    mov rcx, [rax]                  ;; rcx = my_var = 42

    ;; Calculate the sum of the two values
    add rbx, rcx                    ;; rbx = 42 + 42 = 84

    ;; Call the function pointed to by func_ptr with the sum as argument
    mov rax, [func_ptr]             ;; rax = *func_ptr = &my_func  (RIP-relative → .rela.text entry)
    mov rdi, rbx                    ;; first argument = 84
    call rax                        ;; rax = my_func(84) = 85

    ;; exit program using syscall `exit(status)`
    ;; syscall number: 60
    mov rdi, rax                    ;; move the return value of my_func into rdi (exit status)
    mov rax, 60                     ;; syscall number for exit
    syscall
