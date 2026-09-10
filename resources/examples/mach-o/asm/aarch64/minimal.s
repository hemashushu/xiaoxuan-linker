// Ported from resources/examples/elf/asm/aarch64/minimal.s (Linux/ELF) to
// macOS/Mach-O. Differences from the Linux version:
// - Symbols use the Mach-O leading-underscore convention (`_start`).
// - Exit is invoked via the XNU/BSD syscall ABI: the syscall number is
//   ORed with the "Unix/BSD" class bit (0x2000000) and placed in x16
//   (instead of x8 on Linux), then `svc #0x80` (instead of `svc #0`).
//
// Program summary:
// - Exit with status code 42.

.section __TEXT,__text,regular,pure_instructions
.globl _start

// fn _start() -> void
_start:
    mov x0, #42                    // exit status code in x0
    movz x16, #0x0001              // syscall number for exit (1)
    movk x16, #0x0200, lsl #16     // | 0x2000000 (BSD syscall class)
    svc #0x80
