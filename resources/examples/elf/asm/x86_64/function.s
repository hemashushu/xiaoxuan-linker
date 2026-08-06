# Program summary:
# - Prints "Hello, world!\n" to stdout.
# - Exit with status code 0.

.intel_syntax noprefix

.global _start

.section .rodata
    hello: .ascii "Hello"       # read-only global variable with a string value
    hello_len: .quad 5          # read-only global variable with value 5
    world: .ascii ", world!\n"  # read-only global variable with a string value
    world_len: .quad 9          # read-only global variable with value 9

.section .text

# fn print_hello() -> void
print_hello:
    # print string using syscall `write(fd, buf, count)`
    # syscall number: 1

    mov rdi, 1                  # file descriptor for stdout
    lea rsi, [rip + hello]      # pointer to the string to write
    mov rdx, qword ptr [rip + hello_len] # number of bytes to write (length of "Hello,")
    mov rax, 1                  # syscall number for write
    syscall
    ret

# fn print_world() -> void
print_world:
    # print string using syscall `write(fd, buf, count)`
    # syscall number: 1

    mov rdi, 1                  # file descriptor for stdout
    lea rsi, [rip + world]      # pointer to the string to write
    mov rdx, qword ptr [rip + world_len] # number of bytes to write (length of " World!\n")
    mov rax, 1                  # syscall number for write
    syscall
    ret

# fn _start() -> void
_start:
    call print_hello
    call print_world

    # exit program using syscall `exit(status)`
    # syscall number: 60

    xor rdi, rdi                # set exit status to 0
    mov rax, 60                 # syscall number for exit
    syscall
