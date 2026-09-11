// Ported from resources/examples/elf/asm/aarch64/minimal.s (Linux/ELF) to
// macOS/Mach-O.
//
// Program summary:
// - Exit with status code 42.

.section __TEXT,__text,regular,pure_instructions
.globl _main
.p2align 2

// fn main() -> int
_main:
    mov x0, #42                    // exit status code in x0
    ret
