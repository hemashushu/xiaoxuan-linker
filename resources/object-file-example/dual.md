# Dual Assembly Source File Example

## Source code

### `dual0.asm`

```asm
;; Defines a global variable and a function to increment it.

global num
global left
global right
global inc
global dec

section .rodata
    num dq 100              ;; read-only global variable with value 100

section .data
    left dq 11              ;; read-write global variable with initial value 41
    right dq 17             ;; read-write global variable with initial value 42

section .text

;; fn inc() -> int64_t
inc:
    mov rax, [rel left]     ;; read the value of left into rax
    add rax, 1              ;; increment rax by 1
    ret                     ;; return the incremented value in rax

;; fn dec() -> int64_t
dec:
    mov rax, [rel right]    ;; read the value of right into rax
    sub rax, 1              ;; decrement rax by 1
    ret                     ;; return the decremented value in rax
```

### `dual1.asm`

```asm
;; Entry point that imports symbols and exits with num + inc().

global _start
extern num
extern left
extern right
extern inc
extern dec

section .text

;; fn _start() -> void
_start:
    mov rbx, [rel left]     ;; read original num value into rbx, value is 11, but we will not use it
    mov rbx, [rel right]    ;; read original num value into rbx, value is 17, but we will not use it

    mov rbx, [rel num]      ;; read original num value into rbx, value is 100

    call inc                ;; call inc(), result is in rax, value is 12
    add rax, rbx            ;; sum original num and inc() result, value is 112

    mov rbx, rax
    call dec                ;; call dec(), result is in rax, value is 16
    add rax, rbx            ;; sum previous result and dec() result, value is 128

    ; syscall call `exit(status)`
    ; syscall number: 60

    mov rdi, rax        ;; move summed result into rdi (exit status)
    mov rax, 60         ;; syscall number for exit
    syscall
```

## Assemble, link, and run

```sh
nasm -f elf64 -o dual0.o dual0.asm
nasm -f elf64 -o dual1.o dual1.asm
ld -o dual.elf dual0.o dual1.o
# or `ld -pie -o dual.elf dual0.o dual1.o`
./dual.elf
echo $? # output: 128
```

## Symbols, Disassembly, and relocations

### Dual0.o

```text
$ readelf -s dual0.o

Symbol table '.symtab' contains 10 entries:
   Num:    Value          Size Type    Bind   Vis      Ndx Name
     0: 0000000000000000     0 NOTYPE  LOCAL  DEFAULT  UND
     1: 0000000000000000     0 FILE    LOCAL  DEFAULT  ABS dual0.asm
     2: 0000000000000000     0 SECTION LOCAL  DEFAULT    1 .rodata
     3: 0000000000000000     0 SECTION LOCAL  DEFAULT    2 .data
     4: 0000000000000000     0 SECTION LOCAL  DEFAULT    3 .text
     5: 0000000000000000     0 NOTYPE  GLOBAL DEFAULT    1 num
     6: 0000000000000000     0 NOTYPE  GLOBAL DEFAULT    2 left
     7: 0000000000000008     0 NOTYPE  GLOBAL DEFAULT    2 right
     8: 0000000000000000     0 NOTYPE  GLOBAL DEFAULT    3 inc
     9: 000000000000000c     0 NOTYPE  GLOBAL DEFAULT    3 dec

$ nm dual0.o
000000000000000c T dec
0000000000000000 T inc
0000000000000000 D left
0000000000000000 R num
0000000000000008 D right

$ objdump -M intel -sdr dual0.o

dual0.o:     file format elf64-x86-64

Contents of section .rodata:
 0000 64000000 00000000                    d.......
Contents of section .data:
 0000 0b000000 00000000 11000000 00000000  ................
Contents of section .text:
 0000 488b0500 00000048 83c001c3 488b0500  H......H....H...
 0010 00000048 83e801c3                    ...H....

Disassembly of section .text:

0000000000000000 <inc>:
   0:   48 8b 05 00 00 00 00    mov    rax,QWORD PTR [rip+0x0]        # 7 <inc+0x7>
                        3: R_X86_64_PC32        .data-0x4
   7:   48 83 c0 01             add    rax,0x1
   b:   c3                      ret

000000000000000c <dec>:
   c:   48 8b 05 00 00 00 00    mov    rax,QWORD PTR [rip+0x0]        # 13 <dec+0x7>
                        f: R_X86_64_PC32        .data+0x4
  13:   48 83 e8 01             sub    rax,0x1
  17:   c3                      ret

$ readelf -r dual0.o

Relocation section '.rela.text' at offset 0x3e0 contains 2 entries:
  Offset          Info           Type           Sym. Value    Sym. Name + Addend
000000000003  000300000002 R_X86_64_PC32     0000000000000000 .data - 4
00000000000f  000300000002 R_X86_64_PC32     0000000000000000 .data + 4
```

