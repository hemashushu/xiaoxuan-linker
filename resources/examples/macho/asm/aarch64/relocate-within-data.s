// Ported from resources/examples/elf/asm/aarch64/relocate-within-data.s
// (Linux/ELF) to macOS/Mach-O. Differences from the Linux version:
// - .data -> `__DATA,__data`, .bss -> `.zerofill`.
// - Pointer-typed "read-only" globals (`_pfoo`, `_pbar`) cannot live in
//   `__TEXT,__const` on Mach-O: the __TEXT segment is mapped read-only and
//   shared, so the dynamic linker is not allowed to apply load-time pointer
//   relocations there ("illegal text-relocation"). They are placed in
//   `__DATA_CONST,__const` instead, which dyld can rebase at load time and
//   then protect as read-only.
// - Addresses use `sym@PAGE`/`sym@PAGEOFF` instead of GNU `:lo12:sym`.
// - `exit` uses the XNU/BSD syscall ABI (syscall number in x16 ORed with
//   0x2000000, invoked with `svc #0x80`).
//
// Program summary:
// - Exit with status code 24.

.section __DATA,__data
.align 3
_foo:
    .quad 11                   // read-write global variable with initial value 11
.align 3
_bar:
    .quad 13                   // read-write global variable with initial value 13
.align 3
_a:
    .quad 17                   // additional read-write global variable with initial value 17
.align 3
_b:
    .quad 19                   // additional read-write global variable with initial value 19
.align 3
_pdec:
    .quad _dec                 // pointer to _dec (function pointer)
.align 3
_pinc:
    .quad _inc                 // pointer to _inc (function pointer)

.section __DATA_CONST,__const
.align 3
_pfoo:
    .quad _foo                 // pointer to _foo (data pointer)
.align 3
_pbar:
    .quad _bar                  // pointer to _bar (data pointer)

.align 3
.zerofill __DATA,__bss,_x,8,3  // uninitialized global variable (8 bytes)
.align 3
.zerofill __DATA,__bss,_y,8,3  // uninitialized global variable (8 bytes)

.section __TEXT,__text,regular,pure_instructions
.globl _dec
.globl _inc
.globl _start

// fn dec(n: int64_t) -> int64_t
_dec:
    sub x0, x0, #1              // decrement by 1
    ret                         // return x0

// fn inc(n: int64_t) -> int64_t
_inc:
    add x0, x0, #1              // increment by 1
    ret                         // return x0

// fn _start() -> void
_start:
    // Read the value of `_foo` by dereferencing the pointer `_pfoo` (in __DATA_CONST,__const).
    adrp x2, _pfoo@PAGE
    ldr x2, [x2, _pfoo@PAGEOFF]
    ldr x0, [x2]

    // Invoke `_dec` via the function pointer `_pdec` (in __DATA,__data) with the value of `_foo` as argument.
    adrp x3, _pdec@PAGE
    ldr x3, [x3, _pdec@PAGEOFF]
    blr x3

    // Store the result of `_dec(_foo)` into `_foo` (via pointer `_pfoo`).
    // After this, `_foo` should be 10 (11 - 1).
    str x0, [x2]

    // Read the value of `_bar` by dereferencing the pointer `_pbar` (in __DATA_CONST,__const).
    adrp x4, _pbar@PAGE
    ldr x4, [x4, _pbar@PAGEOFF]
    ldr x0, [x4]

    // Invoke `_inc` via the function pointer `_pinc` (in __DATA,__data) with the value of `_bar` as argument.
    adrp x5, _pinc@PAGE
    ldr x5, [x5, _pinc@PAGEOFF]
    blr x5

    // Store the result of `_inc(_bar)` into `_bar` (via pointer `_pbar`).
    // After this, `_bar` should be 14 (13 + 1).
    str x0, [x4]

    // Read the updated values of `_foo` and `_bar` directly from memory (not via pointers), add them together.
    // The result should be 24 (10 + 14).
    adrp x0, _foo@PAGE
    ldr x0, [x0, _foo@PAGEOFF]
    adrp x1, _bar@PAGE
    ldr x1, [x1, _bar@PAGEOFF]
    add x0, x0, x1

    // Exit with the sum as the status code.
    //
    // exit program using syscall `exit(status)`
    // syscall number: 1 (BSD class 0x2000000)
    movz x16, #0x0001
    movk x16, #0x0200, lsl #16
    svc #0x80
