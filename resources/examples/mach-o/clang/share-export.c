// Ported from resources/examples/elf/gcc/share-export.c (Linux) to macOS.
//
// ## share-export.c (macOS/Mach-O version)
//
// Defines the external symbols shared via a dynamic library (`.dylib`)
// instead of an ELF shared object (`.so`).
//
// This file provides:
//
// - foo: a global variable accessed via the (non-lazy) pointer / GOT-style
//   indirection in the app
// - foo_plus: a function called via the lazy-binding stub (PLT-equivalent)
//   in the app
//
// Build commands (see share-import.c for the relocations of interest):
//
//   Compile this library to a relocatable object:
//     clang -arch arm64 -std=c23 -c -O0 -fPIC -o share-export.o share-export.c
//
//   Link object into a dynamic library (to observe stub/GOT-style
//   indirection at runtime):
//     clang -arch arm64 -dynamiclib -install_name @rpath/libshare-export.dylib \
//         -o libshare-export.dylib share-export.o
//
//   Note: unlike ELF, Mach-O code is always position-independent by default
//   on arm64 (there is no `-fno-pic`/`-fno-pie` equivalent needed here).

int foo = 99;

int foo_plus()
{
    return foo + 1; // returns 100
}
