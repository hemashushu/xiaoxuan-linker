# Program summary:
# - Exit with status code 42.

.globl _start

.text
_start:
    lghi %r2, 42                 # exit status in r2
    lghi %r1, 1                  # Linux syscall number for exit
    svc 0
