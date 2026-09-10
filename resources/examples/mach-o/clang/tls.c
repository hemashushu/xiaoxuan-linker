// Ported from resources/examples/elf/gcc/tls.c (Linux) to macOS.
//
// Program summary:
// - Exit with status code 66.
//
// Thread-Local Storage (TLS) example (macOS/Mach-O version)
//
// Unlike ELF (which uses TPOFF/TLSGD-style relocations resolved directly
// against a thread pointer), Mach-O implements thread-local variables via
// "TLV" (Thread-Local Variables): each `__thread` global is emitted as a
// descriptor in `__DATA,__thread_vars`, with its initializer data placed in
// `__DATA,__thread_data` (or `__DATA,__thread_bss` if zero-initialized).
// Accessing a TLV global compiles to a call through the descriptor's
// accessor function pointer (set up by dyld/libSystem at load time), rather
// than a simple offset-from-thread-pointer load.
//
// Because of this, TLS examples cannot use the freestanding `-nostdlib` /
// custom `_start` approach used by minimal.c, data.c, etc.: the TLV
// accessor mechanism depends on dyld's normal process startup. This example
// is therefore linked as a regular `main`-based executable (see build.sh).
//
// Logic: decrement `foo`, increment `bar`, write `foo + bar` into `x`
// (also TLS), then add the regular global `abc` and return the result as
// the exit code.
//
// Build commands:
//
//   clang -arch arm64 -std=c23 -O0 -o tls.macho tls.c

#include <stdlib.h>

// thread-local variables (TLV descriptors in __DATA,__thread_vars,
// initial values in __DATA,__thread_data)
__thread long foo = 11;
__thread long bar = 13;

// thread-local variable, zero-initialized (__DATA,__thread_bss)
__thread long x = 0;

// regular global variable: placed in __DATA,__data
long abc = 42;

int main(void)
{
    foo--;                 // foo = 10
    bar++;                 // bar = 14
    x = foo + bar;         // x = 10 + 14 = 24
    long result = x + abc; // result = 24 + 42 = 66

    // exit with `result` as the exit code (expected: 66)
    return (int)result;
}
