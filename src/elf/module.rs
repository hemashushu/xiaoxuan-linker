// Copyright (c) 2026 Hemashushu <hippospark@gmail.com>, All rights reserved.
//
// This Source Code Form is subject to the terms of
// the Mozilla Public License version 2.0 and additional exceptions.
// For more details, see the LICENSE, LICENSE.additional, and CONTRIBUTING files.

use object::elf;

// https://en.wikipedia.org/wiki/Executable_and_Linkable_Format
#[derive(Debug, PartialEq)]
pub struct FileHeader {
    pub os_abi: OSABI,
    pub machine: Machine,
    pub file_type: FileType,
    pub entry_point: usize,
    pub number_of_program_headers: usize,
    pub number_of_section_headers: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OSABI {
    SystemV, // ELFOSABI_NONE/ELFOSABI_SYSV
    // NetBSD,  // ELFOSABI_NETBSD
    // FreeBSD, // ELFOSABI_FREEBSD
    // OpenBSD, // ELFOSABI_OPENBSD
    // Hurd,    // ELFOSABI_HURD
    Other(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Machine {
    X86_64,    // EM_X86_64
    Aarch64,   // EM_AARCH64
    RiscV,     // EM_RISCV
    LoongArch, // EM_LOONGARCH
    // S390,      // EM_S390
    Other(u16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    Relocatable,  // ET_REL
    Executable,   // ET_EXEC
    SharedObject, // ET_DYN
    Other(u16),
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
            elf::EM_AARCH64 => Machine::Aarch64,
            elf::EM_RISCV => Machine::RiscV,
            elf::EM_LOONGARCH => Machine::LoongArch,
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
    pub name: String,
    pub section_type: SectionType,

    // the offset of the section in the file
    pub offset: usize,

    // the size of the section (in bytes), note that
    // it is not the size in file, but the size in memory,
    // for example, the size of the `.bss` section is 0 in file,
    // but it may be non-zero in memory.
    pub size: usize,

    pub align: usize,

    pub binary: &'data [u8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionType {
    Null,     // SHT_NULL
    Progbits, // SHT_PROGBITS
    Symtab,   // SHT_SYMTAB
    Strtab,   // SHT_STRTAB
    Rela,     // SHT_RELA
    Nobits,   // SHT_NOBITS
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
        // The name of the symbol
        // This name may be empty for symbols that represent sections (e.g. the symbol for the `.text` section).
        name: String,
        bind: SymbolBind,
        symbol_type: SymbolType,

        // the index of the section where the symbol is defined, which can be used to determine the symbol's section type
        section_index: usize,

        // The offset of the symbol in its original section in the object file.
        offset: usize,
    },
    External(/* name */ String),

    // Symbols that the linker does not care about.
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolBind {
    Local,
    Global,
    Weak,
    Other(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolType {
    Notype,  // Note that some assembler emit symbols with STT_NOTYPE type for functions and data.
    Object,  // Data object, e.g. global variable
    Func,    // Function
    Section, // Section
    File,    // File
    TLS,     // Thread-local storage
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
            elf::STT_FILE => SymbolType::File,
            elf::STT_TLS => SymbolType::TLS,
            other => SymbolType::Other(other),
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct RelocationSection {
    pub name: String,
    pub target_section_index: usize,
    pub relocations: Vec<Relocation>,
}

#[derive(Debug, PartialEq)]
pub struct Relocation {
    pub relocation_type: RelocationType,
    pub placeholder_offset: usize,
    pub symbol_index: usize,
    pub addend: isize,
}

/// The `RelocationType` enum represents the type of relocation that needs to be applied to
/// a symbol reference in the code or data.
///
/// We only support a few common relocation types for static linking on x86_64 architecture:
///
/// - `R_X86_64_PC32` (value 2): 32-bit PC-relative. Used by `mov`, `lea`, `call` in PIC code.
/// - `R_X86_64_64`  (value 1): 64-bit absolute. Used when a full pointer is stored in `.data`, e.g. `dq my_var`.
///
/// To support PIC/PIE dynamic linking executables, we also need to support the following relocation types:
///
/// - `R_X86_64_PLT32` (value 4): 32-bit PC-relative call through PLT.
///   Generated by the compiler for calls to external functions in PIC/PIE code.
///   The linker resolves it to a PLT stub (or directly if the symbol is in the same module).
/// - `R_X86_64_GOTPCREL` (value 9): 32-bit PC-relative GOT reference.
///   Generated by the compiler for accesses to external data symbols in PIC/PIE code.
///   The linker resolves it to a GOT entry.
///
/// To support thread-local storage (TLS), we also need to support the following relocation type:
///
/// - `R_X86_64_TPOFF32` (value 23): TLS local-exec thread-pointer offset.
///   Generated by the compiler for `__thread` variables with `-ftls-model=local-exec`.
///
/// To support PIE (or DSO, Dynamic Shared Object, `.so` file) which the source (relocatable object files)
/// contain `R_X86_64_64` relocations, the linker needs to convert those `R_X86_64_64` entries
/// into `R_X86_64_RELATIVE` (written to `.rela.dyn`), and write to the final binaries.
///
/// - `R_X86_64_RELATIVE` (value 8): base-relative pointer, written to `.rela.dyn` by the linker
///   when building PIE/DSO output. The runtime loader applies `*addr = B + A` after mapping
///   the binary at base address `B`.
///   Note that this type is never existing in a `.o` file.
///
/// In summary, for a *static non-PIE* linker, only `R_X86_64_PC32`, `R_X86_64_64`, and
/// `R_X86_64_TPOFF32` need to be handled as input relocations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum RelocationType {
    /// The `R_X86_64_PC32` relocation type represents a 32-bit PC-relative relocation.
    /// It is used for instructions that reference symbols, such as `mov`, `lea`, and `call`.
    /// This type is the most common relocation type used in PIC code.
    ///
    /// The formula for calculating the value to be written at the relocation site is:
    /// `S + A - P`
    /// where:
    /// - `S` is the value of the symbol (the address of the symbol in the final executable).
    /// - `A` is the addend specified in the relocation entry (the `addend` field in the `Relocation` struct).
    /// - `P` is the address of the relocation site (the `placeholder_offset` field in the `Relocation` struct).
    R_X86_64_PC32,

    /// The `R_X86_64_64` relocation type represents a 64-bit absolute relocation.
    /// It is produced by the assembler/compiler whenever a full 64-bit address is stored
    /// in a data section, for example:
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
    /// For a **non-PIE static executable** (ET_EXEC), this is resolved at link time by
    /// writing the final absolute address directly into the data section.
    ///
    /// For a **PIE executable** (ET_DYN), the load address is unknown at link time.
    /// A real PIE linker converts each `R_X86_64_64` entry into an `R_X86_64_RELATIVE`
    /// dynamic relocation (written to `.rela.dyn`), which the runtime loader applies
    /// after determining the base address. The value stored at the site becomes `B + S + A`
    /// where `B` is the base address chosen by the loader.
    R_X86_64_64,
    R_X86_64_32,

    /// For `local-exec` TLS model.
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
    /// The TLS block layout is:
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
}

#[derive(Debug, PartialEq)]
pub struct ProgramHeader {
    pub segment_type: SegmentType,
    pub segment_flags: Vec<SegmentFlag>,
    pub offset: usize,
    pub file_size: usize,
    pub memory_size: usize,
    pub virtual_address: usize,
    pub align: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentType {
    Null,
    PHDR,
    Load,
    TLS,
    Other(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentFlag {
    Execute,
    Write,
    Read,
    Other(u32),
}

impl From<u32> for SegmentType {
    fn from(value: u32) -> Self {
        match value {
            elf::PT_NULL => SegmentType::Null,
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