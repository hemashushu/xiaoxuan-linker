// Thread-Local Storage (TLS) example
//
// Demonstrates:
//   - .tdata: initialized TLS variable (tls_var_a = 11)
//   - .tbss:  zero-initialized TLS variable (tls_var_b = 0)
//   - .data:  regular global variable (my_var = 42)
//
// Logic: read tls_var_a, write its value into tls_var_b, then exit with my_var (42).
//
// TLS model notes:
//   local-exec  — offset from thread pointer is a link-time constant (R_X86_64_TPOFF32).
//                 Valid for executables (both non-PIE ET_EXEC and PIE ET_DYN). Simplest.
//   initial-exec — offset loaded from GOT at runtime (R_X86_64_GOTTPOFF).
//                 Valid for executables and shared libs loaded at startup (not dlopen).
//   global-dynamic — calls __tls_get_addr() at runtime (R_X86_64_TLSGD).
//                 Required only for dlopen'd shared libs. NOT needed for PIE executables.
//
//   PIC/PIE and local-exec are orthogonal: -fpie + -ftls-model=local-exec is valid.
//   GCC automatically uses local-exec for PIE executables when the variable is in the
//   same module, so the TLS relocations are identical with or without -fpie.
//
// Build commands:
//
//   [1] Compile to relocatable object (local-exec, generates R_X86_64_TPOFF32):
//     gcc -c -O1 -ftls-model=local-exec -o tls.o tls.c
//
//   [1.1] Link to static non-PIE executable (ET_EXEC):
//     gcc -static -O1 -ftls-model=local-exec -o tls.elf tls.c
//
//   [1.2] Link to static PIE executable (ET_DYN, position-independent):
//     gcc -static-pie -O1 -ftls-model=local-exec -o tls.elf tls.c
//
//   [2] Compile with global-dynamic (generates R_X86_64_TLSGD):
//     gcc -c -O1 -ftls-model=global-dynamic -o tls_gd.o tls.c


#include <stdlib.h>

// thread-local variables: placed in .tdata (initialized) and .tbss (zero-init)
__thread long tls_var_a = 11;
__thread long tls_var_b = 0;

// regular global variable: placed in .data
long my_var = 42;

int main(void)
{
    // read tls_var_a and write its value into tls_var_b
    tls_var_b = tls_var_a;

    // exit with my_var as the exit code (expected: 42)
    return (int)my_var;
}
