# Program summary:
# - Exit with status code 42.

.abiversion 2
.globl _start

.text
_start:
    li 3, 42                    # exit status in r3
    li 0, 1                     # Linux syscall number for exit
    sc
