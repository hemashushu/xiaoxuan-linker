// Ported from resources/examples/elf/asm/aarch64/override-strong.s
// (Linux/ELF) to macOS/Mach-O. Differences from the Linux version:
// - Symbols use the Mach-O leading-underscore convention.
// - `exit` uses the XNU/BSD syscall ABI (syscall number in x16 ORed with
//   0x2000000, invoked with `svc #0x80`).
//
// Program summary:
// - Exit with status code 53.

.extern _foo

.section __TEXT,__text,regular,pure_instructions
.globl _start
.globl _bar

// Override the weak symbol `_bar` with a strong symbol.
//
// ```c
// int bar() {
//    return 42;
// }
// ```
_bar:
    mov x0, #42                 // return 42
    ret

// fn _start() -> void
_start:
    // Call `_foo`.
    // Now `x0` should be 11 (the value returned by `_foo`).
    bl _foo
    mov x19, x0                 // save the result across the next call

    // Call `_bar`.
    // Now `x0` should be 42 (the value returned by `_bar`).
    bl _bar

    // Sum their results.
    // Now `x0` should be 53 (11 + 42).
    add x0, x0, x19

    // Exit with the sum as the status code.
    //
    // Exit program using syscall `exit(status)`.
    // syscall number: 1 (BSD class 0x2000000)
    movz x16, #0x0001
    movk x16, #0x0200, lsl #16
    svc #0x80