### Dual1.o

```text
$ readelf -s dual1.o

Symbol table '.symtab' contains 9 entries:
   Num:    Value          Size Type    Bind   Vis      Ndx Name
     0: 0000000000000000     0 NOTYPE  LOCAL  DEFAULT  UND
     1: 0000000000000000     0 FILE    LOCAL  DEFAULT  ABS dual1.asm
     2: 0000000000000000     0 SECTION LOCAL  DEFAULT    1 .text
     3: 0000000000000000     0 NOTYPE  GLOBAL DEFAULT  UND num
     4: 0000000000000000     0 NOTYPE  GLOBAL DEFAULT  UND left
     5: 0000000000000000     0 NOTYPE  GLOBAL DEFAULT  UND right
     6: 0000000000000000     0 NOTYPE  GLOBAL DEFAULT  UND inc
     7: 0000000000000000     0 NOTYPE  GLOBAL DEFAULT  UND dec
     8: 0000000000000000     0 NOTYPE  GLOBAL DEFAULT    1 _start

$ nm dual1.o
                 U dec
                 U inc
                 U left
                 U num
                 U right
0000000000000000 T _start

$ objdump -M intel -sdr dual1.o

dual1.o:     file format elf64-x86-64

Contents of section .text:
 0000 488b1d00 00000048 8b1d0000 0000488b  H......H......H.
 0010 1d000000 00e80000 00004801 d84889c3  ..........H..H..
 0020 e8000000 004801d8 4889c7b8 3c000000  .....H..H...<...
 0030 0f05                                 ..

Disassembly of section .text:

0000000000000000 <_start>:
   0:   48 8b 1d 00 00 00 00    mov    rbx,QWORD PTR [rip+0x0]        # 7 <_start+0x7>
                        3: R_X86_64_PC32        left-0x4
   7:   48 8b 1d 00 00 00 00    mov    rbx,QWORD PTR [rip+0x0]        # e <_start+0xe>
                        a: R_X86_64_PC32        right-0x4
   e:   48 8b 1d 00 00 00 00    mov    rbx,QWORD PTR [rip+0x0]        # 15 <_start+0x15>
                        11: R_X86_64_PC32       num-0x4
  15:   e8 00 00 00 00          call   1a <_start+0x1a>
                        16: R_X86_64_PC32       inc-0x4
  1a:   48 01 d8                add    rax,rbx
  1d:   48 89 c3                mov    rbx,rax
  20:   e8 00 00 00 00          call   25 <_start+0x25>
                        21: R_X86_64_PC32       dec-0x4
  25:   48 01 d8                add    rax,rbx
  28:   48 89 c7                mov    rdi,rax
  2b:   b8 3c 00 00 00          mov    eax,0x3c
  30:   0f 05                   syscall

$ readelf -r dual1.o

Relocation section '.rela.text' at offset 0x340 contains 5 entries:
  Offset          Info           Type           Sym. Value    Sym. Name + Addend
000000000003  000400000002 R_X86_64_PC32     0000000000000000 left - 4
00000000000a  000500000002 R_X86_64_PC32     0000000000000000 right - 4
000000000011  000300000002 R_X86_64_PC32     0000000000000000 num - 4
000000000016  000600000002 R_X86_64_PC32     0000000000000000 inc - 4
000000000021  000700000002 R_X86_64_PC32     0000000000000000 dec - 4
```

### Dual.elf

