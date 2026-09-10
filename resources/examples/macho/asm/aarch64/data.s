// Ported from resources/examples/elf/asm/aarch64/data.s (Linux/ELF) to
// macOS/Mach-O. Differences from the Linux version:
// - .rodata -> `__TEXT,__const`, .data -> `__DATA,__data`.
// - .bss uninitialized globals use `.zerofill __DATA,__bss,sym,size,align`
//   instead of `.section .bss` + `.skip N`.
// - Addresses use `sym@PAGE`/`sym@PAGEOFF` instead of GNU `:lo12:sym`.
// - `exit` uses the XNU/BSD syscall ABI (syscall number in x16 ORed with
//   0x2000000, invoked with `svc #0x80`).
//
// Program summary:
// - Exit with status code 24.

.section __TEXT,__const
.align 3
_foo:
    .quad 11                   // read-only global variable with value 11
.align 3
_bar:
    .quad 13                   // read-only global variable with value 13

.section __DATA,__data
.align 3
_a:
    .quad 17                   // read-write global variable with initial value 17
.align 3
_b:
    .quad 19                   // read-write global variable with initial value 19

.align 3
.zerofill __DATA,__bss,_x,8,3  // uninitialized global variable (8 bytes)
.align 3
.zerofill __DATA,__bss,_y,8,3  // uninitialized global variable (8 bytes)

.section __TEXT,__text,regular,pure_instructions
.globl _start

.globl _foo
.globl _bar
.globl _a
.globl _b
.globl _x
.globl _y

// fn _start() -> void
_start:
    // read `_foo` and subtract 1, then store the result in `_a`.
    // (`_a` should be 10 after this)
    adrp x0, _foo@PAGE
    ldr x0, [x0, _foo@PAGEOFF]
    sub x0, x0, #1
    adrp x1, _a@PAGE
    str x0, [x1, _a@PAGEOFF]

    // read `_bar` and add 1, then store the result in `_b`.
    // (`_b` should be 14 after this)
    adrp x0, _bar@PAGE
    ldr x0, [x0, _bar@PAGEOFF]
    add x0, x0, #1
    adrp x1, _b@PAGE
    str x0, [x1, _b@PAGEOFF]

    // read `_a` and `_b`, add them together, and store the result in `_x`.
    // (`_x` should be 24 after this)
    adrp x0, _a@PAGE
    add x0, x0, _a@PAGEOFF
    ldr x0, [x0]
    adrp x1, _b@PAGE
    add x1, x1, _b@PAGEOFF
    ldr x1, [x1]
    add x0, x0, x1
    adrp x2, _x@PAGE
    add x2, x2, _x@PAGEOFF
    str x0, [x2]

    // copy `_x` to `_y`.
    // (`_y` should be 24 after this)
    adrp x0, _x@PAGE
    add x0, x0, _x@PAGEOFF
    ldr x0, [x0]
    adrp x1, _y@PAGE
    add x1, x1, _y@PAGEOFF
    str x0, [x1]

    // read `_y` and exit with the value of `_y` as the status code.
    //
    // exit program using syscall `exit(status)`
    // syscall number: 1 (BSD class 0x2000000)
    adrp x0, _y@PAGE
    add x0, x0, _y@PAGEOFF
    ldr x0, [x0]
    movz x16, #0x0001
    movk x16, #0x0200, lsl #16
    svc #0x80
