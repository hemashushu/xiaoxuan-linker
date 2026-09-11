// Copyright (c) 2026 Hemashushu <hippospark@gmail.com>, All rights reserved.
//
// This Source Code Form is subject to the terms of
// the Mozilla Public License version 2.0 and additional exceptions.
// For more details, see the LICENSE, LICENSE.additional, and CONTRIBUTING files.

use std::fmt::Display;

// Executable Mach-O 64-bit file layout (Apple Silicon / arm64)
// ============================================================
//
// Overall Structure
// -----------------
//
// | Size        | Content                     |
// |-------------|-----------------------------|
// | 32          | Mach-O 64-bit Header        |
// | sizeofcmds  | Load Commands               |
// | ...         | Segment / Section Data      |
// | ...         | Linkedit Data (Symbols, etc)|
//
// Common Segments and Sections (Executable)
// ----------------------------------------
//
// | Segment       | Section            | Description                     | Prot |
// |---------------|--------------------|---------------------------------|------|
// | `__PAGEZERO`  | -                  | Unmapped memory page (NULL ptr) | ---  |
// | `__TEXT`      | `__text`           | Executable code                 | R E  |
// |               | `__const`          | Read-only constants             | R E  |
// |               | `__stubs`          | Dynamic linker stubs            | R E  |
// |               | `__stub_helper`    | Dynamic linker stub helper      | R E  |
// | `__DATA_CONST`| `__got`            | Non-lazy symbol pointers (GOT)  | R W  |
// |               | `__const`          | Relocated constant pointers     | R W  |
// | `__DATA`      | `__la_symbol_ptr`  | Lazy symbol pointers            | R W  |
// |               | `__data`           | Initialized data                | R W  |
// |               | `__thread_vars`    | Thread-local variable headers   | R W  |
// |               | `__thread_data`    | Initialized thread-local data   | R W  |
// |               | `__thread_bss`     | Zero-init thread-local data     | R W  |
// |               | `__bss`            | Zero-initialized data           | R W  |
// | `__LINKEDIT`  | -                  | Symbol table, relocations, etc. | R    |

// Supported segment names
pub const SEGMENT_NAME_PAGEZERO: &str = "__PAGEZERO";
pub const SEGMENT_NAME_TEXT: &str = "__TEXT";
pub const SEGMENT_NAME_DATA_CONST: &str = "__DATA_CONST";
pub const SEGMENT_NAME_DATA: &str = "__DATA";
pub const SEGMENT_NAME_LINKEDIT: &str = "__LINKEDIT";

// Supported section names
pub const SECTION_NAME_TEXT: &str = "__text";
pub const SECTION_NAME_CONST: &str = "__const";
pub const SECTION_NAME_STUBS: &str = "__stubs";
pub const SECTION_NAME_STUB_HELPER: &str = "__stub_helper";
pub const SECTION_NAME_GOT: &str = "__got";
pub const SECTION_NAME_LA_SYMBOL_PTR: &str = "__la_symbol_ptr";
pub const SECTION_NAME_DATA: &str = "__data";
pub const SECTION_NAME_THREAD_VARS: &str = "__thread_vars";
pub const SECTION_NAME_THREAD_DATA: &str = "__thread_data";
pub const SECTION_NAME_THREAD_BSS: &str = "__thread_bss";
pub const SECTION_NAME_BSS: &str = "__bss";
pub const SECTION_NAME_COMMON: &str = "__common";

// Fixed sizes for Mach-O 64-bit structures
pub const MACH_HEADER_64_SIZE: u64 = 32;
pub const SEGMENT_COMMAND_64_SIZE: u64 = 72;
pub const SECTION_64_SIZE: u64 = 80;
pub const NLIST_64_SIZE: u64 = 16;
pub const RELOCATION_INFO_SIZE: u64 = 8;

// Page size for Apple Silicon / macOS arm64 (16KB)
pub const PAGE_SIZE_ARM64: u64 = 0x4000;

// Data and section alignments
pub const SECTION_ALIGN_DATA: u64 = 8;
pub const SECTION_ALIGN_TEXT: u64 = 4;

/// The Mach-O 64-bit file header information used by the linker.
#[derive(Debug, PartialEq)]
pub struct FileHeader {
    /// The target CPU architecture.
    pub cpu_type: CpuType,

    /// The target CPU sub-architecture flag.
    pub cpu_subtype: u32,

    /// The Mach-O file type (e.g., Object, Executable, Dylib).
    pub file_type: FileType,

    /// The number of load commands.
    pub load_commands_count: usize,

    /// The total byte size of all load commands.
    pub load_commands_size: u32,

