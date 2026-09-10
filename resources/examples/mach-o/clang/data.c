// Ported from resources/examples/elf/gcc/data.c (Linux) to macOS.
// See minimal.c for the `start` vs `_start` naming note.
//
// Program summary:
// - Exit with status code 24.

#include "common.in"

// __TEXT,__const
const long foo = 11;
const long bar = 13;

// __DATA,__data
long a = 17;
long b = 19;

// __DATA,__bss
long x;
long y;

[[noreturn]]
void start(void)
{
    a = foo - 1; // 11 - 1 = 10
    b = bar + 1; // 13 + 1 = 14
    x = a + b;   // 10 + 14 = 24
    y = x;       // 24

    exit(y);
}
