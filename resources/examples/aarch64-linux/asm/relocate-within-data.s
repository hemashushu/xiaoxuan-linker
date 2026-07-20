// Program summary:
// - Exit with status code 24.

.section .data
.align 3
foo:
    .quad 11                   // read-write global variable with initial value 11
.align 3
bar:
    .quad 13                   // read-write global variable with initial value 13
.align 3
a:
    .quad 17                   // additional read-write global variable with initial value 17
.align 3
b:
    .quad 19                   // additional read-write global variable with initial value 19
.align 3
pdec:
    .quad dec                  // pointer to dec (function pointer)
.align 3
pinc:
    .quad inc                  // pointer to inc (function pointer)

.section .rodata
.align 3
pfoo:
    .quad foo                  // pointer to foo (data pointer)
.align 3
pbar:
    .quad bar                  // pointer to bar (data pointer)

.section .bss
.align 3
x:
    .skip 8                    // uninitialized global variable (8 bytes)
.align 3
y:
    .skip 8                    // uninitialized global variable (8 bytes)

.section .text
.global dec
.global inc
.global _start

// fn dec(n: int64_t) -> int64_t
dec:
    sub x0, x0, #1             // decrement by 1
    ret                        // return x0

// fn inc(n: int64_t) -> int64_t
inc:
    add x0, x0, #1             // increment by 1
    ret                        // return x0

// fn _start() -> void
_start:
    // Read the value of `foo` by dereferencing the pointer `pfoo` (in .rodata).
    adrp x2, pfoo
    ldr x2, [x2, :lo12:pfoo]
    ldr x0, [x2]

    // Invoke `dec` via the function pointer `pdec` (in .data) with the value of `foo` as argument.
    adrp x3, pdec
    ldr x3, [x3, :lo12:pdec]
    blr x3

    // Store the result of `dec(foo)` into `foo` (via pointer `pfoo`).
    // After this, `foo` should be 10 (11 - 1).
    str x0, [x2]

    // Read the value of `bar` by dereferencing the pointer `pbar` (in .rodata).
    adrp x4, pbar
    ldr x4, [x4, :lo12:pbar]
    ldr x0, [x4]

    // Invoke `inc` via the function pointer `pinc` (in .data) with the value of `bar` as argument.
    // Call `inc(bar)` via dereferencing the pointer `pinc` directly.
    adrp x5, pinc
    ldr x5, [x5, :lo12:pinc]
    blr x5

    // Store the result of `inc(bar)` into `bar` (via pointer `pbar`).
    // After this, `bar` should be 14 (13 + 1).
    str x0, [x4]

    // Read the updated values of `foo` and `bar` directly from memory (not via pointers), add them together.
    // The result should be 24 (10 + 14).
    adrp x0, foo
    ldr x0, [x0, :lo12:foo]
    adrp x1, bar
    ldr x1, [x1, :lo12:bar]
    add x0, x0, x1

    // Exit with the sum as the status code.
    //
    // exit program using syscall `exit(status)`
    // syscall number: 93
    mov x8, #93
    svc #0
