default rel                 ;; use RIP-relative addressing by default for position-independent code

section .data

;; ```c
;; int global_var = 41;
;; ```

global global_var
global_var:
    dd 41

;; ```c
;; static int local_var = 43;
;; ```

local_var:
    dd 43

section .text

global weak_fn:weak

;; ```c
;; __attribute__((weak)) int weak_fn() {
;;     return local_var;
;; }
;; ```

weak_fn:
    mov eax, [rel local_var]
    ret


;; invoke weak_fn() and add its return value to 1, then return the result.
;; ```c
;; int invoke_weak_fn() {
;;     return weak_fn() + 1;
;; }

invoke_weak_fn:
    call weak_fn
    add eax, 1
    ret