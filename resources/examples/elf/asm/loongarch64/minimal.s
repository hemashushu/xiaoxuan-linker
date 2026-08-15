# Program summary:
# - Exit with status code 42.

.globl _start

.text

# fn _start() -> void
_start:
    li.w $a0, 42                 # exit status code in a0
    li.w $a7, 93                 # syscall number for exit
    syscall 0
