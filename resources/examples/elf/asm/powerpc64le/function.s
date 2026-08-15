# Program summary:
# - Prints "Hello, world!\n" to stdout.
# - Exit with status code 0.

.abiversion 2
.globl _start

.section .rodata
hello: .ascii "Hello"
world: .ascii ", world!\n"

.text
print_hello:
    li 3, 1
    lis 4, hello@highest
    ori 4, 4, hello@higher
    sldi 4, 4, 32
    oris 4, 4, hello@h
    ori 4, 4, hello@l
    li 5, 5
    li 0, 4                     # Linux syscall number for write
    sc
    blr

print_world:
    li 3, 1
    lis 4, world@highest
    ori 4, 4, world@higher
    sldi 4, 4, 32
    oris 4, 4, world@h
    ori 4, 4, world@l
    li 5, 9
    li 0, 4
    sc
    blr

_start:
    bl print_hello
    bl print_world
    li 3, 0
    li 0, 1
    sc
