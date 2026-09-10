// Ported from resources/examples/elf/asm/aarch64/symbol-import.s (Linux/ELF)
// to macOS/Mach-O. Differences from the Linux version:
// - Symbols use the Mach-O leading-underscore convention.
// - Addresses use `sym@PAGE`/`sym@PAGEOFF` instead of GNU `:lo12:sym`.
// - `exit` uses the XNU/BSD syscall ABI (syscall number in x16 ORed with
//   0x2000000, invoked with `svc #0x80`).
//
// Program summary:
// - Exit with status code 24.

.section __TEXT,__text,regular,pure_instructions
.globl _start

.extern _foo
.extern _bar
.extern _a
.extern _b
.extern _x
.extern _y
.extern _dec
.extern _inc

// fn _start() -> void
_start:
    // read `_foo` and subtract 1 (by function `_dec`), then store the result in `_a`.
    // (`_a` should be 10 after this)
    adrp x0, _foo@PAGE
    ldr x0, [x0, _foo@PAGEOFF]
    bl _dec
    adrp x1, _a@PAGE
    str x0, [x1, _a@PAGEOFF]

    // read `_bar` and add 1 (by function `_inc`), then store the result in `_b`.
    // (`_b` should be 14 after this)
    adrp x0, _bar@PAGE
    ldr x0, [x0, _bar@PAGEOFF]
    bl _inc
    adrp x1, _b@PAGE
    str x0, [x1, _b@PAGEOFF]

    // read `_a` and `_b`, add them together, and store the result in `_x`.
    // (`_x` should be 24 after this)
    adrp x0, _a@PAGE
    ldr x0, [x0, _a@PAGEOFF]
    adrp x1, _b@PAGE
    ldr x1, [x1, _b@PAGEOFF]
    add x0, x0, x1
    adrp x2, _x@PAGE
    str x0, [x2, _x@PAGEOFF]

    // copy `_x` to `_y`.
    // (`_y` should be 24 after this)
    adrp x0, _x@PAGE
    ldr x0, [x0, _x@PAGEOFF]
    adrp x1, _y@PAGE
    str x0, [x1, _y@PAGEOFF]

    // read `_y` and exit with the value of `_y` as the status code.
    //
    // exit program using syscall `exit(status)`
    // syscall number: 1 (BSD class 0x2000000)
    adrp x0, _y@PAGE
    ldr x0, [x0, _y@PAGEOFF]
    movz x16, #0x0001
    movk x16, #0x0200, lsl #16
    svc #0x80
