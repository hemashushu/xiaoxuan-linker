.option norvc                   # disable compressed instructions for clarity
.global foo
.weak foo
.global bar
.weak bar

.section .text

# Define weak symbols `foo`.
#
# ```c
# __attribute__((weak)) int foo() {
#     return 11;
# }
# ```
foo:
    li a0, 11
    ret

# Define weak symbols `bar`.
#
# ```c
# __attribute__((weak)) int bar() {
#     return 13;
# }
# ```
bar:
    li a0, 13
    ret
