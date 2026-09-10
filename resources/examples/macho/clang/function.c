// Ported from resources/examples/elf/gcc/function.c (Linux) to macOS.
// See minimal.c for the `start` vs `_start` naming note.
//
// Program summary:
// - Prints "Hello, world!\n" to stdout.
// - Exit with status code 0.

#include "common.in"

const char hello[] = "Hello";
const long hello_len = sizeof(hello) - 1; // String literals include a null terminator
const char world[] = ", world!\n";
const long world_len = sizeof(world) - 1;

void print_hello()
{
    (void)write(1, hello, hello_len);
}

void print_world()
{
    (void)write(1, world, world_len);
}

[[noreturn]]
void start(void)
{
    print_hello();
    print_world();
    exit(0);
}
