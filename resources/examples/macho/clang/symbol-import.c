// Ported from resources/examples/elf/gcc/symbol-import.c (Linux) to macOS.
// See minimal.c for the `start` vs `_start` naming note.
//
// Program summary:
// - Exit with status code 24.

#include "common.in"

extern long foo;
extern long bar;
extern long a;
extern long b;
extern long x;
extern long y;
extern long dec(long n);
extern long inc(long n);

[[noreturn]]
void start(void)
{
    a = dec(foo); // a = 11 - 1 = 10
    b = inc(bar); // b = 13 + 1 = 14
    x = a + b;    // x = 10 + 14 = 24
    y = x;        // y = 24
    exit(y);
}
