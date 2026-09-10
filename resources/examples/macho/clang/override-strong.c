// Ported from resources/examples/elf/gcc/override-strong.c (Linux) to macOS.
// See minimal.c for the `start` vs `_start` naming note.
//
// Program summary:
// - Exit with status code 53.

#include "common.in"

extern int foo();
extern int bar();

int bar()
{
    return 42;
}

[[noreturn]]
void start(void)
{
    int i = foo();
    int j = bar();
    int k = i + j;
    exit(k);
}
