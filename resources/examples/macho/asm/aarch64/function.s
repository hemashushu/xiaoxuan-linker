// Ported from resources/examples/elf/asm/aarch64/function.s (Linux/ELF) to
// macOS/Mach-O. Differences from the Linux version:
// - Read-only string literals are placed in `__TEXT,__const` instead of
//   `.rodata` (Mach-O has no dedicated `.rodata` section directive; plain,
//   non-relocated constants live in the __TEXT segment).
// - Addresses are computed with `adrp REG, sym@PAGE` / `add REG, REG, sym@PAGEOFF`
//   instead of the GNU `adrp REG, sym` / `:lo12:sym` syntax.
// - `write`/`exit` use the XNU/BSD syscall ABI (syscall number ORed with the
//   0x2000000 class bit, placed in x16, invoked with `svc #0x80`).
//
// Program summary:
// - Prints "Hello, world!\n" to stdout.
// - Exit with status code 0.

.section __TEXT,__const
_hello:
    .ascii "Hello"              // read-only string literal
hello_len = . - _hello           // assembly-time constant: length of "Hello"

_world:
    .ascii ", world!\n"         // read-only string literal
world_len = . - _world           // assembly-time constant: length of ", world!\n"

.section __TEXT,__text,regular,pure_instructions
.globl _start
.globl _print_hello
.globl _print_world

// fn print_hello() -> void
_print_hello:
    // print string using syscall `write(fd, buf, count)`
    // syscall number: 4 (BSD class 0x2000000)
    mov x0, #1                     // file descriptor for stdout
    adrp x1, _hello@PAGE
    add x1, x1, _hello@PAGEOFF     // pointer to the string to write
    mov x2, #hello_len             // number of bytes to write
    movz x16, #0x0004              // syscall number for write (4)
    movk x16, #0x0200, lsl #16     // | 0x2000000 (BSD syscall class)
    svc #0x80
    ret

// fn print_world() -> void
_print_world:
    mov x0, #1                     // file descriptor for stdout
    adrp x1, _world@PAGE
    add x1, x1, _world@PAGEOFF     // pointer to the string to write
    mov x2, #world_len             // number of bytes to write
    movz x16, #0x0004
    movk x16, #0x0200, lsl #16
    svc #0x80
    ret

// fn _start() -> void
_start:
    bl _print_hello
    bl _print_world

    // exit program using syscall `exit(status)`
    // syscall number: 1 (BSD class 0x2000000)
    mov x0, #0                     // set exit status to 0
    movz x16, #0x0001
    movk x16, #0x0200, lsl #16
    svc #0x80
