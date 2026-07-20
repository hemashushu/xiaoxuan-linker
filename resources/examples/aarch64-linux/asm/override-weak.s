.weak foo
.weak bar

// Define weak symbol `foo`.
//
// ```c
// __attribute__((weak)) int foo() {
//     return 11;
// }
// ```
foo:
    mov x0, #11                 // return 11
    ret

// Define weak symbol `bar`.
//
// ```c
// __attribute__((weak)) int bar() {
//     return 13;
// }
// ```
bar:
    mov x0, #13                 // return 13
    ret
