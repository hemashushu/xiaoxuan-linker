// Copyright (c) 2026 Hemashushu <hippospark@gmail.com>, All rights reserved.
//
// This Source Code Form is subject to the terms of
// the Mozilla Public License version 2.0 and additional exceptions.
// For more details, see the LICENSE, LICENSE.additional, and CONTRIBUTING files.

use object::elf;

/// The ELF file header information used by the linker.
// https://en.wikipedia.org/wiki/Executable_and_Linkable_Format
#[derive(Debug, PartialEq)]
pub struct FileHeader {
    /// The byte order used by the ELF file.
    pub data_encoding: DataEncoding,

    /// The ELF class: 32-bit or 64-bit.
    pub file_class: FileClass,

    /// The operating-system ABI identified by the ELF header.
    pub os_abi: OSABI,

    /// The target machine architecture.
    pub machine: Machine,

    /// The ELF file type, such as relocatable, executable, or shared object.
    pub file_type: FileType,

    /// The virtual address of the entry point.
    pub entry_point: usize,

    /// The number of entries in the program header table.
    pub program_header_count: usize,

    /// The number of entries in the section header table.
    pub section_header_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// The byte order encoded in an ELF file.
pub enum DataEncoding {
    /// Least-significant byte first (`ELFDATA2LSB`).
    LittleEndian,
    /// Most-significant byte first (`ELFDATA2MSB`).
    BigEndian,
    /// An encoding value not recognized by this linker.
    Other(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// The word size encoded in an ELF file.
pub enum FileClass {
    /// 32-bit ELF (`ELFCLASS32`).
    Elf32,
    /// 64-bit ELF (`ELFCLASS64`).
    Elf64,
    /// An ELF class value not recognized by this linker.
    Other(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// The operating-system ABI identified by an ELF file.
pub enum OSABI {
    /// System V ABI (`ELFOSABI_NONE` or `ELFOSABI_SYSV`).
    SystemV,
    // NetBSD,  // ELFOSABI_NETBSD
    // FreeBSD, // ELFOSABI_FREEBSD
    // OpenBSD, // ELFOSABI_OPENBSD
    // Hurd,    // ELFOSABI_HURD
    /// An OS ABI value not recognized by this linker.
    Other(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// A target machine architecture supported or recognized by the linker.
pub enum Machine {
    /// x86-64 (`EM_X86_64`).
    X86_64,
    /// AArch64 (`EM_AARCH64`).
    AArch64,
    /// RISC-V (`EM_RISCV`).
    RiscV,
    /// LoongArch (`EM_LOONGARCH`).
    LoongArch,
    /// 64-bit PowerPC (`EM_PPC64`).
    PowerPC64,
    /// IBM Z (`EM_S390`).
    S390,
    /// A machine value not recognized by this linker.
    Other(u16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// The kind of ELF file.
pub enum FileType {
    /// Relocatable object file (`ET_REL`).
    Relocatable,
    /// Executable file (`ET_EXEC`).
    Executable,
    /// Shared object or position-independent executable (`ET_DYN`).
    SharedObject,
    /// A file type value not recognized by this linker.
    Other(u16),
}

impl From<u8> for DataEncoding {
    fn from(value: u8) -> Self {
        match value {
            elf::ELFDATA2LSB => DataEncoding::LittleEndian,
            elf::ELFDATA2MSB => DataEncoding::BigEndian,
            other => DataEncoding::Other(other),
        }
    }
}

impl From<u8> for FileClass {
    fn from(value: u8) -> Self {
        match value {
            elf::ELFCLASS32 => FileClass::Elf32,
            elf::ELFCLASS64 => FileClass::Elf64,
            other => FileClass::Other(other),
        }
    }
}

impl From<u8> for OSABI {
    fn from(value: u8) -> Self {
        match value {
            elf::ELFOSABI_SYSV => OSABI::SystemV,
            other => OSABI::Other(other),
        }
    }
}

impl From<u16> for Machine {
    fn from(value: u16) -> Self {
        match value {
            elf::EM_X86_64 => Machine::X86_64,
            elf::EM_AARCH64 => Machine::AArch64,
            elf::EM_RISCV => Machine::RiscV,
            elf::EM_LOONGARCH => Machine::LoongArch,
            elf::EM_PPC64 => Machine::PowerPC64,
            elf::EM_S390 => Machine::S390,
            other => Machine::Other(other),
        }
    }
}

impl From<u16> for FileType {
    fn from(value: u16) -> Self {
        match value {
            elf::ET_REL => FileType::Relocatable,
            elf::ET_EXEC => FileType::Executable,
            elf::ET_DYN => FileType::SharedObject,
            other => FileType::Other(other),
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct SectionHeader<'data> {
    /// The section name.
    pub name: String,

    /// The section type, such as `Progbits`, `Nobits`, or `Symtab`.
    pub section_type: SectionType,

    /// The byte offset of the section data in the file.
    pub offset: usize,

    /// The size of the section in bytes.
    ///
    /// For a `Nobits` section, such as `.bss`, this is the size occupied in
    /// memory; the section has no corresponding bytes in the file.
    pub size: usize,

    /// The required alignment of the section in bytes.
    pub align: usize,

    /// The section data stored in the input file.
    ///
    /// This is empty for a `Nobits` section.
    pub binary: &'data [u8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// The kind of an ELF section.
pub enum SectionType {
    /// Inactive section header (`SHT_NULL`).
    Null,
    /// Section containing defined data (`SHT_PROGBITS`).
    Progbits,
    /// Symbol table (`SHT_SYMTAB`).
    Symtab,
    /// String table (`SHT_STRTAB`).
    Strtab,
    /// Relocation entries with explicit addends (`SHT_RELA`).
    Rela,
    /// Section occupying memory but no bytes in the file (`SHT_NOBITS`).
    Nobits,
    /// A section type value not recognized by this linker.
    Other(u32),
}

impl From<u32> for SectionType {
    fn from(value: u32) -> Self {
        match value {
            elf::SHT_NULL => SectionType::Null,
            elf::SHT_PROGBITS => SectionType::Progbits,
            elf::SHT_SYMTAB => SectionType::Symtab,
            elf::SHT_STRTAB => SectionType::Strtab,
            elf::SHT_RELA => SectionType::Rela,
            elf::SHT_NOBITS => SectionType::Nobits,
            other => SectionType::Other(other),
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum Symbol {
    Defined {
        /// The symbol name.
        ///
        /// This may be empty for a section symbol, such as the symbol for
        /// the `.text` section.
        name: String,

        /// The symbol binding, such as `Local`, `Global`, or `Weak`.
        bind: SymbolBind,

        /// The symbol type.
        symbol_type: SymbolType,

        /// The index of the section that defines the symbol.
        section_index: usize,

        /// The offset of the symbol within its original section.
        offset: usize,
    },
    /// An undefined symbol that must be resolved by the linker.
    External(String),

    /// A symbol that is not relevant to the linker's relocation process.
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// The binding or visibility scope of a symbol.
pub enum SymbolBind {
    /// A symbol local to its object file.
    Local,
    /// A symbol available for resolution by other object files.
    Global,
    /// A symbol that may be overridden by a non-weak definition.
    Weak,
    /// A binding value not recognized by this linker.
    Other(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// The kind of entity represented by a symbol.
pub enum SymbolType {
    /// No specific type. Assemblers may use this for functions or data.
    Notype,
    /// A data object, such as a global variable.
    Object,
    /// A function.
    Func,
    /// A section symbol.
    Section,
    // File,    // File
    /// Thread-local storage.
    TLS,
    /// A symbol type value not recognized by this linker.
    Other(u8),
}

impl From<u8> for SymbolBind {
    fn from(value: u8) -> Self {
        match value {
            elf::STB_LOCAL => SymbolBind::Local,
            elf::STB_GLOBAL => SymbolBind::Global,
            elf::STB_WEAK => SymbolBind::Weak,
            other => SymbolBind::Other(other),
        }
    }
}

impl From<u8> for SymbolType {
    fn from(value: u8) -> Self {
        match value {
            elf::STT_NOTYPE => SymbolType::Notype,
            elf::STT_OBJECT => SymbolType::Object,
            elf::STT_FUNC => SymbolType::Func,
            elf::STT_SECTION => SymbolType::Section,
            // elf::STT_FILE => SymbolType::File,
            elf::STT_TLS => SymbolType::TLS,
            other => SymbolType::Other(other),
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct RelocationSection {
    /// The name of the relocation section.
    pub name: String,

    /// The index of the section to which these relocations apply.
    pub target_section_index: usize,

    /// The relocation entries in this section.
    pub relocations: Vec<Relocation>,
}

#[derive(Debug, PartialEq)]
pub struct Relocation {
    /// The architecture-specific relocation type.
    pub relocation_type: RelocationType,

    /// The offset of the value or instruction field to be patched.
    pub placeholder_offset: usize,

    /// The index of the referenced symbol in the symbol table.
    pub symbol_index: usize,

    /// The addend used by the relocation calculation.
    pub addend: isize,
}

/// The type of relocation that must be applied to a symbol reference in code or data.
///
/// For static linking on x86_64, we only need a few common relocation types:
///
/// - `R_X86_64_PC32` (value 2): 32-bit PC-relative. Used by instructions such as `mov`, `lea`,
///   and `call` when their encoding uses a 32-bit PC-relative field.
/// - `R_X86_64_64`  (value 1): 64-bit absolute. Used when a full pointer is stored in `.data`,
///   e.g. `dq my_var`.
///
/// When building a dynamic shared object (DSO, `.so`), a linker may represent an address that
/// depends only on the load base with an `R_X86_64_RELATIVE` relocation in `.rela.dyn`.
///
/// `R_X86_64_RELATIVE` (value 8) is a base-relative relocation written to `.rela.dyn` when
/// building DSO output. The runtime loader (`ld.so`, the dynamic linker) applies
/// `*addr = B + A` after mapping the binary at base address `B`.
///
/// This relocation is normally emitted in a final dynamically linked image rather than in an
/// input relocatable object file.
///
/// To support PIC shared libraries, additional relocation types are typically required:
///
/// - `R_X86_64_PLT32` (value 4): 32-bit PC-relative call target associated with the PLT.
///   It is commonly generated for calls to external functions. The linker may resolve it to a
///   PLT stub or directly to a local definition.
/// - `R_X86_64_GOTPCREL` (value 9): 32-bit PC-relative GOT reference.
///   Generated by the compiler for accesses to external data symbols in PIC code.
///   The linker resolves it to a GOT entry.
///
/// For thread-local storage (TLS), the following relocation type is also needed:
///
/// - `R_X86_64_TPOFF32` (value 23): TLS local-exec thread-pointer offset.
///   Generated by the compiler for `__thread` variables with `-ftls-model=local-exec`.
///
///
/// In this linker's current static, non-PIE mode, the relevant x86_64 input relocations are
/// `R_X86_64_PC32`, `R_X86_64_PLT32`, `R_X86_64_64`, `R_X86_64_32`, and `R_X86_64_TPOFF32`.
///
/// The following relocation types can appear in `.rela.eh_frame`:
///
/// - R_AARCH64_PREL32
///
/// - R_RISCV_32_PCREL
/// - R_RISCV_ADD32
/// - R_RISCV_SUB32
///
/// Adding parameters `-fno-unwind-tables -fno-asynchronous-unwind-tables` to the compiler avoids generating `.eh_frame`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum RelocationType {
    /// A 32-bit PC-relative relocation.
    ///
    /// It is used by instruction encodings that reference symbols through a 32-bit
    /// PC-relative field, including some forms of `mov`, `lea`, and `call`.
    ///
    /// The formula for calculating the value to be written at the relocation site is:
    /// `S + A - P`
    /// where:
    /// - `S` is the value of the symbol (the address of the symbol in the final executable).
    /// - `A` is the addend specified in the relocation entry (the `addend` field in the `Relocation` struct).
    /// - `P` is the address of the relocation site (the `placeholder_offset` field in the `Relocation` struct).
    R_X86_64_PC32,

    /// A 32-bit PC-relative relocation for a function call associated with a PLT entry.
    ///
    /// It is commonly used for calls to external symbols.
    ///
    /// Because this linker does not support dynamic linking, we can treat `R_X86_64_PLT32` the
    /// same as `R_X86_64_PC32` for static linking purposes.
    R_X86_64_PLT32,

    /// A 64-bit absolute relocation.
    ///
    /// It is used when a full 64-bit address is stored in a data section, for example:
    ///
    /// ```asm
    /// my_array:
    ///     dq my_var    ; -> R_X86_64_64 pointing to my_var
    ///     dq my_func   ; -> R_X86_64_64 pointing to my_func
    /// ```
    ///
    /// The formula for calculating the value to be written at the relocation site is:
    /// `S + A`
    /// where:
    /// - `S` is the absolute address of the symbol in the final executable.
    /// - `A` is the addend specified in the relocation entry.
    ///
    /// For a non-PIE static executable (ET_EXEC), this is resolved at link time by
    /// writing the final absolute address directly into the data section.
    ///
    /// For a shared library (ET_DYN), the load address is unknown at link time. A dynamic linker
    /// can use an `R_X86_64_RELATIVE` dynamic relocation in `.rela.dyn` when the symbol is local
    /// to the image. The dynamic linker then adds the load base `B` to the link-time value.
    R_X86_64_64,

    /// A 32-bit absolute relocation whose result is written to a 32-bit field.
    R_X86_64_32,

    /// A 32-bit thread-pointer-relative relocation for the `local-exec` TLS model.
    ///
    /// There are also other TLS models:
    /// - `initial-exec`: `R_X86_64_GOTTPOFF`
    /// - `local-dynamic`: `R_X86_64_TLSLD`
    /// - `global-dynamic`: `R_X86_64_TLSGD`
    ///
    /// Currently, only `local-exec` is supported.
    ///
    /// The formula for calculating the value to be written at the relocation site is:
    /// TPOFF(sym) = symbol_offset_in_tls_block − tls_block_size_rounded
    ///
    /// For example, if we have the following TLS variable declarations in C:
    ///
    /// ```c
    /// __thread int var1;  // offset 0 in the TLS block
    /// __thread int var2;  // offset 4 in the TLS block
    /// ```
    ///
    /// The size of TLS block would be 8 bytes (assuming 4 bytes for each `int`),
    /// and the offsets of `var1` and `var2` in the TLS block would be 0 and 4, respectively.
    /// Then the value to be written at the relocation site for `var1` would be `0 - 8 = -8`,
    /// and for `var2` would be `4 - 8 = -4`.
    ///
    /// Note that the value is negative because the TLS block grows downwards from the thread pointer (TP).
    ///
    /// A simplified TLS block layout is:
    ///
    /// Higher addresses
    /// +---------------------------+
    /// | other TCB fields (if any) |
    /// | self pointer (TCB)        | [fs:0] = FS.base
    /// +---------------------------+
    /// | var2 (offset 4)           | [fs:-4] = FS.base - 4 (tpoff = -4)
    /// | var1 (offset 0)           | [fs:-8] = FS.base - 8 (tpoff = -8)
    /// +---------------------------+
    /// Lower addresses
    R_X86_64_TPOFF32,

    /// `R_AARCH64_ADR_PREL_PG_HI21` and `R_AARCH64_ADD_ABS_LO12_NC`/
    /// `R_AARCH64_LDST64_ABS_LO12_NC` are used to form an address from a page and a page offset.
    ///
    /// - Name 'R_AARCH64_ADR_PREL_PG_HI21' = 'ADRP instruction' + 'PC relative' + 'Page' + 'High 21 bits immediate'
    /// - Name 'R_AARCH64_ADD_ABS_LO12_NC' = 'Add instruction' + 'Absolute' + 'Low 12 bits immediate' + 'Non-Checked'
    /// - Name 'R_AARCH64_LDST64_ABS_LO12_NC' = 'Load/Store instruction' + '64-bit' + 'Absolute' + 'Low 12 bits immediate' + 'Non-Checked'
    ///
    /// There are two steps to form an address and load from it on AArch64:
    ///
    /// ```asm
    /// adrp x0, foo
    /// ldr x0, [x0, :lo12:foo]
    /// ```
    ///
    /// 1. Use `ADRP` to calculate the target symbol's page address relative to the PC.
    /// 2. Use a load/store instruction with the symbol's 12-bit offset within that page.
    ///
    /// The relocation entries for this sequence are `R_AARCH64_ADR_PREL_PG_HI21` + `R_AARCH64_LDST64_ABS_LO12_NC`.
    ///
    /// The page address and page offset can also be combined with `ADD` before loading:
    ///
    /// ```asm
    /// adrp x0, foo
    /// add x0, x0, :lo12:foo
    /// ldr x0, [x0]
    /// ```
    ///
    /// The relocation entries for this sequence are `R_AARCH64_ADR_PREL_PG_HI21` + `R_AARCH64_ADD_ABS_LO12_NC`.
    R_AARCH64_ADR_PREL_PG_HI21,
    R_AARCH64_ADD_ABS_LO12_NC,
    R_AARCH64_LDST64_ABS_LO12_NC,

    /// A 26-bit PC-relative relocation for a function call or jump on AArch64.
    R_AARCH64_CALL26,

    /// A 64-bit absolute relocation on AArch64.
    ///
    /// It is similar to `R_X86_64_64` in x86_64 architecture, and is used when
    /// a full 64-bit address is stored in a data section.
    R_AARCH64_ABS64,

    /// A pair of relocations used to construct a PC-relative address on RISC-V.
    R_RISCV_PCREL_HI20,
    R_RISCV_PCREL_LO12_I,

    /// Absolute high and low-part relocations commonly emitted by GCC.
    R_RISCV_HI20,
    R_RISCV_LO12_I,
    R_RISCV_LO12_S,

    /// A PC-relative function-call relocation that may target a PLT entry on RISC-V.
    R_RISCV_CALL_PLT,

    /// A 64-bit absolute relocation on RISC-V.
    ///
    /// It is similar to `R_X86_64_64` in x86_64 architecture, and is used when
    /// a full 64-bit address is stored in a data section.
    R_RISCV_64,

    /// The high 20 bits of a PC-relative address on LoongArch.
    R_LARCH_PCALA_HI20,

    /// The low 12 bits of a PC-relative address on LoongArch.
    R_LARCH_PCALA_LO12,

    /// A 64-bit absolute relocation on LoongArch.
    R_LARCH_64,

    /// A 26-bit PC-relative relocation on LoongArch.
    R_LARCH_B26,

    /// A 36-bit PC-relative function-call relocation on LoongArch.
    R_LARCH_CALL36,

    /// A 16-bit high-part address relocation on PowerPC64.
    R_PPC64_ADDR16_HI,

    /// A 16-bit low-part address relocation on PowerPC64.
    R_PPC64_ADDR16_LO,

    /// The higher 16 bits of an address on PowerPC64.
    R_PPC64_ADDR16_HIGHER,

    /// The adjusted higher 16 bits of an address on PowerPC64.
    R_PPC64_ADDR16_HIGHERA,

    /// The highest 16 bits of an address on PowerPC64.
    R_PPC64_ADDR16_HIGHEST,

    /// The adjusted highest 16 bits of an address on PowerPC64.
    R_PPC64_ADDR16_HIGHESTA,

    /// A 64-bit absolute address relocation on PowerPC64.
    R_PPC64_ADDR64,

    /// A 24-bit PC-relative branch relocation on PowerPC64.
    R_PPC64_REL24,

    /// A 16-bit PC-relative high-adjusted relocation on PowerPC64.
    R_PPC64_REL16_HA,

    /// A 16-bit PC-relative low-part relocation on PowerPC64.
    R_PPC64_REL16_LO,

    /// A 16-bit high-adjusted TOC-relative relocation on PowerPC64.
    R_PPC64_TOC16_HA,

    /// A 16-bit low-part TOC-relative relocation on PowerPC64.
    R_PPC64_TOC16_LO,

    /// A 32-bit PC-relative relocation whose value is divided by two on S390x.
    R_390_PC32DBL,

    /// A 32-bit PC-relative PLT relocation whose value is divided by two on S390x.
    R_390_PLT32DBL,

    /// A 64-bit absolute address relocation on S390x.
    R_390_64,
}

#[derive(Debug, PartialEq)]
/// A loadable or otherwise relevant program segment in an ELF file.
pub struct ProgramHeader {
    /// The segment type.
    pub segment_type: SegmentType,

    /// The permissions assigned to the segment.
    pub segment_flags: Vec<SegmentFlag>,

    /// The byte offset of the segment data in the file.
    pub offset: usize,

    /// The number of bytes occupied by the segment in the file.
    pub file_size: usize,

    /// The number of bytes occupied by the segment in memory.
    pub memory_size: usize,

    /// The virtual address at which the segment is loaded.
    pub virtual_address: usize,

    /// The required alignment of the segment in bytes.
    pub align: usize,
}

/// Segment types relevant to the linker's output.
// The linker does not need to model some segment types, such as `PT_DYNAMIC`,
// `PT_INTERP`, and `PT_NOTE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentType {
    /// The program header table itself (`PT_PHDR`).
    PHDR,
    /// A loadable segment (`PT_LOAD`).
    Load,
    /// A thread-local storage segment (`PT_TLS`).
    TLS,
    /// A segment type not recognized by this linker.
    Other(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// A permission bit assigned to an ELF program segment.
pub enum SegmentFlag {
    /// The segment is executable (`PF_X`).
    Execute,
    /// The segment is writable (`PF_W`).
    Write,
    /// The segment is readable (`PF_R`).
    Read,
    /// A segment flag value not recognized by this linker.
    Other(u32),
}

impl From<u32> for SegmentType {
    fn from(value: u32) -> Self {
        match value {
            elf::PT_PHDR => SegmentType::PHDR,
            elf::PT_LOAD => SegmentType::Load,
            elf::PT_TLS => SegmentType::TLS,
            other => SegmentType::Other(other),
        }
    }
}

impl From<u32> for SegmentFlag {
    fn from(value: u32) -> Self {
        match value {
            elf::PF_X => SegmentFlag::Execute,
            elf::PF_W => SegmentFlag::Write,
            elf::PF_R => SegmentFlag::Read,
            other => SegmentFlag::Other(other),
        }
    }
}
