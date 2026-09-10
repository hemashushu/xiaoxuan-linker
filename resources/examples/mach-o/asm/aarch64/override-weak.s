// Ported from resources/examples/elf/asm/aarch64/override-weak.s (Linux/ELF)
// to macOS/Mach-O. Differences from the Linux version:
// - GNU `.weak` is replaced with Mach-O's `.weak_definition` (paired with
//   `.globl` so the symbol is both weak and externally visible).

.weak_definition _foo
.weak_definition _bar

.section __TEXT,__text,regular,pure_instructions
.globl _foo
.globl _bar

// Define weak symbol `_foo`.
//
// ```c
// __attribute__((weak)) int foo() {
//     return 11;
// }
// ```
_foo:
    mov x0, #11                 // return 11
    ret

// Define weak symbol `_bar`.
//
// ```c
// __attribute__((weak)) int bar() {
//     return 13;
// }
// ```
_bar:
    mov x0, #13                 // return 13
    ret
