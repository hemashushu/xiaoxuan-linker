// Program summary:
// - Prints "Hello, world!\n" to stdout.
// - Exit with status code 0.

.section .rodata
hello:
    .ascii "Hello"             // read-only string literal
hello_len = . - hello           // assembly-time constant: length of "Hello"

world:
    .ascii ", world!\n"        // read-only string literal
world_len = . - world           // assembly-time constant: length of ", world!\n"

.section .text
.global _start

// fn print_hello() -> void
print_hello:
    // print string using syscall `write(fd, buf, count)`
    // syscall number: 64
    mov x0, #1                  // file descriptor for stdout
    adrp x1, hello
    add x1, x1, :lo12:hello     // pointer to the string to write
    mov x2, #hello_len          // number of bytes to write
    mov x8, #64                 // syscall number for write
    svc #0
    ret

// fn print_world() -> void
print_world:
    // print string using syscall `write(fd, buf, count)`
    // syscall number: 64
    mov x0, #1                  // file descriptor for stdout
    adrp x1, world
    add x1, x1, :lo12:world     // pointer to the string to write
    mov x2, #world_len          // number of bytes to write
    mov x8, #64                 // syscall number for write
    svc #0
    ret

// fn _start() -> void
_start:
    bl print_hello
    bl print_world

    // exit program using syscall `exit(status)`
    // syscall number: 93
    mov x0, #0                  // set exit status to 0
    mov x8, #93                 // syscall number for exit
    svc #0
