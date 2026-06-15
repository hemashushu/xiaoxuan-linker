.section .rodata
hello:
    .ascii "Hello"
hello_len = . - hello

world:
    .ascii ", world!\n"
world_len = . - world

.section .text
.global _start

print_hello:
    mov x0, #1
    adrp x1, hello
    add x1, x1, :lo12:hello
    mov x2, #hello_len
    mov x8, #64
    svc #0
    ret

print_world:
    mov x0, #1
    adrp x1, world
    add x1, x1, :lo12:world
    mov x2, #world_len
    mov x8, #64
    svc #0
    ret

_start:
    bl print_hello
    bl print_world
    mov x0, #0
    mov x8, #93
    svc #0
