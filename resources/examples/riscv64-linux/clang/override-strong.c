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
void _start(void)
{
    int i = foo();
    int j = bar();
    int k = i + j;
    exit(k);
}