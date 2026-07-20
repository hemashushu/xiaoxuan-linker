# Program summary:
# - Prints "Hello, world!\n" to stdout.
# - Exit with status code 0.

.option norvc                   # disable compressed instructions for clarity
.global _start

.section .rodata
    hello: .string "Hello"              # read-only global variable with a string value
    hello_len: .quad 5                  # read-only global variable with value 5
    world: .string ", world!\n"         # read-only global variable with a string value
    world_len: .quad 9                  # read-only global variable with value 9

.section .text

# fn print_hello() -> void
print_hello:
    # print string using syscall `write(fd, buf, count)`
    # syscall number: 64
    li a0, 1                    # file descriptor for stdout
    la a1, hello                # pointer to the string to write
    la a3, hello_len
    ld a2, 0(a3)                # number of bytes to write (length of "Hello")
    li a7, 64                   # syscall number for write
    ecall
    ret

# fn print_world() -> void
print_world:
    # print string using syscall `write(fd, buf, count)`
    # syscall number: 64
    li a0, 1                    # file descriptor for stdout
    la a1, world                # pointer to the string to write
    la a3, world_len
    ld a2, 0(a3)                # number of bytes to write (length of ", world!\n")
    li a7, 64                   # syscall number for write
    ecall
    ret

# fn _start() -> void
_start:
    la gp, __global_pointer$
    call print_hello
    call print_world

    # exit program using syscall `exit(status)`
    # syscall number: 93

    li a0, 0                    # set exit status to 0
    li a7, 93                   # syscall number for exit
    ecall

