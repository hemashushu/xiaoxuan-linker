# Program summary:
# - Prints "Hello, world!\n" to stdout.
# - Exit with status code 0.

.globl _start

.section .rodata
hello:
    .ascii "Hello"
hello_len = . - hello
world:
    .ascii ", world!\n"
world_len = . - world

.text

# fn print_hello() -> void
print_hello:
    li.w $a0, 1                  # file descriptor for stdout
    la.local $a1, hello          # pointer to the string to write
    li.w $a2, hello_len          # number of bytes to write
    li.w $a7, 64                 # syscall number for write
    syscall 0
    ret

# fn print_world() -> void
print_world:
    li.w $a0, 1                  # file descriptor for stdout
    la.local $a1, world          # pointer to the string to write
    li.w $a2, world_len          # number of bytes to write
    li.w $a7, 64                 # syscall number for write
    syscall 0
    ret

# fn _start() -> void
_start:
    bl print_hello
    bl print_world
    li.w $a0, 0                  # set exit status to 0
    li.w $a7, 93                 # syscall number for exit
    syscall 0
