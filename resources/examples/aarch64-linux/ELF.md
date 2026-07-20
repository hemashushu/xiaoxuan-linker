# ELF Explained: AArch64 Linux Object and Executable Files

## Calling Convention

The AArch64 calling convention uses:

- Integer arguments: x0-x7 - up to 8 arguments
- Floating-point arguments: v0-v7
- Return value: x0 (and x1 for larger integer returns)
- Return address: lr (x30)
- Stack pointer: sp
- Caller-saved registers: x0-x18, v0-v7, v16-v31
- Callee-saved registers: x19-x29, v8-v15, sp

## Syscall Convention

AArch64 Linux syscalls use:

- Syscall number: x8
- Arguments: x0-x5
- Instruction: `svc #0`
- Return value: x0

### Common Syscall Numbers

- exit: 93
- write: 64
- read: 63

## Key Differences from x86-64

1. Instruction Set: AArch64 uses a fixed-width 32-bit RISC instruction encoding.
2. Registers: AArch64 has 31 general-purpose 64-bit registers (`x0`-`x30`).
3. Addressing: Global symbols are commonly accessed with `ADRP` + low-12-bit `ADD/LDR/STR` sequences.
4. Calling Convention: Function arguments and return values use `x0`-`x7` and `x0`.
5. Syscall Number Mapping: Linux syscall IDs differ from x86-64.

## Implementation Notes

1. Global Data Addressing: Unlike RISC-V small-data style `gp` access, these AArch64 examples do not require a dedicated global pointer initialization in `_start`.

   Typical code shape for symbol access:

   ```asm
   adrp x0, symbol
   ldr x1, [x0, :lo12:symbol]
   ```

   The linker resolves the ADRP/LO12 relocation pair to the final symbol address.

2. Position-Independent Code: `-fpic`/`-fpie` code typically uses ADRP-based PC-relative sequences and GOT/PLT indirection for external symbols.

3. Relocation: The examples demonstrate common AArch64 relocation patterns:
   - Absolute 64-bit pointers in data/TLS templates (`R_AARCH64_ABS64`)
   - ADRP page-relative + low-12-bit fixups for direct symbol access (`R_AARCH64_ADR_PREL_PG_HI21`, `R_AARCH64_ADD_ABS_LO12_NC`, `R_AARCH64_LDST64_ABS_LO12_NC`)
   - External function calls through PLT (`R_AARCH64_CALL26`)
   - External data access through GOT (`R_AARCH64_ADR_GOT_PAGE`, `R_AARCH64_LD64_GOT_LO12_NC`)
   - TLS local-exec offsets (`R_AARCH64_TLSLE_ADD_TPREL_HI12`, `R_AARCH64_TLSLE_ADD_TPREL_LO12_NC`)

## Further Reading

- [AAPCS64 (Procedure Call Standard)](https://github.com/ARM-software/abi-aa/blob/main/aapcs64/aapcs64.rst)
- [ELF for the Arm 64-bit Architecture (AAELF64)](https://github.com/ARM-software/abi-aa/blob/main/aaelf64/aaelf64.rst)
