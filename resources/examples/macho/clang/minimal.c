// Ported from resources/examples/elf/gcc/minimal.c (Linux) to macOS.
//
// The entry function is named `start` (not `_start`): Mach-O/Darwin C
// symbols get an implicit leading underscore, so `start` becomes the
// linker symbol `_start`, matching the `-e _start` entry point used by
// build.sh.
//
// Program summary:
// - Exit with status code 42.

#include "common.in"

[[noreturn]]
void start(void)
{
    exit(42);
}
