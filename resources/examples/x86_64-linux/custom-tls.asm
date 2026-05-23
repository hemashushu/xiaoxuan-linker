default rel                     ;; use RIP-relative addressing by default for position-independent code

global tls_var_a
global tls_var_b
global my_var
global _start

;; byte offsets of TLS variables within the TLS block
;; (matches the .tdata/.tbss layout: tls_var_a at 0, tls_var_b at 8)
%define TLS_VAR_A  0
%define TLS_VAR_B  8

;; .tdata: TLS template data — copied into each thread's TLS block at thread creation
section .tdata
    tls_var_a dq 11             ;; thread-local variable with initial value 11

;; .tbss: TLS zero-initialized template — each thread's copy starts as 0
section .tbss
    tls_var_b resq 1            ;; thread-local uninitialized variable (8 bytes)

section .data
    my_var dq 42                ;; read-write global variable with initial value 42

section .bss
    ;; manually allocated TLS block for the main thread (8 + 8 = 16 bytes)
    tls_block resb 16

section .text

;; fn _start() -> void
_start:
    ;; TLS initialization for the main thread
    ;; --------------------------------------
    ;;
    ;; NOTE: NASM does not implement the `wrt ..tpoff` TLS relocation for ELF64,
    ;; so we cannot use `[fs:tls_var_a wrt ..tpoff]` directly. Instead, we
    ;; manually initialize TLS by:
    ;;
    ;; 1. allocating a TLS block in .bss
    ;; 2. copying the .tdata template values into it
    ;; 3. setting FS.base to point to the block via arch_prctl(ARCH_SET_FS)
    ;;
    ;; After setup, variables are accessed via [fs:fixed_offset].

    ;; step 1: set FS.base = &tls_block via arch_prctl(ARCH_SET_FS, &tls_block)
    ;;         syscall number: 158, ARCH_SET_FS = 0x1002
    mov rdi, 0x1002             ;; first argument: ARCH_SET_FS
    lea rsi, [rel tls_block]    ;; second argument: address of TLS block
    mov rax, 158                ;; syscall: arch_prctl
    syscall

    ;; step 2: copy tls_var_a initial value from .tdata template into the TLS block
    mov rax, [rel tls_var_a]    ;; read template value (11) from .tdata section
    mov [fs:TLS_VAR_A], rax     ;; write into TLS block at offset 0

    ;; step 3: tls_var_b starts as 0 — .bss is already zero-initialized, no copy needed

    ;; main logic
    ;; ----------
    ;; read/write TLS variables and exit with my_var's value as status

    ;; read tls_var_a from the thread-local TLS block
    mov rax, [fs:TLS_VAR_A]     ;; load tls_var_a (value: 11)

    ;; write the value of tls_var_a into tls_var_b
    mov [fs:TLS_VAR_B], rax     ;; tls_var_b = tls_var_a

    ;; read my_var and use its value as the exit code
    mov rdi, [rel my_var]       ;; load my_var (value: 42)

    ;; exit program using syscall `exit(status)`
    ;; syscall number: 60

    mov rax, 60                 ;; syscall number for exit
    syscall
