# Program "simple"

<!-- @import "[TOC]" {cmd="toc" depthFrom=2 depthTo=4 orderedList=false} -->

<!-- code_chunk_output -->

- [Source code](#source-code)
  - [simple-lib.asm](#simple-libasm)
  - [simple-app.asm](#simple-appasm)
- [Assemble, link, and run](#assemble-link-and-run)
- [Symbols, Disassembly, and relocations](#symbols-disassembly-and-relocations)
  - [simple-lib.o](#simple-libo)
    - [Sections 0](#sections-0)
    - [Symbols 0](#symbols-0)
    - [Symbol type 0](#symbol-type-0)
    - [Disassembly 0](#disassembly-0)
    - [Relocation 0](#relocation-0)
  - [simple-app.o](#simple-appo)
    - [Sections 1](#sections-1)
    - [Symbols 1](#symbols-1)
    - [Symbol type 1](#symbol-type-1)
    - [Disassembly 1](#disassembly-1)
    - [Relocation 1](#relocation-1)
  - [simple.elf](#simpleelf)
    - [Sections EXEC](#sections-exec)
    - [Segments (Program Headers)](#segments-program-headers)
    - [Symbols EXEC](#symbols-exec)
    - [Symbol type EXEC](#symbol-type-exec)
    - [Disassembly EXEC](#disassembly-exec)
    - [Relocation EXEC](#relocation-exec)

<!-- /code_chunk_output -->

## Source code

### simple-lib.asm

```asm
global msg
global len
global left
global right
global foo
global bar
global inc
global dec

;; data types:
;; - dq: data quadword (8 bytes)
;; - dd: data doubleword (4 bytes)
;; - dw: data word (2 bytes)
;; - db: data byte (1 byte)
;;
;; uninitialized global variables (BSS section) can be defined with:
;; - `resq`: reserve quadword (8 bytes)
;; - `resd`: reserve doubleword (4 bytes)
;; - `resw`: reserve word (2 bytes)
;; - `resb`: reserve byte (1 byte)

section .rodata
    msg db "Hello", 10, 0   ;; read-only global variable with a string value
    len dq 6                ;; read-only global variable with value 6

section .data
    left dq 11              ;; read-write global variable with initial value 11
    right dq 17             ;; read-write global variable with initial value 17

section .bss
    foo resd 1              ;; uninitialized global variable (4 bytes)
    bar resd 1              ;; uninitialized global variable (4 bytes)

section .text

;; fn inc() -> int64_t
inc:
    mov rax, [rel left]     ;; read the value of left into rax
    add rax, 1              ;; increment rax by 1
    mov [rel foo], eax      ;; store the incremented value in foo (lower 32 bits of rax)
    ret                     ;; return the incremented value in rax

;; fn dec() -> int64_t
dec:
    mov rax, [rel right]    ;; read the value of right into rax
    sub rax, 1              ;; decrement rax by 1
    mov [rel bar], eax      ;; store the decremented value in bar (lower 32 bits of rax)
    ret                     ;; return the decremented value in rax
```

### simple-app.asm

```asm
global _start

extern msg
extern len
extern left
extern right
extern foo
extern bar
extern inc
extern dec

section .text

;; fn _start() -> void
_start:
    ;; read the values of global variables for testing purposes, but we will not use them
    mov rbx, [rel left]
    mov rbx, [rel right]
    mov ebx, [rel foo]
    mov ebx, [rel bar]

    ;; print msg string using syscall `write(fd, buf, count)`
    ;; syscall number: 1

    mov rdi, 1          ;; file descriptor for stdout
    mov rsi, msg        ;; pointer to the string to write
    mov rdx, [rel len]  ;; number of bytes to write (length of "Hello\0")
    mov rax, 1          ;; syscall number for write
    syscall

    ;; calculate inc() + dec() and exit with the result as status code

    xor rbx, rbx            ;; set rbx to 0

    call inc                ;; call inc(), result is in rax, value is 12
    add rax, rbx            ;; sum original num and inc() result, value is 12

    mov rbx, rax

    call dec                ;; call dec(), result is in rax, value is 16
    add rax, rbx            ;; sum previous result and dec() result, value is 28

    ;; exit program using syscall `exit(status)`
    ;; syscall number: 60

    mov rdi, rax        ;; move summed result into rdi (exit status)
    mov rax, 60         ;; syscall number for exit
    syscall
```

## Assemble, link, and run

```sh
nasm -f elf64 -o simple-lib.o simple-lib.asm
nasm -f elf64 -o simple-app.o simple-app.asm
ld -o simple.elf simple-lib.o simple-app.o
./simple.elf
echo $? # output: Hello\n28
```

## Symbols, Disassembly, and relocations

### simple-lib.o

#### Sections 0

```sh
readelf -S simple-lib.o
```

Output:

```text
There are 9 section headers, starting at offset 0x40:

Section Headers:
  [Nr] Name              Type             Address           Offset
       Size              EntSize          Flags  Link  Info  Align
  [ 0]                   NULL             0000000000000000  00000000
       0000000000000000  0000000000000000           0     0     0
  [ 1] .rodata           PROGBITS         0000000000000000  00000280
       000000000000000f  0000000000000000   A       0     0     4
  [ 2] .data             PROGBITS         0000000000000000  00000290
       0000000000000010  0000000000000000  WA       0     0     4
  [ 3] .bss              NOBITS           0000000000000000  000002a0
       0000000000000008  0000000000000000  WA       0     0     4
  [ 4] .text             PROGBITS         0000000000000000  000002a0
       0000000000000036  0000000000000000  AX       0     0     16
  [ 5] .shstrtab         STRTAB           0000000000000000  000002e0
       000000000000003f  0000000000000000           0     0     1
  [ 6] .symtab           SYMTAB           0000000000000000  00000320
       0000000000000168  0000000000000018           7     7     8
  [ 7] .strtab           STRTAB           0000000000000000  00000490
       0000000000000033  0000000000000000           0     0     1
  [ 8] .rela.text        RELA             0000000000000000  000004d0
       0000000000000090  0000000000000018           6     4     8
```

Note that the `.text` section's file offset is 0x2a0, which is the same as the `.bss` section's file offset. This is because the `.bss` section occupies no space in the file (it contains uninitialized data), so the next section's file offset immediately follows the previous one, despite being different sections in memory.

Be careful when linking `NOBITS` sections like `.bss`. These sections have no actual data in the object file, even though their section header specifies a size.

#### Symbols 0

```sh
readelf -s simple-lib.o
```

Output:

```text
Symbol table '.symtab' contains 15 entries:
   Num:    Value          Size Type    Bind   Vis      Ndx Name
     0: 0000000000000000     0 NOTYPE  LOCAL  DEFAULT  UND
     1: 0000000000000000     0 FILE    LOCAL  DEFAULT  ABS simple-lib.asm
     2: 0000000000000000     0 SECTION LOCAL  DEFAULT    1 .rodata
     3: 0000000000000000     0 SECTION LOCAL  DEFAULT    2 .data
     4: 0000000000000000     0 SECTION LOCAL  DEFAULT    3 .bss
     5: 0000000000000000     0 SECTION LOCAL  DEFAULT    4 .text
     6: 0000000000000024     0 NOTYPE  LOCAL  DEFAULT    4 test
     7: 0000000000000000     0 NOTYPE  GLOBAL DEFAULT    1 msg
     8: 0000000000000007     0 NOTYPE  GLOBAL DEFAULT    1 len
     9: 0000000000000000     0 NOTYPE  GLOBAL DEFAULT    2 left
    10: 0000000000000008     0 NOTYPE  GLOBAL DEFAULT    2 right
    11: 0000000000000000     0 NOTYPE  GLOBAL DEFAULT    3 foo
    12: 0000000000000004     0 NOTYPE  GLOBAL DEFAULT    3 bar
    13: 0000000000000000     0 NOTYPE  GLOBAL DEFAULT    4 inc
    14: 0000000000000012     0 NOTYPE  GLOBAL DEFAULT    4 dec
```

The `test` symbol is a private function defined in `simple-lib.asm`, so it is a local symbol.

#### Symbol type 0

```sh
nm simple-lib.o
```

Output:

```text
0000000000000004 B bar
0000000000000012 T dec
0000000000000000 B foo
0000000000000000 T inc
0000000000000000 D left
0000000000000007 R len
0000000000000000 R msg
0000000000000008 D right
0000000000000024 t test
```

#### Disassembly 0

```sh
objdump -M intel -sdr simple-lib.o
```

Output:

```text
simple-lib.o:     file format elf64-x86-64

Contents of section .rodata:
 0000 48656c6c 6f0a0006 00000000 000000    Hello..........
Contents of section .data:
 0000 0b000000 00000000 11000000 00000000  ................
Contents of section .text:
 0000 488b0500 00000048 83c00189 05000000  H......H........
 0010 00c3488b 05000000 004883e8 01890500  ..H......H......
 0020 000000c3 488d3d00 00000048 8b350000  ....H.=....H.5..
 0030 00004831 c0c3                        ..H1..

Disassembly of section .text:

0000000000000000 <inc>:
   0:   48 8b 05 00 00 00 00    mov    rax,QWORD PTR [rip+0x0]        # 7 <inc+0x7>
                        3: R_X86_64_PC32        .data-0x4
   7:   48 83 c0 01             add    rax,0x1
   b:   89 05 00 00 00 00       mov    DWORD PTR [rip+0x0],eax        # 11 <inc+0x11>
                        d: R_X86_64_PC32        .bss-0x4
  11:   c3                      ret

0000000000000012 <dec>:
  12:   48 8b 05 00 00 00 00    mov    rax,QWORD PTR [rip+0x0]        # 19 <dec+0x7>
                        15: R_X86_64_PC32       .data+0x4
  19:   48 83 e8 01             sub    rax,0x1
  1d:   89 05 00 00 00 00       mov    DWORD PTR [rip+0x0],eax        # 23 <dec+0x11>
                        1f: R_X86_64_PC32       .bss
  23:   c3                      ret

0000000000000024 <test>:
  24:   48 8d 3d 00 00 00 00    lea    rdi,[rip+0x0]        # 2b <test+0x7>
                        27: R_X86_64_PC32       .rodata-0x4
  2b:   48 8b 35 00 00 00 00    mov    rsi,QWORD PTR [rip+0x0]        # 32 <test+0xe>
                        2e: R_X86_64_PC32       .rodata+0x3
  32:   48 31 c0                xor    rax,rax
  35:   c3                      ret
```

Note that the section `.bss` is not present in the file, so there are no contents to display for it.

#### Relocation 0

```sh
readelf -r simple-lib.o
```

Output:

```text
Relocation section '.rela.text' at offset 0x4d0 contains 6 entries:
  Offset          Info           Type           Sym. Value    Sym. Name + Addend
000000000003  000300000002 R_X86_64_PC32     0000000000000000 .data - 4
00000000000d  000400000002 R_X86_64_PC32     0000000000000000 .bss - 4
000000000015  000300000002 R_X86_64_PC32     0000000000000000 .data + 4
00000000001f  000400000002 R_X86_64_PC32     0000000000000000 .bss + 0
000000000027  000200000002 R_X86_64_PC32     0000000000000000 .rodata - 4
00000000002e  000200000002 R_X86_64_PC32     0000000000000000 .rodata + 3
```

### simple-app.o

#### Sections 1

```sh
readelf -S simple-app.o
```

Output:

```text
Section Headers:
  [Nr] Name              Type             Address           Offset
       Size              EntSize          Flags  Link  Info  Align
  [ 0]                   NULL             0000000000000000  00000000
       0000000000000000  0000000000000000           0     0     0
  [ 1] .text             PROGBITS         0000000000000000  000001c0
       0000000000000054  0000000000000000  AX       0     0     16
  [ 2] .shstrtab         STRTAB           0000000000000000  00000220
       000000000000002c  0000000000000000           0     0     1
  [ 3] .symtab           SYMTAB           0000000000000000  00000250
       0000000000000120  0000000000000018           4     3     8
  [ 4] .strtab           STRTAB           0000000000000000  00000370
       0000000000000035  0000000000000000           0     0     1
  [ 5] .rela.text        RELA             0000000000000000  000003b0
       00000000000000c0  0000000000000018           3     1     8
```

There is no `.data` or `.bss` section in `simple-app.o` because `simple-app.asm` does not define any global variables, all data are imported from `simple-lib.o`.

#### Symbols 1

```sh
readelf -s simple-app.o
```

Output:

```text
Symbol table '.symtab' contains 12 entries:
   Num:    Value          Size Type    Bind   Vis      Ndx Name
     0: 0000000000000000     0 NOTYPE  LOCAL  DEFAULT  UND
     1: 0000000000000000     0 FILE    LOCAL  DEFAULT  ABS simple-app.asm
     2: 0000000000000000     0 SECTION LOCAL  DEFAULT    1 .text
     3: 0000000000000000     0 NOTYPE  GLOBAL DEFAULT  UND msg
     4: 0000000000000000     0 NOTYPE  GLOBAL DEFAULT  UND len
     5: 0000000000000000     0 NOTYPE  GLOBAL DEFAULT  UND left
     6: 0000000000000000     0 NOTYPE  GLOBAL DEFAULT  UND right
     7: 0000000000000000     0 NOTYPE  GLOBAL DEFAULT  UND foo
     8: 0000000000000000     0 NOTYPE  GLOBAL DEFAULT  UND bar
     9: 0000000000000000     0 NOTYPE  GLOBAL DEFAULT  UND inc
    10: 0000000000000000     0 NOTYPE  GLOBAL DEFAULT  UND dec
    11: 0000000000000000     0 NOTYPE  GLOBAL DEFAULT    1 _start
```

All imported symbols (`msg`, `len`, `left`, `right`, `foo`, `bar`, `inc`, and `dec`) are marked as undefined (UND).

#### Symbol type 1

```sh
nm simple-app.o
```

Output:

```text
                 U bar
                 U dec
                 U foo
                 U inc
                 U left
                 U len
                 U msg
                 U right
0000000000000000 T _start
```

#### Disassembly 1

```sh
objdump -M intel -sdr simple-app.o
```

Output:

```text
simple-app.o:     file format elf64-x86-64

Contents of section .text:
 0000 488b1d00 00000048 8b1d0000 00008b1d  H......H........
 0010 00000000 8b1d0000 0000bf01 00000048  ...............H
 0020 8d350000 0000488b 15000000 00b80100  .5....H.........
 0030 00000f05 4831dbe8 00000000 4801d848  ....H1......H..H
 0040 89c3e800 00000048 01d84889 c7b83c00  .......H..H...<.
 0050 00000f05                             ....

Disassembly of section .text:

0000000000000000 <_start>:
   0:   48 8b 1d 00 00 00 00    mov    rbx,QWORD PTR [rip+0x0]        # 7 <_start+0x7>
                        3: R_X86_64_PC32        left-0x4
   7:   48 8b 1d 00 00 00 00    mov    rbx,QWORD PTR [rip+0x0]        # e <_start+0xe>
                        a: R_X86_64_PC32        right-0x4
   e:   8b 1d 00 00 00 00       mov    ebx,DWORD PTR [rip+0x0]        # 14 <_start+0x14>
                        10: R_X86_64_PC32       foo-0x4
  14:   8b 1d 00 00 00 00       mov    ebx,DWORD PTR [rip+0x0]        # 1a <_start+0x1a>
                        16: R_X86_64_PC32       bar-0x4
  1a:   bf 01 00 00 00          mov    edi,0x1
  1f:   48 8d 35 00 00 00 00    lea    rsi,[rip+0x0]        # 26 <_start+0x26>
                        22: R_X86_64_PC32       msg-0x4
  26:   48 8b 15 00 00 00 00    mov    rdx,QWORD PTR [rip+0x0]        # 2d <_start+0x2d>
                        29: R_X86_64_PC32       len-0x4
  2d:   b8 01 00 00 00          mov    eax,0x1
  32:   0f 05                   syscall
  34:   48 31 db                xor    rbx,rbx
  37:   e8 00 00 00 00          call   3c <_start+0x3c>
                        38: R_X86_64_PC32       inc-0x4
  3c:   48 01 d8                add    rax,rbx
  3f:   48 89 c3                mov    rbx,rax
  42:   e8 00 00 00 00          call   47 <_start+0x47>
                        43: R_X86_64_PC32       dec-0x4
  47:   48 01 d8                add    rax,rbx
  4a:   48 89 c7                mov    rdi,rax
  4d:   b8 3c 00 00 00          mov    eax,0x3c
  52:   0f 05                   syscall
```

#### Relocation 1

```sh
readelf -r simple-app.o
```

Output:

```text
Relocation section '.rela.text' at offset 0x3b0 contains 8 entries:
  Offset          Info           Type           Sym. Value    Sym. Name + Addend
000000000003  000500000002 R_X86_64_PC32     0000000000000000 left - 4
00000000000a  000600000002 R_X86_64_PC32     0000000000000000 right - 4
000000000010  000700000002 R_X86_64_PC32     0000000000000000 foo - 4
000000000016  000800000002 R_X86_64_PC32     0000000000000000 bar - 4
000000000022  000300000002 R_X86_64_PC32     0000000000000000 msg - 4
000000000029  000400000002 R_X86_64_PC32     0000000000000000 len - 4
000000000038  000900000002 R_X86_64_PC32     0000000000000000 inc - 4
000000000043  000a00000002 R_X86_64_PC32     0000000000000000 dec - 4
```

Comparing the relocation entries in `simple-app.o` with those in `simple-lib.o`, we observe that relocations in `simple-app.o` reference symbols instead of sections (with offsets).

### simple.elf

#### Sections EXEC

```sh
readelf -S simple.elf
```

Output:

```text
There are 8 section headers, starting at offset 0x2230:

Section Headers:
  [Nr] Name              Type             Address           Offset
       Size              EntSize          Flags  Link  Info  Align
  [ 0]                   NULL             0000000000000000  00000000
       0000000000000000  0000000000000000           0     0     0
  [ 1] .text             PROGBITS         0000000000401000  00001000
       0000000000000094  0000000000000000  AX       0     0     16
  [ 2] .rodata           PROGBITS         0000000000402000  00002000
       000000000000000f  0000000000000000   A       0     0     4
  [ 3] .data             PROGBITS         0000000000403010  00002010
       0000000000000010  0000000000000000  WA       0     0     4
  [ 4] .bss              NOBITS           0000000000403020  00002020
       0000000000000008  0000000000000000  WA       0     0     4
  [ 5] .symtab           SYMTAB           0000000000000000  00002020
       0000000000000180  0000000000000018           6     4     8
  [ 6] .strtab           STRTAB           0000000000000000  000021a0
       0000000000000055  0000000000000000           0     0     1
  [ 7] .shstrtab         STRTAB           0000000000000000  000021f5
       0000000000000034  0000000000000000           0     0     1
```

#### Segments (Program Headers)

```sh
readelf -l simple.elf
```

Output:

```text
Elf file type is EXEC (Executable file)
Entry point 0x401040
There are 4 program headers, starting at offset 64

Program Headers:
  Type           Offset             VirtAddr           PhysAddr
                 FileSiz            MemSiz              Flags  Align
  LOAD           0x0000000000000000 0x0000000000400000 0x0000000000400000
                 0x0000000000000120 0x0000000000000120  R      0x1000
  LOAD           0x0000000000001000 0x0000000000401000 0x0000000000401000
                 0x0000000000000094 0x0000000000000094  R E    0x1000
  LOAD           0x0000000000002000 0x0000000000402000 0x0000000000402000
                 0x000000000000000f 0x000000000000000f  R      0x1000
  LOAD           0x0000000000002010 0x0000000000403010 0x0000000000403010
                 0x0000000000000010 0x0000000000000018  RW     0x1000

 Section to Segment mapping:
  Segment Sections...
   00
   01     .text
   02     .rodata
   03     .data .bss
```

Note that segment `03` contains both the `.data` and `.bss` sections. Its file size is 0x10 (the size of `.data`), but its memory size is 0x18 (the combined size of `.data` and `.bss`).

#### Symbols EXEC

```sh
readelf -s simple.elf
```

Output:

```text
Symbol table '.symtab' contains 16 entries:
   Num:    Value          Size Type    Bind   Vis      Ndx Name
     0: 0000000000000000     0 NOTYPE  LOCAL  DEFAULT  UND
     1: 0000000000000000     0 FILE    LOCAL  DEFAULT  ABS simple-lib.asm
     2: 0000000000401024     0 NOTYPE  LOCAL  DEFAULT    1 test
     3: 0000000000000000     0 FILE    LOCAL  DEFAULT  ABS simple-app.asm
     4: 0000000000403018     0 NOTYPE  GLOBAL DEFAULT    3 right
     5: 0000000000402000     0 NOTYPE  GLOBAL DEFAULT    2 msg
     6: 0000000000403010     0 NOTYPE  GLOBAL DEFAULT    3 left
     7: 0000000000401040     0 NOTYPE  GLOBAL DEFAULT    1 _start
     8: 0000000000401012     0 NOTYPE  GLOBAL DEFAULT    1 dec
     9: 0000000000403020     0 NOTYPE  GLOBAL DEFAULT    4 __bss_start
    10: 0000000000401000     0 NOTYPE  GLOBAL DEFAULT    1 inc
    11: 0000000000403020     0 NOTYPE  GLOBAL DEFAULT    4 foo
    12: 0000000000403020     0 NOTYPE  GLOBAL DEFAULT    3 _edata
    13: 0000000000403028     0 NOTYPE  GLOBAL DEFAULT    4 _end
    14: 0000000000403024     0 NOTYPE  GLOBAL DEFAULT    4 bar
    15: 0000000000402007     0 NOTYPE  GLOBAL DEFAULT    2 len
```

The symbol `test` is a local symbol defined in `simple-lib.o` that does not appear in the `simple.elf` symbol table.

The linker generates several special symbols related to the BSS section: `__bss_start`, `_edata`, and `_end`. These are typically used for BSS section initialization (all data in the BSS section is zeroed when the program starts).

A typical executable's memory layout is as follows:

- `.text`
- `.rodata`
- `.data`
- `_edata`
- `.bss`
- `__bss_start`
- `_end`

Linkers often optimize memory layout by merging the `.data` and `.bss` sections, resulting in `_edata == __bss_start`.

#### Symbol type EXEC

```sh
nm simple.elf
```

Output:

```text
0000000000403024 B bar
0000000000403020 B __bss_start
0000000000401012 T dec
0000000000403020 D _edata
0000000000403028 B _end
0000000000403020 B foo
0000000000401000 T inc
0000000000403010 D left
0000000000402007 R len
0000000000402000 R msg
0000000000403018 D right
0000000000401040 T _start
0000000000401024 t test
```

#### Disassembly EXEC

```sh
objdump -M intel -sdr simple.elf
```

Output:

```text
simple.elf:     file format elf64-x86-64

Contents of section .text:
 401000 488b0509 20000048 83c00189 050f2000  H... ..H...... .
 401010 00c3488b 05ff1f00 004883e8 01890501  ..H......H......
 401020 200000c3 488d3dd5 0f000048 8b35d50f   ...H.=....H.5..
 401030 00004831 c0c3662e 0f1f8400 00000000  ..H1..f.........
 401040 488b1dc9 1f000048 8b1dca1f 00008b1d  H......H........
 401050 cc1f0000 8b1dca1f 0000bf01 00000048  ...............H
 401060 8d359a0f 0000488b 159a0f00 00b80100  .5....H.........
 401070 00000f05 4831dbe8 84ffffff 4801d848  ....H1......H..H
 401080 89c3e88b ffffff48 01d84889 c7b83c00  .......H..H...<.
 401090 00000f05                             ....
Contents of section .rodata:
 402000 48656c6c 6f0a0006 00000000 000000    Hello..........
Contents of section .data:
 403010 0b000000 00000000 11000000 00000000  ................

Disassembly of section .text:

0000000000401000 <inc>:
  401000:       48 8b 05 09 20 00 00    mov    rax,QWORD PTR [rip+0x2009]        # 403010 <left>
  401007:       48 83 c0 01             add    rax,0x1
  40100b:       89 05 0f 20 00 00       mov    DWORD PTR [rip+0x200f],eax        # 403020 <__bss_start>
  401011:       c3                      ret

0000000000401012 <dec>:
  401012:       48 8b 05 ff 1f 00 00    mov    rax,QWORD PTR [rip+0x1fff]        # 403018 <right>
  401019:       48 83 e8 01             sub    rax,0x1
  40101d:       89 05 01 20 00 00       mov    DWORD PTR [rip+0x2001],eax        # 403024 <bar>
  401023:       c3                      ret

0000000000401024 <test>:
  401024:       48 8d 3d d5 0f 00 00    lea    rdi,[rip+0xfd5]        # 402000 <msg>
  40102b:       48 8b 35 d5 0f 00 00    mov    rsi,QWORD PTR [rip+0xfd5]        # 402007 <len>
  401032:       48 31 c0                xor    rax,rax
  401035:       c3                      ret
  401036:       66 2e 0f 1f 84 00 00    cs nop WORD PTR [rax+rax*1+0x0]
  40103d:       00 00 00

0000000000401040 <_start>:
  401040:       48 8b 1d c9 1f 00 00    mov    rbx,QWORD PTR [rip+0x1fc9]        # 403010 <left>
  401047:       48 8b 1d ca 1f 00 00    mov    rbx,QWORD PTR [rip+0x1fca]        # 403018 <right>
  40104e:       8b 1d cc 1f 00 00       mov    ebx,DWORD PTR [rip+0x1fcc]        # 403020 <__bss_start>
  401054:       8b 1d ca 1f 00 00       mov    ebx,DWORD PTR [rip+0x1fca]        # 403024 <bar>
  40105a:       bf 01 00 00 00          mov    edi,0x1
  40105f:       48 8d 35 9a 0f 00 00    lea    rsi,[rip+0xf9a]        # 402000 <msg>
  401066:       48 8b 15 9a 0f 00 00    mov    rdx,QWORD PTR [rip+0xf9a]        # 402007 <len>
  40106d:       b8 01 00 00 00          mov    eax,0x1
  401072:       0f 05                   syscall
  401074:       48 31 db                xor    rbx,rbx
  401077:       e8 84 ff ff ff          call   401000 <inc>
  40107c:       48 01 d8                add    rax,rbx
  40107f:       48 89 c3                mov    rbx,rax
  401082:       e8 8b ff ff ff          call   401012 <dec>
  401087:       48 01 d8                add    rax,rbx
  40108a:       48 89 c7                mov    rdi,rax
  40108d:       b8 3c 00 00 00          mov    eax,0x3c
  401092:       0f 05                   syscall
```

#### Relocation EXEC

```sh
readelf -r simple.elf
```

Output:

```text
There are no relocations in this file.
```
