// ## pie-export.c
//
// Defines the external symbols
//
// This file provides:
//
// - foo: a global variable accessed via GOT in the app
// - foo_plus: a function called via PLT in the app
//
// Build commands (see pie-import.c for the relocations of interest):
//
//   Compile this library to a relocatable object:
//     gcc -c -fpic -o pie-export.o pie-export.c
//
//   Link both objects into a shared library (to observe PLT/GOT at runtime):
//     gcc -shared -o pie-export.so pie-export.o
//
//   Or link both into a static executable (for static linker testing):
//     gcc -static -o pie.elf pie-export.o pie-import.o

int foo = 99;

int foo_plus()
{
    return foo + 1;  // returns 100
}
