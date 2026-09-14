// Ported from resources/examples/elf/gcc/function.c (Linux) to macOS.
//
// Program summary:
// - Prints "Hello, world!\n" to stdout.
// - Exit with status code 0.

// #include <stdio.h>
#include <unistd.h>

const char hello[] = "Hello";
const char world[] = ", world!\n";

void print_hello(void)
{
    // puts(hello);
    write(1, hello, sizeof(hello) - 1); // write to stdout (fd=1)
}

void print_world(void)
{
    // puts(world);
    write(1, world, sizeof(world) - 1); // write to stdout (fd=1)
}

int main(void)
{
    print_hello();
    print_world();
    return 0;
}
