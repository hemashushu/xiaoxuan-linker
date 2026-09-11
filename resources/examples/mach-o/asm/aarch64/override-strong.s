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
.globl _main
.globl _bar
.p2align 2

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

// fn main() -> int
_main:
    stp x19, x30, [sp, #-16]!
    mov x29, sp

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

    ldp x19, x30, [sp], #16
    ret
