// Ported from resources/examples/elf/asm/aarch64/symbol-export.s (Linux/ELF)
// to macOS/Mach-O. Differences from the Linux version:
// - .rodata -> `__TEXT,__const`, .data -> `__DATA,__data`, .bss -> `.zerofill`.
// - Symbols use the Mach-O leading-underscore convention.

.section __TEXT,__const
.align 3
.globl _foo
.globl _bar
_foo:
    .quad 11                   // read-only global variable with value 11
.align 3
_bar:
    .quad 13                   // read-only global variable with value 13

.section __DATA,__data
.align 3
.globl _a
.globl _b
_a:
    .quad 17                   // read-write global variable with initial value 17
.align 3
_b:
    .quad 19                   // read-write global variable with initial value 19

.globl _x
.globl _y
.align 3
.zerofill __DATA,__bss,_x,8,3  // uninitialized global variable (8 bytes)
.align 3
.zerofill __DATA,__bss,_y,8,3  // uninitialized global variable (8 bytes)

.section __TEXT,__text,regular,pure_instructions
.globl _dec
.globl _inc

// fn dec(int64_t) -> int64_t
_dec:
    sub x0, x0, #1              // decrement the argument by 1 and return the result
    ret

// fn inc(int64_t) -> int64_t
_inc:
    add x0, x0, #1              // increment the argument by 1 and return the result
    ret
