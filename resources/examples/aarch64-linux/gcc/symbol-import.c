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
void _start(void)
{
    a = dec(foo); // a = 11 - 1 = 10
    b = inc(bar); // b = 13 + 1 = 14
    x = a + b;    // x = 10 + 14 = 24
    y = x;        // y = 24
    exit(y);
}