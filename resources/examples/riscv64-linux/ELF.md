# ELF Explained: RISC-V64 Linux Object and Executable Files

## Calling Convention

The RISC-V64 calling convention uses:

- Integer arguments: a0-a7 (x10-x17) - up to 8 arguments
- Floating-point arguments: fa0-fa7 (f10-f17)
- Return value: a0 (and a1 for 128-bit values in RV64)
- Return address: ra (x1)
- Stack pointer: sp (x2)
- Caller-saved registers: a0-a7, t0-t6
- Callee-saved registers: s0-s11, sp, gp, tp

## Syscall Convention

RISC-V64 syscalls use:

- Syscall number: a7
- Arguments: a0-a5
- Instruction: `ecall`
- Return value: a0

### Common Syscall Numbers

- exit: 93
- write: 64
- read: 63

## Key Differences from x86-64

1. Instruction Set: RISC-V has simpler instruction encoding compared to x86-64
2. Registers: RISC-V uses a different register naming scheme (x0-x31 with aliases)
3. Addressing: Load address using `la` pseudo-instruction, setup `gp` register for data access
4. Calling Convention: Different register usage and calling mechanism
5. Syscall Number Mapping: Different syscall numbers for the same operations

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
