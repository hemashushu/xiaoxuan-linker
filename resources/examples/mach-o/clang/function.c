// Ported from resources/examples/elf/gcc/function.c (Linux) to macOS.
//
// Program summary:
// - Prints "Hello, world!\n" to stdout.
// - Exit with status code 0.

#include <stdio.h>

const char hello[] = "Hello";
const char world[] = ", world!\n";

void print_hello(void)
{
    printf("%s", hello);
}

void print_world(void)
{
    printf("%s", world);
}

int main(void)
{
    print_hello();
    print_world();
    return 0;
}
