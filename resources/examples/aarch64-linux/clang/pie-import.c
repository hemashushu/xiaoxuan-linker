// Program summary:
// - Exit with status code 199.

// ## pie-import.c
//
// Demonstrates AArch64 PLT and GOT relocations for external symbols.
//
// When compiled with -fpic, accesses to extern symbols generate:
//
//   R_AARCH64_CALL26:
//     A 26-bit PC-relative branch relocation for a call to an external
//     function. The linker resolves it to a PLT entry for dynamic linking,
//     or directly to the function body for static linking.
//
//   R_AARCH64_ADR_GOT_PAGE + R_AARCH64_LD64_GOT_LO12_NC:
//     A two-instruction sequence that computes the page of the GOT entry with
//     `adrp`, then loads the 64-bit pointer from that GOT slot.
//
// In the current object file, `readelf -r pie-import.o` shows exactly these
// three relocations in `.rela.text`: one CALL26 for `foo_plus`, and the ADRP +
// LD64 pair for `foo`.
//
// Build commands:
//
//   Compile to relocatable object (generates CALL26 + GOT-page relocations):
//     gcc -c -fpic -o pie-import.o pie-import.c
//
//   Inspect relocations:
//     readelf -r pie-import.o
//
//   Link into a static executable (for static linker testing):
//     gcc -static -o pie.elf pie-export.o pie-import.o
//
//   Run and check exit code (expected: 199 = extern_func() + extern_var = 100 + 99):
//     ./pie.elf; echo "exit code: $?"

extern int foo;            // accessed via GOT -> ADR_GOT_PAGE + LD64_GOT_LO12_NC
extern int foo_plus(void); // called via PLT  -> CALL26

int main(void)
{
    // call through PLT: generates R_AARCH64_CALL26 for foo_plus
    int a = foo_plus(); // 100

    // load via GOT: generates ADR_GOT_PAGE + LD64_GOT_LO12_NC for foo
    int b = foo; // 99

    return a + b; // exit code 199
}
