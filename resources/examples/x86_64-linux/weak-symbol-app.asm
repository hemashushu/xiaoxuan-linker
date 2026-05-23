default rel                 ;; use RIP-relative addressing by default for position-independent code

extern global_var

section .text

global _start
global weak_fn

;; ```c
;; int weak_fn() {
;;    return 47;
;; }
;; ```

weak_fn:
    mov eax, 47
    ret

_start:
    ;; weak_fn() + global_var = 47 + 41 = 88
    call weak_fn
    add eax, [rel global_var]

    ;; exit program using syscall `exit(status)`
    ;; syscall number: 60

    mov edi, eax
    mov eax, 60
    syscall