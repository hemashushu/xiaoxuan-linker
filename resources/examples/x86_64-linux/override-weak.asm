default rel                 ;; use RIP-relative addressing by default for position-independent code

global foo:weak
global bar:weak

;; Define weak symbols `foo`.
;;
;; ```c
;; __attribute__((weak)) int foo() {
;;     return 11;
;; }
;; ```
foo:
    mov eax, 11
    ret

;; Define weak symbols `bar`.
;;
;; ```c
;; __attribute__((weak)) int bar() {
;;     return 13;
;; }
;; ```
bar:
    mov eax, 13
    ret