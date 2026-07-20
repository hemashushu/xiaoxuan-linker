// Program summary:
// - Exit with status code 126.

// ## relocate-data-tls.c
//
// Demonstrates pointer-typed globals that produce R_RISCV_64 relocation
// entries in three different sections of the relocatable object file:
//
// - .rela.srodata -- const pointer stored in .srodata
// - .rela.sdata   -- non-const pointer stored in .sdata
// - .rela.tdata  -- TLS pointer stored in .tdata (the per-thread TLS template)
//
// NOTE: with this RISC-V toolchain, these small objects are placed in the
// small-data sections `.srodata` and `.sdata` rather than `.rodata` and `.data`.
// `-fno-pie` is still useful here to avoid PIE-specific data-relocation sections.
//
// Most modern Linux distributions (Arch, Ubuntu, Fedora, etc.) configure GCC
// with --enable-default-pie, meaning -fpie is active even when no flag is given.
// With -fpie (or -fpic), GCC places relocatable const pointers in .data.rel.ro.local
// and non-const pointers in .data.rel.local, because the dynamic linker needs to
// patch those slots at load time and .rodata/.data are subject to special mapping.
// Passing -fno-pie overrides the distro default and restores the classic layout:
//
// - const pointer -> .srodata -> .rela.srodata
// - non-const ptr -> .sdata   -> .rela.sdata
// - TLS pointer   -> .tdata   -> .rela.tdata
//
// Build commands:
//
//   Compile to relocatable object (-fno-pie overrides distro default-pie):
//     gcc -c -O0 -fno-pie -o relocate-within-data-tls.o relocate-within-data-tls.c
//
//   Inspect relocation sections (expect .rela.srodata, .rela.sdata, .rela.tdata):
//     readelf -r relocate-within-data-tls.o
//
//   Link and run (expected exit code: 126 = 42 + 42 + 42, clamped to uint8):
//     gcc -static -O0 -fno-pie -o relocate-within-data-tls.elf relocate-within-data-tls.c
//     ./relocate-within-data-tls.elf; echo "exit code: $?"

// The target symbol whose address is stored as a pointer in each section.
int target = 42;

// Stored in .rodata (because the pointer itself is const and has no other
// writable qualifiers). Produces a R_RISCV_64 entry in .rela.srodata.
const int *const rodata_ptr = &target;

// Stored in .sdata (non-const pointer). Produces a R_RISCV_64 entry in .rela.sdata.
int *data_ptr = &target;

// Stored in .tdata (TLS template for initialized thread-local variables).
// The initial value (&target) is a link-time constant address, so the compiler
// emits a R_RISCV_64 entry in .rela.tdata for the linker to resolve.
__thread int *tdata_ptr = &target;

int main(void)
{
    // dereference each pointer so the compiler cannot optimize them away
    return *rodata_ptr + *data_ptr + *tdata_ptr;  // 42 + 42 + 42 = 126
}
