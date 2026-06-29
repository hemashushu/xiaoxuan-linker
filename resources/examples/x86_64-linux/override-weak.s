.intel_syntax noprefix

.global foo
.weak foo
.global bar
.weak bar

# Define weak symbols `foo`.
#
# ```c
# __attribute__((weak)) int foo() {
#     return 11;
# }
# ```
foo:
    mov eax, 11
    ret

# Define weak symbols `bar`.
#
# ```c
# __attribute__((weak)) int bar() {
#     return 13;
# }
# ```
bar:
    mov eax, 13
    ret
