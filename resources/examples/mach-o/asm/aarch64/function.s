// Ported from resources/examples/elf/asm/aarch64/function.s (Linux/ELF) to
// macOS/Mach-O.
//
// Program summary:
// - Prints "Hello, world!\n" to stdout using standard library `write`.
// - Exit with status code 0.

.section __TEXT,__const
_hello:
    .ascii "Hello"              // read-only string literal
hello_len = . - _hello           // assembly-time constant: length of "Hello"

_world:
    .ascii ", world!\n"         // read-only string literal
world_len = . - _world           // assembly-time constant: length of ", world!\n"

.section __TEXT,__text,regular,pure_instructions
.globl _main
.globl _print_hello
.globl _print_world
.p2align 2

// fn print_hello() -> void
_print_hello:
    stp x29, x30, [sp, #-16]!
    mov x29, sp
    mov x0, #1                     // file descriptor for stdout
    adrp x1, _hello@PAGE
    add x1, x1, _hello@PAGEOFF     // pointer to the string to write
    mov x2, #hello_len             // number of bytes to write
    bl _write
    ldp x29, x30, [sp], #16
    ret

// fn print_world() -> void
_print_world:
    stp x29, x30, [sp, #-16]!
    mov x29, sp
    mov x0, #1                     // file descriptor for stdout
    adrp x1, _world@PAGE
    add x1, x1, _world@PAGEOFF     // pointer to the string to write
    mov x2, #world_len             // number of bytes to write
    bl _write
    ldp x29, x30, [sp], #16
    ret

// fn main() -> int
_main:
    stp x29, x30, [sp, #-16]!
    mov x29, sp
    bl _print_hello
    bl _print_world
    mov x0, #0                     // set exit status to 0
    ldp x29, x30, [sp], #16
    ret