```text
$ readelf -S dual.elf
There are 7 section headers, starting at offset 0x21b0:

Section Headers:
  [Nr] Name              Type             Address           Offset
       Size              EntSize          Flags  Link  Info  Align
  [ 0]                   NULL             0000000000000000  00000000
       0000000000000000  0000000000000000           0     0     0
  [ 1] .text             PROGBITS         0000000000401000  00001000
       0000000000000052  0000000000000000  AX       0     0     16
  [ 2] .rodata           PROGBITS         0000000000402000  00002000
       0000000000000008  0000000000000000   A       0     0     4
  [ 3] .data             PROGBITS         0000000000403008  00002008
       0000000000000010  0000000000000000  WA       0     0     4
  [ 4] .symtab           SYMTAB           0000000000000000  00002018
       0000000000000120  0000000000000018           5     3     8
  [ 5] .strtab           STRTAB           0000000000000000  00002138
       0000000000000044  0000000000000000           0     0     1
  [ 6] .shstrtab         STRTAB           0000000000000000  0000217c
       000000000000002f  0000000000000000           0     0     1
Key to Flags:
  W (write), A (alloc), X (execute), M (merge), S (strings), I (info),
  L (link order), O (extra OS processing required), G (group), T (TLS),
  C (compressed), x (unknown), o (OS specific), E (exclude),
  D (mbind), l (large), p (processor specific)

$ readelf -s dual.elf

Symbol table '.symtab' contains 12 entries:
   Num:    Value          Size Type    Bind   Vis      Ndx Name
     0: 0000000000000000     0 NOTYPE  LOCAL  DEFAULT  UND
     1: 0000000000000000     0 FILE    LOCAL  DEFAULT  ABS dual0.asm
     2: 0000000000000000     0 FILE    LOCAL  DEFAULT  ABS dual1.asm
     3: 0000000000403010     0 NOTYPE  GLOBAL DEFAULT    3 right
     4: 0000000000403008     0 NOTYPE  GLOBAL DEFAULT    3 left
     5: 0000000000401020     0 NOTYPE  GLOBAL DEFAULT    1 _start
     6: 000000000040100c     0 NOTYPE  GLOBAL DEFAULT    1 dec
     7: 0000000000402000     0 NOTYPE  GLOBAL DEFAULT    2 num
     8: 0000000000403018     0 NOTYPE  GLOBAL DEFAULT    3 __bss_start
     9: 0000000000401000     0 NOTYPE  GLOBAL DEFAULT    1 inc
    10: 0000000000403018     0 NOTYPE  GLOBAL DEFAULT    3 _edata
    11: 0000000000403018     0 NOTYPE  GLOBAL DEFAULT    3 _end


$ nm dual.elf
0000000000403018 D __bss_start
000000000040100c T dec
0000000000403018 D _edata
0000000000403018 D _end
0000000000401000 T inc
0000000000403008 D left
0000000000402000 R num
0000000000403010 D right
0000000000401020 T _start

$ objdump -M intel -sdr dual.elf

dual.elf:     file format elf64-x86-64

Contents of section .text:
 401000 488b0501 20000048 83c001c3 488b05fd  H... ..H....H...
 401010 1f000048 83e801c3 0f1f8400 00000000  ...H............
 401020 488b1de1 1f000048 8b1de21f 0000488b  H......H......H.
 401030 1dcb0f00 00e8c6ff ffff4801 d84889c3  ..........H..H..
 401040 e8c7ffff ff4801d8 4889c7b8 3c000000  .....H..H...<...
 401050 0f05                                 ..
Contents of section .rodata:
 402000 64000000 00000000                    d.......
Contents of section .data:
 403008 0b000000 00000000 11000000 00000000  ................

Disassembly of section .text:

0000000000401000 <inc>:
  401000:       48 8b 05 01 20 00 00    mov    rax,QWORD PTR [rip+0x2001]        # 403008 <left>
  401007:       48 83 c0 01             add    rax,0x1
  40100b:       c3                      ret

000000000040100c <dec>:
  40100c:       48 8b 05 fd 1f 00 00    mov    rax,QWORD PTR [rip+0x1ffd]        # 403010 <right>
  401013:       48 83 e8 01             sub    rax,0x1
  401017:       c3                      ret
  401018:       0f 1f 84 00 00 00 00    nop    DWORD PTR [rax+rax*1+0x0]
  40101f:       00

0000000000401020 <_start>:
  401020:       48 8b 1d e1 1f 00 00    mov    rbx,QWORD PTR [rip+0x1fe1]        # 403008 <left>
  401027:       48 8b 1d e2 1f 00 00    mov    rbx,QWORD PTR [rip+0x1fe2]        # 403010 <right>
  40102e:       48 8b 1d cb 0f 00 00    mov    rbx,QWORD PTR [rip+0xfcb]        # 402000 <num>
  401035:       e8 c6 ff ff ff          call   401000 <inc>
  40103a:       48 01 d8                add    rax,rbx
  40103d:       48 89 c3                mov    rbx,rax
  401040:       e8 c7 ff ff ff          call   40100c <dec>
  401045:       48 01 d8                add    rax,rbx
  401048:       48 89 c7                mov    rdi,rax
  40104b:       b8 3c 00 00 00          mov    eax,0x3c
  401050:       0f 05                   syscall

$ readelf -r dual.elf

There are no relocations in this file.
```

