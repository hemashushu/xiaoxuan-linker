// Program summary:
// - Exit with status code 53.

.extern foo
.extern bar

.section .text
.global _start
.global bar

// Override the weak symbol `bar` with a strong symbol.
//
// ```c
// int bar() {
//    return 42;
// }
// ```
bar:
    mov x0, #42                 // return 42
    ret

// fn _start() -> void
_start:
    // Call `foo`.
    // Now `x0` should be 11 (the value returned by `foo`).
    bl foo
    mov x19, x0                 // save the result across the next call

    // Call `bar`.
    // Now `x0` should be 42 (the value returned by `bar`).
    bl bar

    // Sum their results.
    // Now `x0` should be 53 (11 + 42).
    add x0, x0, x19

    // Exit with the sum as the status code.
    //
    // Exit program using syscall `exit(status)`.
    // syscall number: 93
    mov x8, #93
    svc #0
