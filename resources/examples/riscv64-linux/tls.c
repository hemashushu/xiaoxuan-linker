// Program summary:
// - Exit with status code 66.

// Thread-Local Storage (TLS) example
//
// Demonstrates:
//   - .tdata: initialized TLS variable (tls_var_a = 11, tls_var_b = 13)
//   - .tbss:  zero-initialized TLS variable (tls_var_c = 0)
//   - .data:  regular global variable (my_var = 42)
//
// Logic: read tls_var_a, write its value into tls_var_c, then
// read tls_var_b and my_var, add them together, and return the result as the exit code.
//
// TLS model notes:
//   local-exec  -- offset from thread pointer is a link-time constant (R_X86_64_TPOFF32).
//                 Valid for executables (both non-PIE ET_EXEC and PIE ET_DYN). Simplest.
//   initial-exec -- offset loaded from GOT at runtime (R_X86_64_GOTTPOFF).
//                 Valid for executables and shared libs loaded at startup (not dlopen).
//   global-dynamic -- calls __tls_get_addr() at runtime (R_X86_64_TLSGD).
//                 Required only for dlopen'd shared libs. NOT needed for PIE executables.
//
//   PIC/PIE and local-exec are orthogonal: -fpie + -ftls-model=local-exec is valid.
//   GCC automatically uses local-exec for PIE executables when the variable is in the
//   same module, so the TLS relocations are identical with or without -fpie.
//
// Build commands:
//
//   [1.1] Compile to relocatable object (with local-exec model, generates R_X86_64_TPOFF32):
//     gcc -c -ftls-model=local-exec -o tls.o tls.c
//
//   [1.2] Link to non-PIE executable (ET_EXEC):
//     gcc -ftls-model=local-exec -o tls.elf tls.c
//
//   [2.1] Compile with global-dynamic (generates R_X86_64_TLSGD):
//     gcc -c -ftls-model=global-dynamic -o tls_gd.o tls.c
//
//   [2.2] Link to non-PIE executable (ET_EXEC):
//     gcc -ftls-model=global-dynamic -o tls_gd.elf tls_gd.o

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
