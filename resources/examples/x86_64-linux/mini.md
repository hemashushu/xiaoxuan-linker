# Program "mini"

<!-- @import "[TOC]" {cmd="toc" depthFrom=2 depthTo=4 orderedList=false} -->

<!-- code_chunk_output -->

- [Source code](#source-code)
- [Calling convention](#calling-convention)
- [Assemble, link, and run](#assemble-link-and-run)
- [Object file (relocatable file)](#object-file-relocatable-file)
  - [File header](#file-header)
  - [Sections](#sections)
  - [Symbols](#symbols)
  - [Symbols via `nm`](#symbols-via-nm)
    - ['U' undefined symbol](#u-undefined-symbol)
    - ['W' weak symbol](#w-weak-symbol)
  - [Disassembly code](#disassembly-code)
  - [Disassembly data](#disassembly-data)
  - [Relocations](#relocations)
  - [Calling](#calling)
- [Executable file](#executable-file)
  - [File header (ET_EXEC)](#file-header-et_exec)
  - [Section headers](#section-headers)
  - [Program headers](#program-headers)
  - [Symbols (ET_EXEC)](#symbols-et_exec)
  - [Relocations (ET_EXEC)](#relocations-et_exec)

<!-- /code_chunk_output -->

## Source code

mini.asm:

```asm
global _start

section .data
    num dq 41           ;; read-write global variable with initial value 41

section .text

;; fn inc() -> int64_t
inc:
    mov rax, [rel num]  ;; read the value of num into rax
    add rax, 1          ;; increment rax by 1
    ret                 ;; return the incremented value in rax

;; fn _start() -> void
_start:
    call inc            ;; call the inc function, result is in rax

    ;; exit program using syscall `exit(status)`
    ;; syscall number: 60

    mov rdi, rax        ;; move the result of inc (the incremented value) into rdi, the first syscall argument
    mov rax, 60         ;; syscall number for exit
    syscall
```

## Calling convention

x86_64 System V function calling convention:

- rdi, rsi, rdx, rcx, r8, r9 for integer/pointer arguments,
- xmm0-xmm7 for floating-point arguments,
- rax for return value.

syscall convention:

- rax for syscall number,
- rdi, rsi, rdx, r10, r8, r9 for syscall arguments,
- rax for return value.

Note that syscalls use `r10` instead of `rcx` for the 4th argument. The caller must save `rcx` and `r11` if needed, because the `syscall` instruction clobbers both.

## Assemble, link, and run

```sh
nasm -f elf64 -o mini.o mini.asm
ld -o mini.elf mini.o
./mini.elf
echo $? # output: 42
```

## Object file (relocatable file)

### File header

```sh
readelf -h mini.o
```

Output:

```text
  OS/ABI:                            UNIX - System V
  ABI Version:                       0
  Type:                              REL (Relocatable file)
  Machine:                           Advanced Micro Devices X86-64
  Version:                           0x1
  Entry point address:               0x0
```

### Sections

```sh
readelf -S mini.o
```

Output:

```text
There are 7 section headers, starting at offset 0x40:

Section Headers:
  [Nr] Name              Type             Address           Offset
       Size              EntSize          Flags  Link  Info  Align
  [ 0]                   NULL             0000000000000000  00000000
       0000000000000000  0000000000000000           0     0     0
  [ 1] .data             PROGBITS         0000000000000000  00000200
       0000000000000008  0000000000000000  WA       0     0     4
  [ 2] .text             PROGBITS         0000000000000000  00000210
       000000000000001b  0000000000000000  AX       0     0     16
  [ 3] .shstrtab         STRTAB           0000000000000000  00000230
       0000000000000032  0000000000000000           0     0     1
  [ 4] .symtab           SYMTAB           0000000000000000  00000270
       00000000000000a8  0000000000000018           5     6     8
  [ 5] .strtab           STRTAB           0000000000000000  00000320
       000000000000001b  0000000000000000           0     0     1
  [ 6] .rela.text        RELA             0000000000000000  00000340
       0000000000000018  0000000000000018           4     2     8
Key to Flags:
  W (write), A (alloc), X (execute), M (merge), S (strings), I (info),
  L (link order), O (extra OS processing required), G (group), T (TLS),
  C (compressed), x (unknown), o (OS specific), E (exclude),
  D (mbind), l (large), p (processor specific)
```

The `Nr` column is the section index, for example:

| Index | Section |
|-------|---------|
| 1     | .data   |
| 2     | .text   |

General sections:

| Section Name | Section Type | Description                                          |
|--------------|--------------|------------------------------------------------------|
| .rodata      | PROGBITS     | Read-only data                                       |
| .data        | PROGBITS     | Initialized data                                     |
| .bss         | NOBITS       | Uninitialized data                                   |
| .text        | PROGBITS     | Executable code                                      |
| .symtab      | SYMTAB       | Symbol table                                         |
| .rela.text   | RELA         | Relocation entries for the .text section             |
| .shstrtab    | STRTAB       | Section header string table (used for section names) |
| .strtab      | STRTAB       | String table (used for symbol names)                 |

Common section types:

| Section Type | Description                                                                |
|--------------|----------------------------------------------------------------------------|
| PROGBITS     | A section that contains data or code.                                      |
| NOBITS       | A section that does not occupy space in the file but has a size in memory. |
| STRTAB       | A section that contains null-terminated strings.                           |
| SYMTAB       | A section that contains symbol table entries.                              |
| RELA         | A section that contains relocation entries with addends.                   |

Fields:

- `Address` is the virtual address of the section in memory (which is `0x0` for object files, since they are not yet linked).
- `Offset` is the offset of the section in the **file**, which is used for locating or loading the section's data within the file.
- `EntSize` is the size of each entry in the section. It is meaningful only for sections that contain fixed-size entries (such as `.symtab`, `.rela.text`, `.dynsym`, and `init_array`). For example, `.symtab` has `EntSize = 0x18` (24 bytes) because each symbol table entry is 24 bytes.
- `Link` and `Info` are used for sections that have a relationship with other sections, for example:
  - The `.rela.text` section has `Link` value `4`, which means it is linked to the section with index `4` (which is the `.symtab` section), and it has `Info` value `2`, which means the relocation entries in `.rela.text` apply to the section with index `2` (which is the `.text` section).
  - The `.symtab` section has `Link` value `5`, which means it is linked to the section with index `5` (which is the `.strtab` section), and it has `Info` value `6`, which means the first 6 entries in the symbol table are reserved for special symbols (like the null symbol, file symbols, and section symbols), and the actual symbols start from index `6`.

In short:

- For a relocation section, `link = symtab_section_index` and `info = target_section_index`.
- For a symbol table section, `link = strtab_section_index` and `info = first_non_local_symbol`.

### Symbols

```sh
readelf -s mini.o
```

Output:

```text
Symbol table '.symtab' contains 7 entries:
   Num:    Value          Size Type    Bind   Vis      Ndx Name
     0: 0000000000000000     0 NOTYPE  LOCAL  DEFAULT  UND
     1: 0000000000000000     0 FILE    LOCAL  DEFAULT  ABS mini.asm
     2: 0000000000000000     0 SECTION LOCAL  DEFAULT    1 .data
     3: 0000000000000000     0 SECTION LOCAL  DEFAULT    2 .text
     4: 0000000000000000     0 NOTYPE  LOCAL  DEFAULT    1 num
     5: 0000000000000000     0 NOTYPE  LOCAL  DEFAULT    2 inc
     6: 000000000000000c     0 NOTYPE  GLOBAL DEFAULT    2 _start
```

Fields:

- `Value` is the offset of the symbol within **its section** (not the final address, which is determined by the linker) when the object file is `ET_REL` (relocatable), and `Ndx` is the index of the section it belongs to (1 for `.data`, 2 for `.text`). In the `ET_EXEC` (executable) file, the `Value` is the virtual address of the symbol in memory.
- `Size` is the size of the symbol. It is `0` here because NASM does not emit symbol sizes in this case.

### Symbols via `nm`

```sh
nm mini.o
```

Output:

```text
0000000000000000 t inc
0000000000000000 d num
000000000000000c T _start
```

| Type | Description      |
|------|------------------|
| T/t  | text             |
| D/d  | initialized data |
| B/b  | BSS              |
| R/r  | read-only data   |
| U    | undefined symbol |
| W    | weak             |
| C    | common           |

The type is determined by the symbol section index (`st_shndx`) and `st_info`. For example:

- `num` is in the `.data` section (section index 1), so it is a `D` symbol.
- `inc` and `_start` are in the `.text` section (section index 2), so they are `T` symbols.
- If `st_shndx` is `SHN_UNDEF`, then the symbol is an undefined symbol (`U`).
- If `st_shndx` is `SHN_COMMON`, then the symbol is a common symbol (`C`).
- If the symbol has the `STB_WEAK` binding (from the field `st_info` high 4 bits), then it is a weak symbol (`W`).

#### 'U' undefined symbol

`U` means an undefined (imported) symbol. It is referenced in this object file but defined elsewhere, so the linker must resolve it from another object file or library.

For example, if we have the following code that references an external function `puts`:

```asm
extern puts

section .text
global _start

_start:
    call puts
```

Output of `nm`:

```text
                 U puts
0000000000000000 T _start
```

And the output of `readelf -r`:

```text
RELOCATION RECORDS FOR [.text]:
OFFSET           TYPE              VALUE
0000000000000001 R_X86_64_PC32     puts-0x0000000000000004
```

#### 'W' weak symbol

`W` means a weak symbol. A weak definition can be overridden by a strong definition of the same name. If multiple weak definitions exist, the linker selects one (typically the first one encountered).

Strong vs weak:

| Combination       | Result                              |
|-------------------|-------------------------------------|
| strong + weak     | strong                              |
| weak + weak       | any of them (usually the first one) |
| strong + strong   | linker error                        |

### Disassembly code

```sh
objdump -M intel -d -r mini.o
```

Output:

```text
mini.o:     file format elf64-x86-64

Disassembly of section .text:

0000000000000000 <inc>:
    0:   48 8b 05 00 00 00 00    mov    rax,QWORD PTR [rip+0x0]        # 7 <inc+0x7>
                         3: R_X86_64_PC32        .data-0x4
    7:   48 83 c0 01             add    rax,0x1
    b:   c3                      ret

000000000000000c <_start>:
    c:   e8 ef ff ff ff          call   0 <inc>
   11:   48 89 c7                mov    rdi,rax
   14:   b8 3c 00 00 00          mov    eax,0x3c
   19:   0f 05                   syscall
```

### Disassembly data

```sh
objdump -s -j .data mini.o
```

Output:

```text
mini.o:     file format elf64-x86-64

Contents of section .data:
 0000 29000000 00000000                    ).......
```

### Relocations

In the disassembly, `<inc+0x7>` reflects a temporary relocation target (placeholder) that the linker will later resolve. The `0x7` comes from `effective_address = next_instruction_rip + disp32`, where `next_instruction_rip` is `0x7` for this instruction.

The `0x7` is shown for readability in disassembly; it is not directly encoded in the instruction bytes. The actual encoding is `48 8b 05 00 00 00 00`, where `00 00 00 00` is the placeholder to be patched by relocation.

Check out the relocation section:

```sh
readelf -r mini.o
```

Output:

```text
Relocation section '.rela.text' at offset 0x340 contains 1 entry:
  Offset          Info           Type           Sym. Value    Sym. Name + Addend
000000000003  000200000002 R_X86_64_PC32     0000000000000000 .data - 4
```

Fields:

- `Offset` is the offset within the section where the relocation applies (in this case, `0x3` in the `.text` section).
- `Info` encodes the symbol index and relocation type. In this case, `000200000002` means symbol index `2` and type `R_X86_64_PC32`, using `info = sym_index << 32 | type`.
- `Type` is the type of relocation (in this case, `R_X86_64_PC32`)
- `Sym. Value` is the offset of the symbol that the relocation references (in this case, `0x0` because the symbol is `.data`, which is the section containing `num`, and its offset is `0x0`), it is used for calculating the final address during linking.
- `Sym. Name` is the symbol that the relocation references (in this case, `.data`, which is the section containing `num`).
- `+ Addend` indicates that the linker should add `-4` to the symbol's address (which is the section `.data` in this case) when applying the relocation.

When the CPU executes `mov rax, [rip + disp32]`, the effective address is `effective_address = next_instruction_rip + disp32`. Therefore, `disp32 = target_address - next_instruction_rip`.

The linker replaces the placeholder in `48 8b 05 00 00 00 00` with the final displacement. In a relocatable file, `disp32` is computed with this relocation formula:

`disp32 = S (symbol address = section address [+ symbol offset]) + A (addend) - P (placeholder address)`

Where:

- `S` is the address of the symbol (in this case, the address of section `.data`).
- `A` is the addend (in this case, `-4`), it is an adjustment to calculate the correct offset from the next instruction to the symbol.
- `P` is the placeholder address (in this case, the address of the placeholder in the instruction `MOV`, which is `0x3` in the `.text` section).

The formula `disp32 = S + A - P` is equivalent to `disp32 = S - P + A`. For `R_X86_64_PC32`, `A = -4` adjusts from the placeholder location (`P`) to the next instruction address (`P + 4`).

> The addend here is `-4` because the relocation place (`0x03`) is 4 bytes before the next instruction (`0x07`). More generally, when relocation references a section symbol (such as `.data`) rather than a specific symbol (such as `num`), the addend can include both the `-4` RIP adjustment and any symbol offset within that section.

### Calling

In the disassembly, `call 0 <inc>` is the `CALL rel32` instruction. The bytes `ef ff ff ff` are the little-endian encoding of `-17`, which is the offset from the next instruction to `inc`.

The target address is calculated as `target = next_instruction_rip + rel32`. Here, `next_instruction_rip = 0x11`, so `0x11 + (-17) = 0x0`, which is the start of `inc`.

## Executable file

### File header (ET_EXEC)

```sh
readelf -h mini.elf
```

Output:

```text
ELF Header:
  Magic:   7f 45 4c 46 02 01 01 00 00 00 00 00 00 00 00 00
  Class:                             ELF64
  Data:                              2's complement, little endian
  Version:                           1 (current)
  OS/ABI:                            UNIX - System V
  ABI Version:                       0
  Type:                              EXEC (Executable file)
  Machine:                           Advanced Micro Devices X86-64
  Version:                           0x1
  Entry point address:               0x40100c
  Start of program headers:          64 (bytes into file)
  Start of section headers:          8480 (bytes into file)
  Flags:                             0x0
  Size of this header:               64 (bytes)
  Size of program headers:           56 (bytes)
  Number of program headers:         3
  Size of section headers:           64 (bytes)
  Number of section headers:         6
  Section header string table index: 5
```

### Section headers

```sh
readelf -S mini.elf
```

Output:

```text
There are 6 section headers, starting at offset 0x2120:

Section Headers:
  [Nr] Name              Type             Address           Offset
       Size              EntSize          Flags  Link  Info  Align
  [ 0]                   NULL             0000000000000000  00000000
       0000000000000000  0000000000000000           0     0     0
  [ 1] .text             PROGBITS         0000000000401000  00001000
       000000000000001b  0000000000000000  AX       0     0     16
  [ 2] .data             PROGBITS         0000000000402000  00002000
       0000000000000008  0000000000000000  WA       0     0     4
  [ 3] .symtab           SYMTAB           0000000000000000  00002008
       00000000000000c0  0000000000000018           4     4     8
  [ 4] .strtab           STRTAB           0000000000000000  000020c8
       000000000000002c  0000000000000000           0     0     1
  [ 5] .shstrtab         STRTAB           0000000000000000  000020f4
       0000000000000027  0000000000000000           0     0     1
```

### Program headers

```sh
readelf -l mini.elf
```

Output:

```text
Elf file type is EXEC (Executable file)
Entry point 0x40100c
There are 3 program headers, starting at offset 64

Program Headers:
  Type           Offset             VirtAddr           PhysAddr
                 FileSiz            MemSiz              Flags  Align
  LOAD           0x0000000000000000 0x0000000000400000 0x0000000000400000
                 0x00000000000000e8 0x00000000000000e8  R      0x1000
  LOAD           0x0000000000001000 0x0000000000401000 0x0000000000401000
                 0x000000000000001b 0x000000000000001b  R E    0x1000
  LOAD           0x0000000000002000 0x0000000000402000 0x0000000000402000
                 0x0000000000000008 0x0000000000000008  RW     0x1000

 Section to Segment mapping:
  Segment Sections...
   00
   01     .text
   02     .data
```

The first segment contains the ELF header and program headers, the size is `0xe8` bytes, which is calculated by:

- ELF header size: `0x40` bytes (64 bytes)
- Program header size: `0x38` bytes (56 bytes) * 3 = `0xa8` bytes
- Total: `0x40 + 0xa8 = 0xe8` bytes

The `Section to Segment mapping` is determined by the `p_offset` and `p_filesz` fields of the program headers. For example, the second segment (which is executable) has `p_offset = 0x1000` and `p_filesz = 0x1b`, which means it includes the `.text` section that starts at offset `0x1000` and has size `0x1b`.

The `PhysAddr` field is usually the same as `VirtAddr` for executable files, only for some special cases (like baremetal programs, EFI applications) they might differ.

### Symbols (ET_EXEC)

```sh
readelf -s mini.elf
```

Output:

```text
Symbol table '.symtab' contains 8 entries:
   Num:    Value          Size Type    Bind   Vis      Ndx Name
     0: 0000000000000000     0 NOTYPE  LOCAL  DEFAULT  UND
     1: 0000000000000000     0 FILE    LOCAL  DEFAULT  ABS mini.asm
     2: 0000000000402000     0 NOTYPE  LOCAL  DEFAULT    2 num
     3: 0000000000401000     0 NOTYPE  LOCAL  DEFAULT    1 inc
     4: 000000000040100c     0 NOTYPE  GLOBAL DEFAULT    1 _start
     5: 0000000000402008     0 NOTYPE  GLOBAL DEFAULT    2 __bss_start
     6: 0000000000402008     0 NOTYPE  GLOBAL DEFAULT    2 _edata
     7: 0000000000402008     0 NOTYPE  GLOBAL DEFAULT    2 _end
```

### Relocations (ET_EXEC)

```sh
readelf -r mini.elf
```

Output:

```text
There are no relocations in this file.
```
