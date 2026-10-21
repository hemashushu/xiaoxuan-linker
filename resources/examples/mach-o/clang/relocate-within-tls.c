// Ported from resources/examples/elf/gcc/relocate-within-tls.c (Linux) to
// macOS.
//
// Program summary:
// - Exit with status code 126.
//
// ## relocate-within-tls.c (macOS/Mach-O version)
//
// Demonstrates pointer-typed globals whose initializer is the address of
// another global, across three different storage classes:
//
// - __TEXT,__const or __DATA_CONST,__const -- const pointer
// - __DATA,__data                          -- non-const pointer
// - TLV-initialized data (__DATA,__thread_data) -- thread-local pointer
//
// See tls.c for details on how Mach-O implements thread-local variables
// (TLV descriptors + accessor functions) instead of ELF-style TPOFF
// relocations. Because TLV access depends on the normal dyld/libSystem
// process startup, this example is linked as a regular `main`-based
// executable rather than with a custom `_start` entry point.
//
// Build commands:
//
//   clang -arch arm64 -std=c23 -O0 -o relocate-within-tls.macho relocate-within-tls.c
//   ./relocate-within-tls.macho; echo "exit code: $?"

// The target symbol whose address is stored as a pointer in each section.
int num0 = 42;

// Const pointer: the compiler places this wherever it can apply a load-time
// fixup safely (never in the read-only __TEXT segment on Mach-O).
const int *const addr_num0_ro = &num0;

// Stored in __DATA,__data (non-const pointer).
int *addr_num0_rw = &num0;

// Thread-local pointer: initializer is a link-time constant address, stored
// in the TLV initial-value template (__DATA,__thread_data).
__thread int *addr_num0_tls = &num0;

int main(void)
{
    // dereference each pointer so the compiler cannot optimize them away
    return *addr_num0_ro + *addr_num0_rw + *addr_num0_tls; // 42 + 42 + 42 = 126
}