    /// Mach-O header flags (e.g., MH_NOUNDEFS, MH_PIE, MH_DYLDLINK).
    pub flags: u32,

    /// Reserved 32-bit field in Mach-O 64-bit header.
    pub reserved: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// CPU architecture in Mach-O header.
pub enum CpuType {
    /// ARM 64-bit architecture (`CPU_TYPE_ARM64`).
    ARM64,
    /// x86 64-bit architecture (`CPU_TYPE_X86_64`).
    X86_64,
    /// An unrecognized CPU type value.
    Other(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Mach-O file type.
pub enum FileType {
    /// Relocatable object file (`MH_OBJECT`).
    Object,
    /// Demand-paged executable file (`MH_EXECUTE`).
    Executable,
    /// Dynamic shared library (`MH_DYLIB`).
    Dylib,
    /// Dynamic link editor (`MH_DYLINKER`).
    Dylinker,
    /// Dynamically bound bundle (`MH_BUNDLE`).
    Bundle,
    /// An unrecognized file type value.
    Other(u32),
}

impl From<object::macho::CpuType> for CpuType {
    fn from(value: object::macho::CpuType) -> Self {
        match value {
            object::macho::CPU_TYPE_ARM64 => CpuType::ARM64,
            object::macho::CPU_TYPE_X86_64 => CpuType::X86_64,
            other => CpuType::Other(other.0),
        }
    }
}

impl From<CpuType> for object::macho::CpuType {
    fn from(value: CpuType) -> Self {
        match value {
            CpuType::ARM64 => object::macho::CPU_TYPE_ARM64,
            CpuType::X86_64 => object::macho::CPU_TYPE_X86_64,
            CpuType::Other(val) => object::macho::CpuType(val),
        }
    }
}

impl From<object::macho::FileType> for FileType {
    fn from(value: object::macho::FileType) -> Self {
        match value {
            object::macho::MH_OBJECT => FileType::Object,
            object::macho::MH_EXECUTE => FileType::Executable,
            object::macho::MH_DYLIB => FileType::Dylib,
            object::macho::MH_DYLINKER => FileType::Dylinker,
            object::macho::MH_BUNDLE => FileType::Bundle,
            other => FileType::Other(other.0),
        }
    }
}

impl From<FileType> for object::macho::FileType {
    fn from(value: FileType) -> Self {
        match value {
            FileType::Object => object::macho::MH_OBJECT,
            FileType::Executable => object::macho::MH_EXECUTE,
            FileType::Dylib => object::macho::MH_DYLIB,
            FileType::Dylinker => object::macho::MH_DYLINKER,
            FileType::Bundle => object::macho::MH_BUNDLE,
            FileType::Other(val) => object::macho::FileType(val),
        }
    }
}

impl Display for CpuType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CpuType::ARM64 => write!(f, "arm64"),
            CpuType::X86_64 => write!(f, "x86_64"),
            CpuType::Other(val) => write!(f, "unknown ({})", val),
        }
    }
}

impl Display for FileType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FileType::Object => write!(f, "MH_OBJECT"),
            FileType::Executable => write!(f, "MH_EXECUTE"),
            FileType::Dylib => write!(f, "MH_DYLIB"),
            FileType::Dylinker => write!(f, "MH_DYLINKER"),
            FileType::Bundle => write!(f, "MH_BUNDLE"),
            FileType::Other(val) => write!(f, "unknown ({})", val),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Mach-O load command command type (`cmd`).
pub enum LoadCommandType {
    /// 64-bit segment command (`LC_SEGMENT_64`).
    Segment64,
    /// Symbol table command (`LC_SYMTAB`).
    Symtab,
    /// Dynamic symbol table command (`LC_DYSYMTAB`).
    Dysymtab,
    /// Dynamic linker load command (`LC_LOAD_DYLINKER`).
    LoadDylinker,
    /// Dynamic library load command (`LC_LOAD_DYLIB`).
    LoadDylib,
    /// Main entry point command (`LC_MAIN`).
    Main,
    /// Build version command (`LC_BUILD_VERSION`).
    BuildVersion,
    /// Source version command (`LC_SOURCE_VERSION`).
    SourceVersion,
    /// Data in code command (`LC_DATA_IN_CODE`).
    DataInCode,
    /// Function starts command (`LC_FUNCTION_STARTS`).
    FunctionStarts,
    /// Code signature command (`LC_CODE_SIGNATURE`).
    CodeSignature,
    /// Unrecognized load command type.
    Other(u32),
}

