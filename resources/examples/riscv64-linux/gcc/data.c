// Program summary:
// - Exit with status code 24.

#include "common.in"

// .rodata
const static long foo = 11;
const static long bar = 13;

// .data
static long a = 17;
static long b = 19;

// .bss
static long x;
static long y;

[[noreturn]]
void _start(void)
{
    a = foo - 1; // 11 - 1 = 10
    b = bar + 1; // 13 + 1 = 14
    x = a + b;   // 10 + 14 = 24
    y = x;       // 24

    exit(y);
}