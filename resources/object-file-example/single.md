# Single Assembly Source File Example

## Source code

single.asm:

```asm
;; A simple NASM assembly program that defines a global variable,
;; a function to increment it, and a main entry point that calls the function and exits with the result.

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

    ; syscall call `exit(status)`
    ; syscall number: 60

    mov rdi, rax        ;; move the result of inc (the incremented value) into rdi, which is the first argument to the syscall
    mov rax, 60         ;; syscall number for exit
    syscall
```

## Assemble, link, and run

```sh
nasm -f elf64 -o single.o single.asm
ld -o single.elf single.o
./single.elf
echo $? # output: 42
```

## Object file sections

```sh
readelf -S single.o
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

Where the `Nr` column is the section index, for example:

| Nr | Section |
|----|---------|
| 1  | .data   |
| 2  | .text   |

General sections:

| Section Name | Section Type | Description                              |
|--------------|--------------|------------------------------------------|
| .data        | PROGBITS     | Initialized data                         |
| .text        | PROGBITS     | Executable code                          |
| .bss         | NOBITS       | Uninitialized data                       |
| .rodata      | PROGBITS     | Read-only data                           |
| .symtab      | SYMTAB       | Symbol table                             |
| .strtab      | STRTAB       | String table (used for symbol names)     |
| .rela.text   | RELA         | Relocation entries for the .text section |

The section types:

| Section Type | Description                                                                |
|--------------|----------------------------------------------------------------------------|
| PROGBITS     | A section that contains data or code.                                      |
| NOBITS       | A section that does not occupy space in the file but has a size in memory. |
| STRTAB       | A section that contains null-terminated strings.                           |
| SYMTAB       | A section that contains symbol table entries.                              |
| RELA         | A section that contains relocation entries with addends.                   |

## Symbols

```sh
readelf -s single.o
```

Lists the symbols with more details, including their size and section index.

Output:

```text
Symbol table '.symtab' contains 7 entries:
   Num:    Value          Size Type    Bind   Vis      Ndx Name
     0: 0000000000000000     0 NOTYPE  LOCAL  DEFAULT  UND
     1: 0000000000000000     0 FILE    LOCAL  DEFAULT  ABS single.asm
     2: 0000000000000000     0 SECTION LOCAL  DEFAULT    1 .data
     3: 0000000000000000     0 SECTION LOCAL  DEFAULT    2 .text
     4: 0000000000000000     0 NOTYPE  LOCAL  DEFAULT    1 num
     5: 0000000000000000     0 NOTYPE  LOCAL  DEFAULT    2 inc
     6: 000000000000000c     0 NOTYPE  GLOBAL DEFAULT    2 _start
```

Where `Value` is the offset of the symbol within **its section**
(not the final address, which is determined by the linker),
and `Ndx` is the index of the section it belongs to (1 for `.data`, 2 for `.text`).

## Symbols via `nm`

```sh
nm single.o
```

Lists the symbols in the object file.

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

### 'U' undefined symbol

'U' means undefined symbol (imported symbol), which is a symbol that is referenced in the code but not defined in the object file. The linker will need to resolve this symbol by finding it in another object file or library during the linking process.

```asm
extern puts

section .text
global _start

_start:
    call puts
```

Will output:

```text
                 U puts
0000000000000000 T _start
```

and:

```text
RELOCATION RECORDS FOR [.text]:
OFFSET           TYPE              VALUE
0000000000000001 R_X86_64_PC32     puts-0x0000000000000004
```

### 'W' weak symbol

'W' means weak symbol, which is a symbol that has a default definition but can be overridden by another definition with the same name. If there are multiple definitions of a weak symbol, the linker will choose one of them (usually the first one it encounters) and ignore the others.

Strong vs weak:

| Combination       | Result                              |
|-------------------|-------------------------------------|
| strong + weak     | strong                              |
| weak + weak       | any of them (usually the first one) |
| strong + strong   | linker error                        |

## Disassembly code

```sh
objdump -M intel -d -r single.o
```

The disassembly of the resulting object file `single.o` would look like this:

```text
single.o:     file format elf64-x86-64

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

## Disassembly data

```sh
objdump -s -j .data single.o
```

Output:

```text
single.o:     file format elf64-x86-64

Contents of section .data:
 0000 29000000 00000000                    ).......
```

## Relocations

Where `<inc+0x7>` is a temporary relocation (placeholder) that will be resolved by the linker, the `0x7` is just because the `effective_address = next_instruction_rip + disp32` and the `next_instruction_rip` is `0x7` at the time of encoding the instruction.

Check out the relocation section:

```sh
readelf -r single.o
```

Output:

```text
Relocation section '.rela.text' at offset 0x340 contains 1 entry:
  Offset          Info           Type           Sym. Value    Sym. Name + Addend
000000000003  000200000002 R_X86_64_PC32     0000000000000000 .data - 4
```

Which means that at offset `0x3` in the `.text` section, there is a relocation entry of type `R_X86_64_PC32`:

Fields:

- `Offset` is the offset within the section where the relocation applies (in this case, `0x3` in the `.text` section).
- `Info` encodes the symbol index and the type of relocation (in this case, `000200000002` means symbol index `2` and type `R_X86_64_PC32`), its value is `sym_index << 32 | type`.
- `Type` is the type of relocation (in this case, `R_X86_64_PC32`)
- `Sym. Name` is the symbol that the relocation references (in this case, `.data`, which is the section containing `num`).
- `+ Addend` indicates that the linker should add `-4` to the symbol's address (which is the section `.data` in this case) when applying the relocation.

Note that when CPU executes the instruction `mov rax, [rip + disp32]`, the effective address is calculated as `effective_address = next_instruction_rip + disp32`, thus `disp32 = source_address (address of num) - next_instruction_address`.

We should subsitute the placeholder in the "48 8b 05 00 00 00 00" instruction with the actual offset.

However, in the relocation table, it only provides the offset of the placeholder (`0x03`) and the source section name (`.data`), it does not provider the source symbol and the `next_instruction_rip`, so we need to calculate it as:

`target = S (source address = secion + offset ) + A (addend) - P (placeholder address)`

Where:

- `S` is the address of the symbol (in this case, the address of `num` in the `.data` section, which is `0x0` since it's the first symbol in the `.data` section).
- `A` is the addend (in this case, `-4`), it is an adjustment to calculate the correct offset from the next instruction to the symbol.
- `P` is the place address (in this case, the address of the instruction that contains the placeholder, which is `0x3` in the `.text` section).

The formular `target = S + A - P` is equivalent to `target = S - P + A`, where `- P + A` is the `next_instruction_rip`, becuase the `P` is less than the `next_instruction_rip`, so we need to subtract an additional value (which is `4`).

> 'Addend' is the distance from the placeholder to the next instruction, so the addend is `-4` because the placeholder (`0x03`) is 4 bytes before the next instruction (`0x07`). If the symbol is the section (`.data`) instead of the actual symbol (`num`), then the addend is needed to adjust by adding the actual symbol offset. For example, if another symbol is at offset `0x8` in the `.data` section, then the addend would be `-4 + 0x8 = 4`.

## Calling

Where `call 0 <inc>` is `CALL rel32`, and `ef ff ff ff` is the little-endian encoding of `-17` (the offset from the next instruction to the start of `inc`).

The target address is `target = next_rip + rel32`, and since `next_rip` is `0x11` at the time of encoding, the target address is `0x11 - 17 = 0x0`, which is the start of `inc`.
