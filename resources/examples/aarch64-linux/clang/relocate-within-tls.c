// Program summary:
// - Exit with status code 126.

// ## relocate-data-tls.c
//
// Demonstrates pointer-typed globals that produce R_AARCH64_ABS64 relocation
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
//     gcc -c -O0 -fno-pie -o relocate-within-tls.o relocate-within-tls.c
//
//   Link and run (expected exit code: 126 = 42 + 42 + 42, clamped to uint8):
//     gcc -O0 -no-pie -o relocate-within-tls.elf relocate-within-tls.o
//     ./relocate-within-tls.elf; echo "exit code: $?"
//
//   P.S.
//   - `-pie`, `-no-pie` is a linker option, not a compiler option.
//     The compiler flag is `-fpie`, `-fno-pie` (`-fpic`, `-fno-pic`).
//   - PIC generates PLT/GOT indirection for global symbols,
//   - PIE generates a position-independent executable, which means the entire program
//     can be loaded at any address in memory.

// The target symbol whose address is stored as a pointer in each section.
int target = 42;

// Stored in .rodata (because the pointer itself is const and has no other
// writable qualifiers). Produces a R_AARCH64_ABS64 entry in .rela.rodata.
const int *const rodata_ptr = &target;

// Stored in .data (non-const pointer). Produces a R_AARCH64_ABS64 entry in .rela.data.
int *data_ptr = &target;

// Stored in .tdata (TLS template for initialized thread-local variables).
// The initial value (&target) is a link-time constant address, so the compiler
// emits a R_AARCH64_ABS64 entry in .rela.tdata for the linker to resolve.
__thread int *tdata_ptr = &target;

int main(void)
{
    // dereference each pointer so the compiler cannot optimize them away
    return *rodata_ptr + *data_ptr + *tdata_ptr;  // 42 + 42 + 42 = 126
}
