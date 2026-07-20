// Program summary:
// - Exit with status code 66.

// Thread-Local Storage (TLS) example
//
// Demonstrates:
//   - .tdata: initialized TLS variables (foo = 11, bar = 13)
//   - .tbss:  zero-initialized TLS variable (x = 0)
//   - .data:  regular global variable (abc = 42)
//
// Logic: decrement `foo`, increment `bar`, store their sum in `x`, then add
// `abc` and return the final result as the exit code.
//
// TLS model notes:
//   local-exec  -- offset from thread pointer is encoded with the relocation pair
//                 R_AARCH64_TLSLE_ADD_TPREL_HI12 + R_AARCH64_TLSLE_ADD_TPREL_LO12_NC.
//                 Valid for executables (both non-PIE ET_EXEC and PIE ET_DYN). Simplest.
//   initial-exec -- offset is typically loaded via the GOT for symbols that may be
//                 resolved by the dynamic linker.
//   global-dynamic -- may call __tls_get_addr() for fully dynamic TLS lookups.
//                 That model is required only when the TLS variable is not known to be local.
//
//   PIC/PIE and local-exec are orthogonal: -fpie + -ftls-model=local-exec is valid.
//   GCC automatically uses local-exec for PIE executables when the variable is in the
//   same module, so the TLS relocations are identical with or without -fpie.
//   In this self-contained example, even `-ftls-model=global-dynamic` is relaxed
//   back to local-exec style relocations because all TLS symbols are defined locally.
//
// Build commands:
//
//   [1.1] Compile to relocatable object (with local-exec model, generates the
//         AArch64 TLSLE HI12/LO12 relocation pair):
//     gcc -c -ftls-model=local-exec -o tls.o tls.c
//
//   [1.2] Link to non-PIE executable (ET_EXEC):
//     gcc -ftls-model=local-exec -o tls.elf tls.c
//
//   [2.1] Compile with a global-dynamic request:
//     gcc -c -ftls-model=global-dynamic -o tls-gd.o tls.c
//
//   [2.2] Link to non-PIE executable (ET_EXEC):
//     gcc -ftls-model=global-dynamic -o tls-gd.elf tls-gd.o

#include <stdlib.h>

// thread-local variables: placed in .tdata (initialized)
__thread long foo = 11;
__thread long bar = 13;

// thread-local variables: placed in .tbss (zero-init)
__thread long x = 0;

// regular global variable: placed in .data
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