impl From<object::macho::LoadCommandType> for LoadCommandType {
    fn from(value: object::macho::LoadCommandType) -> Self {
        match value {
            object::macho::LC_SEGMENT_64 => LoadCommandType::Segment64,
            object::macho::LC_SYMTAB => LoadCommandType::Symtab,
            object::macho::LC_DYSYMTAB => LoadCommandType::Dysymtab,
            object::macho::LC_LOAD_DYLINKER => LoadCommandType::LoadDylinker,
            object::macho::LC_LOAD_DYLIB => LoadCommandType::LoadDylib,
            object::macho::LC_MAIN => LoadCommandType::Main,
            object::macho::LC_BUILD_VERSION => LoadCommandType::BuildVersion,
            object::macho::LC_SOURCE_VERSION => LoadCommandType::SourceVersion,
            object::macho::LC_DATA_IN_CODE => LoadCommandType::DataInCode,
            object::macho::LC_FUNCTION_STARTS => LoadCommandType::FunctionStarts,
            object::macho::LC_CODE_SIGNATURE => LoadCommandType::CodeSignature,
            other => LoadCommandType::Other(other.0),
        }
    }
}

/// VM memory protection permissions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VmProtection(pub u32);

impl VmProtection {
    pub const NONE: VmProtection = VmProtection(0);
    pub const READ: VmProtection = VmProtection(object::macho::VM_PROT_READ.0);
    pub const WRITE: VmProtection = VmProtection(object::macho::VM_PROT_WRITE.0);
    pub const EXECUTE: VmProtection = VmProtection(object::macho::VM_PROT_EXECUTE.0);

    pub fn is_readable(&self) -> bool {
        (self.0 & object::macho::VM_PROT_READ.0) != 0
    }

    pub fn is_writable(&self) -> bool {
        (self.0 & object::macho::VM_PROT_WRITE.0) != 0
    }

    pub fn is_executable(&self) -> bool {
        (self.0 & object::macho::VM_PROT_EXECUTE.0) != 0
    }
}

/// A 64-bit Mach-O segment command (`LC_SEGMENT_64`).
#[derive(Debug, PartialEq)]
pub struct SegmentCommand<'data> {
    /// Segment name (e.g., `"__TEXT"`, `"__DATA"`).
    pub segment_name: String,

    /// Virtual memory address of the segment (`vmaddr`).
    pub virtual_address: u64,

    /// Virtual memory size of the segment (`vmsize`).
    pub virtual_size: u64,

    /// File offset of the segment data (`fileoff`).
    pub file_offset: u64,

    /// File size of the segment data (`filesize`).
    pub file_size: u64,

    /// Maximum VM protection (`maxprot`).
    pub max_protection: VmProtection,

    /// Initial VM protection (`initprot`).
    pub init_protection: VmProtection,

    /// Segment flags.
    pub flags: u32,

    /// Sections contained in this segment.
    pub sections: Vec<SectionHeader<'data>>,
}

/// Header information for a section in a Mach-O 64-bit file.
#[derive(Debug, PartialEq)]
pub struct SectionHeader<'data> {
    /// Segment name containing this section (e.g., `"__TEXT"`).
    pub segment_name: String,

    /// Section name (e.g., `"__text"`).
    pub section_name: String,

    /// Virtual memory address of the section in memory (`addr`).
    pub virtual_address: u64,

    /// Size of the section in bytes (`size`).
    pub size: u64,

    /// Offset of the section data in the file (`offset`).
    pub offset: u32,

    /// Required alignment in bytes (computed from `2^align`).
    pub align: u64,

    /// File offset to relocation entries (`reloff`).
    pub relocation_offset: u32,

    /// Number of relocation entries (`nreloc`).
    pub relocation_count: u32,

    /// Raw flags field containing section type and attributes.
    pub flags: u32,

    /// Section type parsed from flags.
    pub section_type: SectionType,

    /// Binary content of the section.
    pub binary: &'data [u8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Mach-O section type (extracted from low 8 bits of section flags `SECTION_TYPE`).
