// Program summary:
// - Exit with status code 24.

// data types:
// - .quad: 64-bit integer
// - .word: 32-bit integer
// - .hword: 16-bit integer
// - .byte: 8-bit integer
//
// uninitialized global variables (BSS section) can be defined with:
// - .skip N: reserve N bytes

.section .rodata
.align 3
foo:
    .quad 11                   // read-only global variable with value 11
.align 3
bar:
    .quad 13                   // read-only global variable with value 13

.section .data
.align 3
a:
    .quad 17                   // read-write global variable with initial value 17
.align 3
b:
    .quad 19                   // read-write global variable with initial value 19

.section .bss
.align 3
x:
    .skip 8                    // uninitialized global variable (8 bytes)
.align 3
y:
    .skip 8                    // uninitialized global variable (8 bytes)

.section .text
.global _start

// fn _start() -> void
_start:
    // read `foo` and subtract 1, then store the result in `a`.
    // (`a` should be 10 after this)
    adrp x0, foo
    add x0, x0, :lo12:foo
    ldr x0, [x0]
    sub x0, x0, #1
    adrp x1, a
    add x1, x1, :lo12:a
    str x0, [x1]

    // read `bar` and add 1, then store the result in `b`.
    // (`b` should be 14 after this)
    adrp x0, bar
    ldr x0, [x0, :lo12:bar]
    add x0, x0, #1
    adrp x1, b
    str x0, [x1, :lo12:b]

    // read `a` and `b`, add them together, and store the result in `x`.
    // (`x` should be 24 after this)
    adrp x0, a
    add x0, x0, :lo12:a
    ldr x0, [x0]
    adrp x1, b
    add x1, x1, :lo12:b
    ldr x1, [x1]
    add x0, x0, x1
    adrp x2, x
    add x2, x2, :lo12:x
    str x0, [x2]

    // copy `x` to `y`.
    // (`y` should be 24 after this)
    adrp x0, x
    add x0, x0, :lo12:x
    ldr x0, [x0]
    adrp x1, y
    add x1, x1, :lo12:y
    str x0, [x1]

    // read `y` and exit with the value of `y` as the status code.
    //
    // exit program using syscall `exit(status)`
    // syscall number: 93
    adrp x0, y
    add x0, x0, :lo12:y
    ldr x0, [x0]
    mov x8, #93
    svc #0
