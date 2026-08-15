# Program summary:
# - Prints "Hello, world!\n" to stdout.
# - Exit with status code 0.

.globl _start

.section .rodata
.align 2
hello: .ascii "Hello"
.align 2
world: .ascii ", world!\n"

.text
print_hello:
    lghi %r2, 1
    larl %r3, hello
    lghi %r4, 5
    lghi %r1, 4
    svc 0
    br %r14

print_world:
    lghi %r2, 1
    larl %r3, world
    lghi %r4, 9
    lghi %r1, 4
    svc 0
    br %r14

_start:
    brasl %r14, print_hello
    brasl %r14, print_world
    lghi %r2, 0
    lghi %r1, 1
    svc 0
