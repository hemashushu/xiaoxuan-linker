# Example Programs

This directory contains Linux example programs.

## Building the Examples

To rebuild all examples:

```bash
./rebuild.sh
```

## Assembly Examples

### minimal.s

- Purpose: Minimal x86_64 program that exits with status code 42
- Demonstrates: Basic `syscall` usage
- Exit code: 42

### function.s

- Purpose: Demonstrates function calls and string output to stdout
- Demonstrates: Function calling convention (rdi, rsi, rdx, rcx, r8, r9 for args, rax for return value)
- Output: "Hello, world!\n"
- Exit code: 0

### data.s

- Purpose: Demonstrates global data manipulation across different sections
- Demonstrates:
  - `.rodata` section for read-only data
  - `.data` section for read-write initialized data
  - `.bss` section for uninitialized data
- Exit code: 24 (result of: (11-1) + (13+1) = 10 + 14)

### symbol-export.s

- Purpose: Defines global symbols and functions that can be imported by other modules
- Demonstrates: Symbol export for linking with other object files
- Symbols exported: `foo`, `bar`, `a`, `b`, `x`, `y`, `dec()`, `inc()`

### symbol-import.s

- Purpose: Imports and uses external symbols from symbol-export.s
- Demonstrates: Symbol resolution and linking multiple object files
- Exit code: 24

### override-weak.s

- Purpose: Defines weak symbols that can be overridden
- Demonstrates: Weak symbol definition (functions `foo()` and `bar()` that return 11 and 13)

### override-strong.s

- Purpose: Provides a strong override for the weak `bar` symbol from override-weak.s
- Demonstrates: Symbol resolution with weak and strong definitions
- Exit code: 53 (11 + 42, where 42 comes from the overridden bar())

### relocate-within-data.s

- Purpose: Demonstrates pointer relocations across different data sections
- Demonstrates:
  - Pointers in `.rodata` section (read-only pointer data)
  - Pointers in `.data` section (read-write pointer data)
  - Function pointers for indirect function calls
  - Dynamic memory access patterns
- Exit code: 24

## C Examples

### pie-export.c & pie-import.c

- Purpose: Demonstrates Position-Independent Executable (PIE) and relocation types
- Demonstrates: R_X86_64_PLT32 (PLT calls) and R_X86_64_GOTPCREL (GOT references)
- Exit code: 199 (100 + 99)

### tls.c

- Purpose: Demonstrates Thread-Local Storage (TLS)
- Demonstrates: Different TLS models:
  - `local-exec`: For TLS variables in the main executable (generates R_X86_64_TPOFF32)
  - `global-dynamic`: For dlopen'd shared libraries (generates R_X86_64_TLSGD)
- Exit code: 66 (11 + 13 + 42)

### relocate-within-data-tls.c

- Purpose: Demonstrates pointer relocations in TLS sections
- Demonstrates: Relocations in `.rela.rodata`, `.rela.data`, and `.rela.tdata` sections
- Exit code: 126 (42 + 42 + 42)

## Running the Examples

To run an example, use the following command if the current architecture is x86_64:

```bash
./ELF_NAME # Replace ELF_NAME with the name of the executable
```

Or use `qemu-x86_64` if running on a different architecture:

```bash
qemu-x86_64 ./ELF_NAME # Replace ELF_NAME with the name of the executable
```

## Testing

To run all tests:

```bash
./test.sh
```
