# RISC-V 64 Example Programs

This directory contains RISC-V 64-bit Linux example programs, mirroring the functionality of the x86_64-linux examples but using RISC-V64 ISA.

## Architecture Overview

All examples are built and tested for:
- Architecture: RISC-V 64-bit (RV64)
- ABI: LP64D (long pointer 64, double-precision floating point)
- Operating System: Linux

## Building the Examples

The build process uses:
- Assembler: `riscv64-linux-gnu-as` - GNU assembler for RISC-V
- Linker: `riscv64-linux-gnu-ld` - GNU linker for RISC-V
- Compiler: `riscv64-linux-gnu-gcc` - GCC for RISC-V (for C examples)
- Emulator: `qemu-riscv64` - QEMU user-mode emulator for RISC-V

To rebuild all examples:

```bash
chmod +x rebuild.sh
./rebuild.sh
```

## Assembly Examples

### minimal.s
- Purpose: Minimal RISC-V program that exits with status code 42
- Demonstrates: Basic `ecall` syscall usage
- Exit code: 42

### function.s
- Purpose: Demonstrates function calls and string output to stdout
- Demonstrates: Function calling convention (a0-a7 for args, a0 for return value)
- Output: "Hello, world!\n"
- Exit code: 0

### data.s
- Purpose: Demonstrates global data manipulation across different sections
- Demonstrates:
  - `.rodata` section for read-only data
  - `.data` section for read-write initialized data
  - `.bss` section for uninitialized data
  - Global pointer (gp) initialization
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
- Demonstrates: R_RISCV_CALL (PLT calls) and R_RISCV_GOT_HI20/LO12 (GOT references)
- Exit code: 199 (100 + 99)

### tls.c
- Purpose: Demonstrates Thread-Local Storage (TLS)
- Demonstrates: Different TLS models:
  - `local-exec`: For TLS variables in the main executable (generates R_RISCV_TPREL32)
  - `global-dynamic`: For dlopen'd shared libraries (generates R_RISCV_TLS_GD_HI20/LO12)
- Exit code: 66 (11 + 13 + 42)

### relocate-within-data-tls.c
- Purpose: Demonstrates pointer relocations in TLS sections
- Demonstrates: Relocations in `.rela.rodata`, `.rela.data`, and `.rela.tdata` sections
- Exit code: 126 (42 + 42 + 42)

## Calling Convention (RISC-V64)

The RISC-V64 calling convention uses:
- Integer arguments: a0-a7 (x10-x17) - up to 8 arguments
- Floating-point arguments: fa0-fa7 (f10-f17)
- Return value: a0 (and a1 for 128-bit values in RV64)
- Return address: ra (x1)
- Stack pointer: sp (x2)
- Caller-saved registers: a0-a7, t0-t6
- Callee-saved registers: s0-s11, sp, gp, tp

## Syscall Convention (RISC-V64)

RISC-V64 syscalls use:
- Syscall number: a7
- Arguments: a0-a5
- Instruction: `ecall`
- Return value: a0

### Common Syscall Numbers (RISC-V64)
- exit: 93
- write: 64
- read: 63

## Key Differences from x86-64

1. Instruction Set: RISC-V has simpler instruction encoding compared to x86-64
2. Registers: RISC-V uses a different register naming scheme (x0-x31 with aliases)
3. Assembler Syntax: Uses AT&T syntax (`.s` files with `as`) instead of Intel syntax
4. Addressing: Load address using `la` pseudo-instruction, setup `gp` register for data access
5. Calling Convention: Different register usage and calling mechanism
6. Syscall Number Mapping: Different syscall numbers for the same operations

## Testing

All examples can be tested using QEMU:

```bash
/usr/bin/qemu-riscv64 ./minimal.elf
echo "Exit code: $?"
```

## Implementation Notes

1. Global Pointer Setup: Many examples initialize the global pointer (gp) with `la gp, __global_pointer$` to enable efficient access to small data sections.

2. Position-Independent Code: While these examples are mostly statically linked, the code uses position-independent addressing patterns where applicable.

3. Relocation: The examples demonstrate various relocation types that the linker must resolve:
   - Absolute addresses in data sections (R_RISCV_64)
   - PC-relative branches and calls (R_RISCV_BRANCH, R_RISCV_JAL)
   - Symbol references (R_RISCV_HI20/LO12)

## Further Reading

- [RISC-V Specification](https://riscv.org/technical/specifications/)
- [RISC-V Calling Conventions](https://github.com/riscv-non-profit/riscv-elf-psabi-doc)
- [RISC-V ELF Psabi](https://github.com/riscv-non-profit/riscv-elf-psabi-doc/blob/master/riscv-elf.md)
