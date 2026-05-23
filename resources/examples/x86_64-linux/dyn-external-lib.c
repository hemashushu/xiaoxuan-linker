// dyn-lib.c — defines the external symbols used by dyn-app.c
//
// This file provides:
//   - extern_var: a global variable accessed via GOT in the app
//   - extern_func: a function called via PLT in the app
//
// Build commands (see dyn-app.c for the relocations of interest):
//
//   Compile this library to a relocatable object:
//     gcc -c -fpic -o dyn-lib.o dyn-lib.c
//
//   Link both objects into a shared library (to observe PLT/GOT at runtime):
//     gcc -shared -o dyn-lib.so dyn-lib.o
//
//   Or link both into a static executable (for static linker testing):
//     gcc -static -o dyn.elf dyn-lib.o dyn-app.o

int extern_var = 99;

int extern_func(void)
{
    return extern_var + 1;  // returns 100
}
