// Program summary:
// - Exit with status code 199.

// ## share-import.c
//
// Demonstrates AArch64 PLT/GOT relocations for external symbols
//
// When compiled with -fpic, accesses to extern symbols generate:
//
//   R_AARCH64_CALL26:
//     A PC-relative branch relocation for a call to an external function.
//     The linker resolves it to a PLT stub (for dynamic linking) or directly
//     to the function's address (for static linking, no PLT needed).
//
//   R_AARCH64_ADR_GOT_PAGE + R_AARCH64_LD64_GOT_LO12_NC:
//     A two-instruction GOT access sequence for loading an external data symbol's
//     address. The linker fills the GOT slot with the symbol's final address;
//     the code reads that pointer from the GOT at runtime.
//
// NOTE: On AArch64, the compiler typically emits ADRP/ADD or ADRP/LDR relocation
//       pairs rather than x86-64 GOTPCRELX/REX_GOTPCRELX forms.
//
// Build commands:
//
//   Compile to relocatable object (generates AArch64 CALL26 and GOT relocation pairs):
//     gcc -c -fpic -o share-import.o share-import.c
//
//   Inspect relocations in the object file:
//     readelf -r share-import.o
//
//   Link into a static executable (for static linker testing):
//     gcc -static -o share.elf share-export.o share-import.o
//
//   Run and check exit code (expected: 199 = extern_func() + extern_var = 100 + 99):
//     ./share.elf; echo "exit code: $?"
//
//   Or link with a shared library share-export.so (to observe PLT/GOT at runtime),
//   for simplicity, use RUNPATH (or RPATH) to avoid setting LD_LIBRARY_PATH,
//   the `$ORIGIN` tells the dynamic linker to search for shared libraries in the same directory as the executable:
//     gcc -o share.elf share-import.o -L. -Wl,-rpath,'$ORIGIN' -l:share-export.so
//
//   Run and check exit code (expected: 199 = extern_func() + extern_var = 100 + 99):
//     ./share.elf; echo "exit code: $?"
//
//   P.S. the searching order for shared libraries is:
//   - RPATH:
//     RPATH -> LD_LIBRARY_PATH -> System default paths (/lib, /usr/lib, etc.)
//   - RUNPATH:
//     LD_LIBRARY_PATH -> RUNPATH -> /etc/ld.so.cache -> System default paths (/lib, /usr/lib, etc.)
//
//   Check the dynamic linking info of the executable:
//     readelf -d share.elf

extern int foo;            // accessed via GOT -> R_AARCH64_ADR_GOT_PAGE + R_AARCH64_LD64_GOT_LO12_NC
extern int foo_plus(void); // called via PLT  -> R_AARCH64_CALL26

int main(void)
{
    // call through PLT: generates R_AARCH64_CALL26 for foo_plus
    int a = foo_plus(); // 100

    // load via GOT: generates a GOT relocation pair for foo on AArch64
    int b = foo; // 99


    return a + b; // exit code 199
}
