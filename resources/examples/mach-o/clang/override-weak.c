// Ported from resources/examples/elf/gcc/override-weak.c (Linux) to macOS.
// No source changes were needed; `__attribute__((weak))` is supported
// identically by clang on Mach-O.

__attribute__((weak)) int foo()
{
    return 11;
}

__attribute__((weak)) int bar()
{
    return 13;
}
