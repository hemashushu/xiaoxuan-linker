// Program summary:
// - Exit with status code 126.

// ## relocate-data-tls.c
//
// Demonstrates pointer-typed globals that produce R_X86_64_64 relocation
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
// - const pointer -> .rodata  -> .rela.rodata
// - non-const ptr -> .data    -> .rela.data
// - TLS pointer   -> .tdata   -> .rela.tdata
//
// Build commands:
//
//   Compile to relocatable object (-fno-pie overrides distro default-pie):
//     gcc -c -O0 -fno-pie -o pointer-in-tls.o pointer-in-tls.c
//
//   Inspect relocation sections (expect .rela.rodata, .rela.data, .rela.tdata):
//     readelf -r pointer-in-tls.o
//
//   Link and run (expected exit code: 126 = 42 + 42 + 42, clamped to uint8):
//     gcc -static -O0 -fno-pie -o pointer-in-tls.elf pointer-in-tls.c
//     ./pointer-in-tls.elf; echo "exit code: $?"

// The target symbol whose address is stored as a pointer in each section.
int target = 42;

// Stored in .rodata (because the pointer itself is const and has no other
// writable qualifiers). Produces a R_X86_64_64 entry in .rela.rodata.
const int *const rodata_ptr = &target;

// Stored in .data (non-const pointer). Produces a R_X86_64_64 entry in .rela.data.
int *data_ptr = &target;

// Stored in .tdata (TLS template for initialized thread-local variables).
// The initial value (&target) is a link-time constant address, so the compiler
// emits a R_X86_64_64 entry in .rela.tdata for the linker to resolve.
__thread int *tdata_ptr = &target;

int main(void)
{
    // dereference each pointer so the compiler cannot optimize them away
    return *rodata_ptr + *data_ptr + *tdata_ptr;  // 42 + 42 + 42 = 126
}
