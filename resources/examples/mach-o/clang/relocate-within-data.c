// Ported from resources/examples/elf/gcc/relocate-within-data.c (Linux) to
// macOS.
//
// Program summary:
// - Exit with status code 24.
//
// ## relocate-within-data.c (macOS/Mach-O version)
//
// Demonstrates pointer-typed globals that produce address relocations
// (ARM64_RELOC_UNSIGNED) in different sections of the object file:
//
// - __DATA_CONST,__const -- const pointer stored alongside other constants
// - __DATA,__data         -- non-const pointer and function pointers
//
// Unlike the Linux/ELF version, `-fno-pie` is not required/relevant here:
// clang on Mach-O always places pointer-typed const globals that require a
// load-time fixup into a writable-but-protectable section
// (`__DATA_CONST,__const`), never into the read-only `__TEXT` segment,
// because __TEXT is mapped shared/read-only and cannot host load-time
// relocations ("illegal text-relocation"). Plain (non-pointer) `const`
// values still end up in `__TEXT,__const` (see data.c).
//
// Build commands:
//
//   Compile to relocatable object:
//     clang -arch arm64 -std=c23 -c -O0 -o relocate-within-data.o relocate-within-data.c
//
//   Link and run (expected exit code: 24 = 10 + 14):
//     clang -arch arm64 -o relocate-within-data.macho relocate-within-data.o
//     ./relocate-within-data.macho; echo "exit code: $?"

#include "common.in"

// __DATA,__data
long foo = 11;
long bar = 13;

long dec(long x)
{
    return x - 1;
}

long inc(long x)
{
    return x + 1;
}

// __DATA,__data
long (*pdec)(long) = dec;
long (*pinc)(long) = inc;

// __DATA_CONST,__const
long *const pfoo = &foo;
long *const pbar = &bar;

[[noreturn]]
void start(void)
{
    long i = *pfoo;   // i = 11
    long j = pdec(i); // j = 10
    foo = j;          // foo = 10

    long m = *pbar;   // m = 13
    long n = pinc(m); // n = 14
    bar = n;          // bar = 14

    long r = foo + bar; // r = 10 + 14 = 24
    exit(r);
}