pub enum SectionType {
    /// Regular section containing data or instructions (`S_REGULAR`).
    Regular,
    /// Zero-filled section in memory (`S_ZEROFILL`).
    ZeroFill,
    /// Literal C-string section (`S_CSTRING_LITERALS`).
    CStringLiterals,
    /// 4-byte literal constants (`S_4BYTE_LITERALS`).
    FourByteLiterals,
    /// 8-byte literal constants (`S_8BYTE_LITERALS`).
    EightByteLiterals,
    /// Literal pointers (`S_LITERAL_POINTERS`).
    LiteralPointers,
    /// Non-lazy symbol pointers / GOT (`S_NON_LAZY_SYMBOL_POINTERS`).
    NonLazySymbolPointers,
    /// Lazy symbol pointers (`S_LAZY_SYMBOL_POINTERS`).
    LazySymbolPointers,
    /// Symbol stubs (`S_SYMBOL_STUBS`).
    SymbolStubs,
    /// Thread-local regular data (`S_THREAD_LOCAL_REGULAR`).
    ThreadLocalRegular,
    /// Thread-local zero-fill data (`S_THREAD_LOCAL_ZEROFILL`).
    ThreadLocalZeroFill,
    /// Thread-local variable structures (`S_THREAD_LOCAL_VARIABLES`).
    ThreadLocalVariables,
    /// An unrecognized section type.
    Other(u8),
}

impl From<u8> for SectionType {
    fn from(value: u8) -> Self {
        match value {
            0x00 => SectionType::Regular,
            0x01 => SectionType::ZeroFill,
            0x02 => SectionType::CStringLiterals,
            0x03 => SectionType::FourByteLiterals,
            0x04 => SectionType::EightByteLiterals,
            0x05 => SectionType::LiteralPointers,
            0x06 => SectionType::NonLazySymbolPointers,
            0x07 => SectionType::LazySymbolPointers,
            0x08 => SectionType::SymbolStubs,
            0x11 => SectionType::ThreadLocalRegular,
            0x12 => SectionType::ThreadLocalZeroFill,
            0x13 => SectionType::ThreadLocalVariables,
            other => SectionType::Other(other),
        }
    }
}

#[derive(Debug, PartialEq)]
/// A symbol in a Mach-O file.
pub enum Symbol {
    Null,

    /// A symbol defined in a section of the Mach-O file (`N_SECT`).
    Defined {
        /// Symbol name (including leading underscore if present, e.g. `_main`).
        name: String,

        /// Binding / scope of the symbol (`Local`, `Global`, or `Weak`).
        bind: SymbolBind,

        /// Symbol type.
        symbol_type: SymbolType,

        /// 1-based section index in the Mach-O file (`n_sect`).
        section_index: usize,

        /// Symbol address or section-relative offset.
        value: u64,
    },

    /// An absolute symbol (`N_ABS`).
    Absolute {
        /// Symbol name.
        name: String,

        /// Symbol binding.
        bind: SymbolBind,

        /// Absolute value.
        value: u64,
    },

    /// An undefined external symbol (`N_UNDF`).
    External(String),

    /// A symbol representing a source file name (`N_SO`).
    File(String),