### PIE dual.elf

```text
$ readelf -h dual.elf

  Type:                              DYN (Position-Independent Executable file)

$ readelf -S dual.elf
There are 14 section headers, starting at offset 0x3218:

Section Headers:
  [Nr] Name              Type             Address           Offset
       Size              EntSize          Flags  Link  Info  Align
  [ 0]                   NULL             0000000000000000  00000000
       0000000000000000  0000000000000000           0     0     0
  [ 1] .interp           PROGBITS         0000000000000200  00000200
       000000000000000f  0000000000000000   A       0     0     1
  [ 2] .hash             HASH             0000000000000210  00000210
       0000000000000010  0000000000000004   A       4     0     8
  [ 3] .gnu.hash         GNU_HASH         0000000000000220  00000220
       000000000000001c  0000000000000000   A       4     0     8
  [ 4] .dynsym           DYNSYM           0000000000000240  00000240
       0000000000000018  0000000000000018   A       5     1     8
  [ 5] .dynstr           STRTAB           0000000000000258  00000258
       0000000000000001  0000000000000000   A       0     0     1
  [ 6] .text             PROGBITS         0000000000001000  00001000
       0000000000000052  0000000000000000  AX       0     0     16
  [ 7] .rodata           PROGBITS         0000000000002000  00002000
       0000000000000008  0000000000000000   A       0     0     4
  [ 8] .eh_frame         PROGBITS         0000000000002008  00002008
       0000000000000000  0000000000000000   A       0     0     8
  [ 9] .dynamic          DYNAMIC          0000000000003f20  00002f20
       00000000000000e0  0000000000000010  WA       5     0     8
  [10] .data             PROGBITS         0000000000004000  00003000
       0000000000000010  0000000000000000  WA       0     0     4
  [11] .symtab           SYMTAB           0000000000000000  00003010
       0000000000000150  0000000000000018          12     5     8
  [12] .strtab           STRTAB           0000000000000000  00003160
       000000000000004d  0000000000000000           0     0     1
  [13] .shstrtab         STRTAB           0000000000000000  000031ad
       0000000000000064  0000000000000000           0     0     1
Key to Flags:
  W (write), A (alloc), X (execute), M (merge), S (strings), I (info),
  L (link order), O (extra OS processing required), G (group), T (TLS),
  C (compressed), x (unknown), o (OS specific), E (exclude),
  D (mbind), l (large), p (processor specific)

$ readelf -s dual.elf

Symbol table '.dynsym' contains 1 entry:
   Num:    Value          Size Type    Bind   Vis      Ndx Name
     0: 0000000000000000     0 NOTYPE  LOCAL  DEFAULT  UND

Symbol table '.symtab' contains 14 entries:
   Num:    Value          Size Type    Bind   Vis      Ndx Name
     0: 0000000000000000     0 NOTYPE  LOCAL  DEFAULT  UND
     1: 0000000000000000     0 FILE    LOCAL  DEFAULT  ABS dual0.asm
     2: 0000000000000000     0 FILE    LOCAL  DEFAULT  ABS dual1.asm
     3: 0000000000000000     0 FILE    LOCAL  DEFAULT  ABS
     4: 0000000000003f20     0 OBJECT  LOCAL  DEFAULT    9 _DYNAMIC
     5: 0000000000004008     0 NOTYPE  GLOBAL DEFAULT   10 right
     6: 0000000000004000     0 NOTYPE  GLOBAL DEFAULT   10 left
     7: 0000000000001020     0 NOTYPE  GLOBAL DEFAULT    6 _start
     8: 000000000000100c     0 NOTYPE  GLOBAL DEFAULT    6 dec
     9: 0000000000002000     0 NOTYPE  GLOBAL DEFAULT    7 num
    10: 0000000000004010     0 NOTYPE  GLOBAL DEFAULT   10 __bss_start
    11: 0000000000001000     0 NOTYPE  GLOBAL DEFAULT    6 inc
    12: 0000000000004010     0 NOTYPE  GLOBAL DEFAULT   10 _edata
    13: 0000000000004010     0 NOTYPE  GLOBAL DEFAULT   10 _end

$ nm dual.elf
0000000000004010 D __bss_start
000000000000100c T dec
0000000000003f20 d _DYNAMIC
0000000000004010 D _edata
0000000000004010 D _end
0000000000001000 T inc
0000000000004000 D left
0000000000002000 R num
0000000000004008 D right
0000000000001020 T _start

$ objdump -M intel -sdr dual.elf

dual.elf:     file format elf64-x86-64

Contents of section .interp:
 0200 2f6c6962 2f6c6436 342e736f 2e3100    /lib/ld64.so.1.
Contents of section .hash:
 0210 01000000 01000000 00000000 00000000  ................
Contents of section .gnu.hash:
 0220 01000000 01000000 01000000 00000000  ................
 0230 00000000 00000000 00000000           ............
Contents of section .dynsym:
 0240 00000000 00000000 00000000 00000000  ................
 0250 00000000 00000000                    ........
Contents of section .dynstr:
 0258 00                                   .
Contents of section .text:
 1000 488b05f9 2f000048 83c001c3 488b05f5  H.../..H....H...
 1010 2f000048 83e801c3 0f1f8400 00000000  /..H............
 1020 488b1dd9 2f000048 8b1dda2f 0000488b  H.../..H.../..H.
 1030 1dcb0f00 00e8c6ff ffff4801 d84889c3  ..........H..H..
 1040 e8c7ffff ff4801d8 4889c7b8 3c000000  .....H..H...<...
 1050 0f05                                 ..
Contents of section .rodata:
 2000 64000000 00000000                    d.......
Contents of section .dynamic:
 3f20 04000000 00000000 10020000 00000000  ................
 3f30 f5feff6f 00000000 20020000 00000000  ...o.... .......
 3f40 05000000 00000000 58020000 00000000  ........X.......
 3f50 06000000 00000000 40020000 00000000  ........@.......
 3f60 0a000000 00000000 01000000 00000000  ................
 3f70 0b000000 00000000 18000000 00000000  ................
 3f80 15000000 00000000 00000000 00000000  ................
 3f90 fbffff6f 00000000 00000008 00000000  ...o............
 3fa0 00000000 00000000 00000000 00000000  ................
 3fb0 00000000 00000000 00000000 00000000  ................
 3fc0 00000000 00000000 00000000 00000000  ................
 3fd0 00000000 00000000 00000000 00000000  ................
 3fe0 00000000 00000000 00000000 00000000  ................
 3ff0 00000000 00000000 00000000 00000000  ................
Contents of section .data:
 4000 0b000000 00000000 11000000 00000000  ................

Disassembly of section .text:

0000000000001000 <inc>:
    1000:       48 8b 05 f9 2f 00 00    mov    rax,QWORD PTR [rip+0x2ff9]        # 4000 <left>
    1007:       48 83 c0 01             add    rax,0x1
    100b:       c3                      ret

000000000000100c <dec>:
    100c:       48 8b 05 f5 2f 00 00    mov    rax,QWORD PTR [rip+0x2ff5]        # 4008 <right>
    1013:       48 83 e8 01             sub    rax,0x1
    1017:       c3                      ret
    1018:       0f 1f 84 00 00 00 00    nop    DWORD PTR [rax+rax*1+0x0]
    101f:       00

0000000000001020 <_start>:
    1020:       48 8b 1d d9 2f 00 00    mov    rbx,QWORD PTR [rip+0x2fd9]        # 4000 <left>
    1027:       48 8b 1d da 2f 00 00    mov    rbx,QWORD PTR [rip+0x2fda]        # 4008 <right>
    102e:       48 8b 1d cb 0f 00 00    mov    rbx,QWORD PTR [rip+0xfcb]        # 2000 <num>
    1035:       e8 c6 ff ff ff          call   1000 <inc>
    103a:       48 01 d8                add    rax,rbx
    103d:       48 89 c3                mov    rbx,rax
    1040:       e8 c7 ff ff ff          call   100c <dec>
    1045:       48 01 d8                add    rax,rbx
    1048:       48 89 c7                mov    rdi,rax
    104b:       b8 3c 00 00 00          mov    eax,0x3c
    1050:       0f 05                   syscall

$ readelf -r dual.elf

There are no relocations in this file.
```
