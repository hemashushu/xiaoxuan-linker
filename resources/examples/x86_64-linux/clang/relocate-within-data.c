// Program summary:
// - Exit with status code 24.

// ## relocate-within-data.c
//
// Demonstrates pointer-typed globals that produce R_X86_64_64 relocation
// entries in three different sections of the relocatable object file:
//
// - .rela.rodata -- const pointer stored in .rodata
// - .rela.data   -- non-const pointer stored in .data
// - .rela.tdata  -- TLS pointer stored in .tdata (the per-thread TLS template)
//
// NOTE: must compile with -fno-pie to ensure pointers land in .rodata and .data.
//
// Most modern Linux distributions (Arch, Ubuntu, Fedora, etc.) configure GCC
// with --enable-default-pie, meaning -fpie is active even when no flag is given.
// With -fpie (or -fpic), GCC places relocatable const pointers in .data.rel.ro.local
// and non-const pointers in .data.rel.local, because the dynamic linker needs to
// patch those slots at load time and .rodata/.data are subject to special mapping.
// Passing -fno-pie overrides the distro default and restores the classic layout:
//
// - const pointer  -> .rodata               -> .rela.rodata
// - non-const ptr  -> .data                 -> .rela.data
// - const pointer  -> .data.rel.ro.local    -> .rela.data.rel.ro.local (if -fpie)
// - non-const ptr  -> .data.rel.local       -> .rela.data.rel.local (if -fpie)
// - TLS pointer    -> .tdata                -> .rela.tdata
//
// Build commands:
//
//   Compile to relocatable object (-fno-pie overrides distro default-pie):
//     gcc -c -O0 -fno-pie -o relocate-within-data.o relocate-within-data.c
//
//   Link and run (expected exit code: 24 = 10 + 14, clamped to uint8):
//     gcc -O0 -no-pie -o relocate-within-data.elf relocate-within-data.o
//     ./relocate-within-data.elf; echo "exit code: $?"
//
//   P.S.
//   - `-pie`, `-no-pie` is a linker option, not a compiler option.
//     The compiler flag is `-fpie`, `-fno-pie` (`-fpic`, `-fno-pic`).
//   - PIC generates PLT/GOT indirection for global symbols,
//   - PIE generates a position-independent executable, which means the entire program
//     can be loaded at any address in memory.

#include "common.in"

// .data
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

// .data
long (*pdec)(long) = dec;
long (*pinc)(long) = inc;

// .rodata
long *const pfoo = &foo;
long *const pbar = &bar;

[[noreturn]]
void _start(void)
{
    long i = *pfoo;   // i = 11
    long j = pdec(i); // j = 10
    foo = j;          // foo = 10

    long m = *pbar;   // m = 13
    long n = pinc(m); // n = 14
    bar = n;          // bar = 14

    long r = foo + bar; // k = 10 + 14 = 24
    exit(r);
}