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

// __DATA,__data
long num0 = 11;
long num1 = 13;

long dec(long x)
{
    return x - 1;
}

long inc(long x)
{
    return x + 1;
}

// __DATA,__data
long (*addr_fn_dec)(long) = dec;
long (*addr_fn_inc)(long) = inc;

// __DATA_CONST,__const
long *const addr_num0 = &num0;
long *const addr_num1 = &num1;

int main(void)
{
    long i = *addr_num0;     // i = 11
    long j = addr_fn_dec(i); // j = 10
    num0 = j;                // num0 = 10

    long m = *addr_num1;     // m = 13
    long n = addr_fn_inc(m); // n = 14
    num1 = n;                // num1 = 14

    long r = num0 + num1; // r = 10 + 14 = 24
    return (int)r;
}
