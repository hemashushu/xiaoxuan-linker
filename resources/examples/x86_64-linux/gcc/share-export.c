// ## share-export.c
//
// Defines the external symbols
//
// This file provides:
//
// - foo: a global variable accessed via GOT in the app
// - foo_plus: a function called via PLT in the app
//
// Build commands (see share-import.c for the relocations of interest):
//
//   Compile this library to a relocatable object:
//     gcc -c -fpic -o share-export.o share-export.c
//
//   Link object into a shared library (to observe PLT/GOT at runtime):
//     gcc -shared -o share-export.so share-export.o
//
//   `-fpic` generates PLT/GOT indirection for global symbols, the
//   GCC shipped with most modern Linux distributions (Arch, Ubuntu, Fedora, etc.) configures
//   GCC with --enable-default-pie, meaning -fpie is active even when no flag is given.
//   But -fpic is not enabled by default, so we need to explicitly pass -fpic to the compiler to generate
//   the PLT/GOT indirection for the global symbols in this shared library.

int foo = 99;

int foo_plus()
{
    return foo + 1;  // returns 100
}
