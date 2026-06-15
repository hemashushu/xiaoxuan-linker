// Program summary:
// - Exit with status code 24.

.section .text
.global _start

.extern foo
.extern bar
.extern a
.extern b
.extern x
.extern y
.extern dec
.extern inc

// fn _start() -> void
_start:
    // read `foo` and subtract 1 (by function `dec`), then store the result in `a`.
    // (`a` should be 10 after this)
    adrp x0, foo
    ldr x0, [x0, :lo12:foo]
    bl dec
    adrp x1, a
    str x0, [x1, :lo12:a]

    // read `bar` and add 1 (by function `inc`), then store the result in `b`.
    // (`b` should be 14 after this)
    adrp x0, bar
    ldr x0, [x0, :lo12:bar]
    bl inc
    adrp x1, b
    str x0, [x1, :lo12:b]

    // read `a` and `b`, add them together, and store the result in `x`.
    // (`x` should be 24 after this)
    adrp x0, a
    ldr x0, [x0, :lo12:a]
    adrp x1, b
    ldr x1, [x1, :lo12:b]
    add x0, x0, x1
    adrp x2, x
    str x0, [x2, :lo12:x]

    // copy `x` to `y`.
    // (`y` should be 24 after this)
    adrp x0, x
    ldr x0, [x0, :lo12:x]
    adrp x1, y
    str x0, [x1, :lo12:y]

    // read `y` and exit with the value of `y` as the status code.
    //
    // exit program using syscall `exit(status)`
    // syscall number: 93
    adrp x0, y
    ldr x0, [x0, :lo12:y]
    mov x8, #93
    svc #0
