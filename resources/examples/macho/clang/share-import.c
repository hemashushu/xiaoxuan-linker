// Ported from resources/examples/elf/gcc/share-import.c (Linux) to macOS.
//
// Program summary:
// - Exit with status code 199.
//
// ## share-import.c (macOS/Mach-O version)
//
// Demonstrates dynamic-library symbol import on Mach-O: calls to
// `foo_plus` go through a lazy symbol stub (the dyld-stub-binder mechanism,
// analogous to ELF's PLT), and loads of `foo` go through a non-lazy
// (GOT-equivalent) pointer that dyld binds at load time.
//
// Build commands:
//
//   Compile to relocatable object:
//     clang -arch arm64 -std=c23 -c -O0 -o share-import.o share-import.c
//
//   Link against the dynamic library built from share-export.c, using
//   `@executable_path` (the Mach-O equivalent of ELF's `$ORIGIN`) so the
//   dynamic library is found next to the executable without needing
//   DYLD_LIBRARY_PATH:
//     clang -arch arm64 -o share.macho share-import.c \
//         -L. -Wl,-rpath,@executable_path -lshare-export
//
//   Run and check exit code (expected: 199 = foo_plus() + foo = 100 + 99):
//     ./share.macho; echo "exit code: $?"
//
//   Check the dynamic linking info of the executable:
//     otool -L share.macho

extern int foo;            // loaded via a non-lazy (GOT-equivalent) pointer
extern int foo_plus(void); // called via a lazy symbol stub (PLT-equivalent)

int main(void)
{
    // call through the lazy stub: resolved to libshare-export.dylib's foo_plus
    int a = foo_plus(); // 100

    // load through the non-lazy pointer: resolved to libshare-export.dylib's foo
    int b = foo; // 99

    return a + b; // exit code 199
}
