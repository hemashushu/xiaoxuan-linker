// Program summary:
// - Exit with status code 199.

// ## pie-import.c
//
// Demonstrates R_X86_64_PLT32 and R_X86_64_GOTPCREL relocations
//
// When compiled with -fpic, accesses to extern symbols generate:
//
//   R_X86_64_PLT32   (value 4):
//     A 32-bit PC-relative relocation for a call to an external function.
//     The linker resolves it to a PLT stub (for dynamic linking) or directly
//     to the function's address (for static linking, no PLT needed).
//     Formula: L + A - P  (L = PLT entry address, A = addend, P = relocation site)
//
//   R_X86_64_GOTPCREL (value 9):
//     A 32-bit PC-relative relocation for a load of an external data symbol's
//     address from the GOT. The linker fills the GOT slot with the symbol's
//     final address; the code reads the pointer from the GOT at runtime.
//     Formula: G + A - P  (G = GOT slot address, A = addend, P = relocation site)
//
// NOTE: Modern GCC (>= 7) defaults to emitting R_X86_64_GOTPCRELX (value 41) or
//       R_X86_64_REX_GOTPCRELX (value 42) -- optimized variants that allow the linker
//       to rewrite the instruction sequence for better performance. To force the
//       original R_X86_64_GOTPCREL, pass -Wa,-mrelax-relocations=no to GCC.
//
// Build commands:
//
//   Compile to relocatable object (generates R_X86_64_PLT32 + R_X86_64_REX_GOTPCRELX):
//     gcc -c -fpic -o pie-import.o pie-import.c
//
//   Compile to relocatable object (force classic R_X86_64_GOTPCREL instead):
//     gcc -c -fpic -Wa,-mrelax-relocations=no -o pie-import.o pie-import.c
//
//   Inspect relocations:
//     readelf -r pie-import.o
//
//   Link into a static executable (for static linker testing):
//     gcc -static -o pie.elf pie-export.o pie-import.o
//
//   Run and check exit code (expected: 199 = extern_func() + extern_var = 100 + 99):
//     ./pie.elf; echo "exit code: $?"

extern int foo;            // accessed via GOT -> R_X86_64_GOTPCREL (or GOTPCRELX)
extern int foo_plus(void); // called via PLT  -> R_X86_64_PLT32

int main(void)
{
    // call through PLT: generates R_X86_64_PLT32 for foo_plus
    int a = foo_plus(); // 100

    // load via GOT: generates R_X86_64_GOTPCREL (or REX_GOTPCRELX) for foo
    int b = foo; // 99

    return a + b; // exit code 199
}