    /// Other symbol types (e.g. debugging stabs).
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Symbol binding / visibility scope in Mach-O.
pub enum SymbolBind {
    /// Local symbol.
    Local,
    /// Global / external symbol (`N_EXT`).
    Global,
    /// Weak definition or reference (`N_WEAK_DEF` / `N_WEAK_REF`).
    Weak,
    /// Other binding.
    Other(u16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Symbol entity type in Mach-O.
pub enum SymbolType {
    /// No specific type.
    Notype,
    /// Global data object.
    Object,
    /// Function code.
    Func,
    /// Section symbol.
    Section,
    /// Thread-local symbol.
    TLS,
    /// Other symbol type.
    Other(u8),
}

#[derive(Debug, PartialEq)]
/// A collection of relocation entries targeting a specific section.
pub struct RelocationSection {
    /// Name of the section (e.g. `"__TEXT,__text"`).
    pub name: String,

    /// Index of the target section.
    pub target_section_index: usize,

    /// List of relocations in this section.
    pub relocations: Vec<Relocation>,
}

#[derive(Debug, PartialEq, Clone)]
/// A Mach-O relocation entry (`relocation_info`).
pub struct Relocation {
    /// The architecture-specific relocation type.
    pub relocation_type: RelocationType,

    /// Byte offset in the target section where relocation is applied (`r_address`).
    pub offset: u64,

    /// True if PC-relative relocation (`r_pcrel`).
    pub is_pcrel: bool,

    /// Size of field to relocate in bytes: 1, 2, 4, or 8 (derived from `r_length`).
    pub length: u8,

    /// True if symbol index refers to external symbol table, false if section ordinal.
    pub is_extern: bool,

    /// Referenced symbol index (if `is_extern` is true) or section index (if `is_extern` is false).
    pub symbol_index: usize,

    /// Explicit addend (if `ARM64_RELOC_ADDEND`) or implicit addend read from section data.
    pub addend: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
/// ARM64 relocation types in Mach-O (`ARM64_RELOC_*`).
pub enum RelocationType {
    /// Absolute relocation: writes target address + addend (`ARM64_RELOC_UNSIGNED`).
    ARM64_RELOC_UNSIGNED,

    /// Subtractor relocation for difference calculations (`ARM64_RELOC_SUBTRACTOR`).
    ARM64_RELOC_SUBTRACTOR,

    /// 26-bit PC-relative branch relocation for `b` / `bl` (`ARM64_RELOC_BRANCH26`).
    ARM64_RELOC_BRANCH26,

    /// High 21-bit page relocation for `adrp` (`ARM64_RELOC_PAGE21`).
    ARM64_RELOC_PAGE21,

    /// Low 12-bit page offset relocation for `add` / `ldr` / `str` (`ARM64_RELOC_PAGEOFF12`).
    ARM64_RELOC_PAGEOFF12,

    /// GOT load page relocation for `adrp` (`ARM64_RELOC_GOT_LOAD_PAGE21`).
    ARM64_RELOC_GOT_LOAD_PAGE21,

    /// GOT load page offset relocation for `ldr` (`ARM64_RELOC_GOT_LOAD_PAGEOFF12`).
    ARM64_RELOC_GOT_LOAD_PAGEOFF12,

    /// Pointer to GOT relocation (`ARM64_RELOC_POINTER_TO_GOT`).
    ARM64_RELOC_POINTER_TO_GOT,

    /// TLV load page relocation for `adrp` (`ARM64_RELOC_TLVP_LOAD_PAGE21`).
    ARM64_RELOC_TLVP_LOAD_PAGE21,

    /// TLV load page offset relocation for `ldr` (`ARM64_RELOC_TLVP_LOAD_PAGEOFF12`).
    ARM64_RELOC_TLVP_LOAD_PAGEOFF12,

    /// Explicit addend relocation (`ARM64_RELOC_ADDEND`).
    ARM64_RELOC_ADDEND,

    /// Unrecognized relocation type.
    Other(u8),
}

impl From<u8> for RelocationType {
    fn from(value: u8) -> Self {
        match value {
            0 => RelocationType::ARM64_RELOC_UNSIGNED,
            1 => RelocationType::ARM64_RELOC_SUBTRACTOR,
            2 => RelocationType::ARM64_RELOC_BRANCH26,
            3 => RelocationType::ARM64_RELOC_PAGE21,
            4 => RelocationType::ARM64_RELOC_PAGEOFF12,
            5 => RelocationType::ARM64_RELOC_GOT_LOAD_PAGE21,
            6 => RelocationType::ARM64_RELOC_GOT_LOAD_PAGEOFF12,
            7 => RelocationType::ARM64_RELOC_POINTER_TO_GOT,
            8 => RelocationType::ARM64_RELOC_TLVP_LOAD_PAGE21,
            9 => RelocationType::ARM64_RELOC_TLVP_LOAD_PAGEOFF12,
            10 => RelocationType::ARM64_RELOC_ADDEND,
            other => RelocationType::Other(other),
        }
    }
}

/// A relocatable module parsed from an input Mach-O relocatable object file (`MH_OBJECT`).
#[derive(Debug, PartialEq)]
pub struct RelocatableModule<'a> {
    /// Identifier or filename of the module.
    pub name: String,

    /// File header information.
    pub header: FileHeader,

    /// Sections contained in the module.
    pub sections: Vec<SectionHeader<'a>>,

    /// Symbols in the module.
    pub symbols: Vec<Symbol>,

    /// Relocation sections for code and data sections.
    pub relocation_sections: Vec<RelocationSection>,
}

pub fn get_load_address_base(cpu_type: CpuType) -> u64 {
    match cpu_type {
        // Typical base address for macOS 64-bit executables (above __PAGEZERO segment of 4GB)
        CpuType::ARM64 => 0x100000000,
        CpuType::X86_64 => 0x100000000,
        _ => unimplemented!("Unsupported CPU architecture: {}", cpu_type),
    }
}

pub fn get_segment_align_page_size(cpu_type: CpuType) -> u64 {
    match cpu_type {
        // Apple Silicon macOS uses 16KB pages
        CpuType::ARM64 => PAGE_SIZE_ARM64,
        CpuType::X86_64 => 0x1000,
        _ => unimplemented!("Unsupported CPU architecture: {}", cpu_type),
    }
}

pub fn get_section_align_text(cpu_type: CpuType) -> u64 {
    match cpu_type {
        CpuType::ARM64 => SECTION_ALIGN_TEXT,
        CpuType::X86_64 => 16,
        _ => unimplemented!("Unsupported CPU architecture: {}", cpu_type),
    }
}
