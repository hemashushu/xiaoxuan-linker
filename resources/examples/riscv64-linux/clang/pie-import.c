// Program summary:
// - Exit with status code 199.

// ## pie-import.c
//
// Demonstrates RISC-V PLT and GOT relocations for external symbols.
//
// When compiled with -fpic, accesses to extern symbols generate:
//
//   R_RISCV_CALL_PLT:
//     A function-call relocation used for an external branch target. The linker
//     resolves it to a PLT entry for dynamic linking, or directly to the target
//     function for static linking.
//
//   R_RISCV_GOT_HI20 + R_RISCV_PCREL_LO12_I:
//     A two-instruction sequence that materializes the GOT entry address for an
//     external data symbol, then loads the pointer from that slot.
//
//   R_RISCV_RELAX:
//     An auxiliary marker that allows the linker to relax the instruction
//     sequence when a shorter encoding is possible.
//
// Build commands:
//
//   Compile to relocatable object:
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

extern int foo;            // accessed via GOT -> GOT_HI20 + PCREL_LO12_I
extern int foo_plus(void); // called via PLT  -> CALL_PLT

int main(void)
{
    // call through PLT: generates R_RISCV_CALL_PLT for foo_plus
    int a = foo_plus(); // 100

    // load via GOT: generates GOT_HI20 + PCREL_LO12_I for foo
    int b = foo; // 99

    return a + b; // exit code 199
}
