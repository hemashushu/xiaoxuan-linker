.extern foo
.extern bar

.section .text
.global _start
.global bar

bar:
    mov x0, #42
    ret

_start:
    bl foo
    mov x19, x0
    bl bar
    add x0, x0, x19
    mov x8, #93
    svc #0
