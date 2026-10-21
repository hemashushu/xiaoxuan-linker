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
// | Size       | Content                                                       |
// |------------|---------------------------------------------------------------|
// | 32         | Mach-O 64-bit Header                                          |
// | sizeofcmds | Load Commands                                                 | <-- variable-length records
// | ...        | Data (code, relocations, symbol table and string table etc.)  | <-- binary
//
// Note that there is no explicit load command for relocations, the relocation entries are typically
// followed directly by the `__TEXT` segment, but before the `__TEXT, __common` (uninitialized data) section
// if present.
//
// Object (MH_OBJECT) Load Commands:
//
// - LC_SEGMENT_64: Segment command for 64-bit architectures
//   In the object file, all segments are merged into a single segment (typically named empty "")
//   for compacted size purposes.
// - LC_BUILD_VERSION: Build version command
// - LC_SYMTAB: Symbol table command.
//   Symbols are grouped by their types, these types include:
//   - Local symbols
//   - External symbols (Global/Exported symbols)
//   - Undefined symbols
// - LC_DYSYMTAB: The grouping information for the symbols.
//
// Object Segments and Sections:
//
// - `__TEXT, __text`: Executable code section
// - `__TEXT, __const`: Read-only constants (includes char arrays) section
// - `__DATA, __data`: Initialized data section
// - `__DATA, __thread_data`: Thread-local storage data section
// - `__DATA, __thread_vars`: Thread-local storage variable section
// - `__DATA, __thread_bss`: Thread-local storage uninitialized data section
// - `__DATA, __common`: Uninitialized data section, similar to the `.bss` section in ELF.
//   Note that by default, clang generates `undefined+external` symbols (with non-zero
//   value that indicates the size) for uninitialized data, and no section `__common` is created in this case,
//   these symbols are treated as common symbols by the linker and multiple definitions are all allowed.
//   This behavior is error-prone and it is not consistent with the behavior of GCC.
//   To avoid this, it is recommended to use the `-fno-common` flag when compiling with clang.
// - `__TEXT, __cstring`: String literals
//   Duplicate string literals may be merged by the compiler, and the common-tail strings may also be optimized.
//
// Executable (MH_EXECUTE) Load Commands:
//
// - LC_SEGMENT_64 (__PAGEZERO): Segment with VM size (0x100000000) and zero file size.
// - LC_SEGMENT_64 (__TEXT): Segment containing the executable code and read-only data.
// - LC_SEGMENT_64 (__DATA_CONST): Segment containing read-only data that may be modified at runtime (e.g., GOT).
// - LC_SEGMENT_65 (__DATA): Segment containing writable data.
// - LC_SEGMENT_64 (__LINKEDIT)
// - LC_DYLD_CHAINED_FIXUPS
// - LC_DYLD_EXPORTS_TRIE
// - LC_SYMTAB
// - LC_DYSYMTAB
// - LC_LOAD_DYLINKER: "/usr/lib/dyld" (exec SPEC)
// - LC_UUID: e.g. "ECBA8DD6-628E-4233-A3EA-8A4BAC6EE0AB"
// - LC_BUILD_VERSION
// - LC_SOURCE_VERSION
// - LC_MAIN: Entry point for the executable. (exec SPEC)
// - LC_LOAD_DYLIB: "/usr/lib/libSystem.B.dylib" (stdc)
// - LC_FUNCTION_STARTS
// - LC_DATA_IN_CODE
// - LC_CODE_SIGNATURE
//
// Executable Segments and Sections:
// ----------------------------------------
//
// | Segment       | Section            | Description                     | Prot |
// |---------------|--------------------|---------------------------------|------|
// | `__PAGEZERO`  | no section         |                                 | NONE |
// |---------------|--------------------|---------------------------------|------|
// | `__TEXT`      | `__text`           | Executable code                 | R E  |
// |               | `__const`          | Read-only constants             | R E  |
// |               | `__stubs`          | Dynamic linker stubs            | R E  | // see "function.macho", "override.macho"
// |               | -                  |                                 |      |
// |               | `__cstring`        | String literals                 | R E  | // see "external.macho"
// |               | `__auth_stubs`     | Authenticated stubs             | R E  | // see "fat.arm64e.macho"
// |---------------|--------------------|---------------------------------|------|
// | `__DATA`      | `__data`           | Initialized data                | R W  |
// |               | `__common`         | Uninitialized data              | R W  | // see "data.macho"
// |               | -                  |                                 |      |
// |               | `__thread_vars`    | Thread-local variable headers   | R W  | // see "tls.macho"
// |               | `__thread_data`    | Initialized thread-local data   | R W  | // see "tls.macho"
// |               | `__thread_bss`     | Zero-init thread-local data     | R W  | // see "tls.macho"
// |---------------|--------------------|---------------------------------|------|
// | `__DATA_CONST`| `__got`            | Global Offset Table (GOT)       | R W  | // see "function.macho", "override.macho"
// |               | `__auth_got`       | Authenticated (GOT)             | R E  | // see "fat.arm64e.macho"
// |               | `__const`          | Relocated constant pointers     | R W  | // see "relocate-within-data.macho", "relocate-within-tls.macho"
// |---------------|--------------------|---------------------------------|------|
// | `__LINKEDIT`  | no section         |                                 | R    |
// |---------------|--------------------|---------------------------------|------|
//
// Dylib (Dynamic Library, MH_DYLIB) Segments and Sections:
//
// - LC_SEGMENT_64 (__TEXT)
// - LC_SEGMENT_65 (__DATA)
// - LC_SEGMENT_64 (__LINKEDIT)
// - LC_ID_DYLIB: "lib${NAME}.dylib": (dylib SPEC)
// - LC_DYLD_CHAINED_FIXUPS
// - LC_DYLD_EXPORTS_TRIE
// - LC_SYMTAB
// - LC_DYSYMTAB
// - LC_UUID: e.g. "ECBA8DD6-628E-4233-A3EA-8A4BAC6EE0AB"
// - LC_BUILD_VERSION
// - LC_SOURCE_VERSION
// - LC_LOAD_DYLIB: "/usr/lib/libSystem.B.dylib" (stdc)
// - LC_FUNCTION_STARTS
// - LC_DATA_IN_CODE
// - LC_CODE_SIGNATURE
//
// References:
// - https://www.reinterpretcast.com/hello-world-mach-o
// - https://oliviagallucci.com/the-anatomy-of-a-mach-o-structure-code-signing-and-pac/
// - https://alexdremov.me/mystery-of-mach-o-object-file-builders/
// - https://github.com/gdbinit/machoview

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
pub const SECTION_NAME_CSTRING: &str = "__cstring";
pub const SECTION_NAME_AUTH_STUBS: &str = "__auth_stubs";
pub const SECTION_NAME_DATA: &str = "__data";
pub const SECTION_NAME_COMMON: &str = "__common";
pub const SECTION_NAME_THREAD_DATA: &str = "__thread_data";
pub const SECTION_NAME_THREAD_VARS: &str = "__thread_vars";
pub const SECTION_NAME_THREAD_BSS: &str = "__thread_bss";
pub const SECTION_NAME_GOT: &str = "__got";
pub const SECTION_NAME_AUTH_GOT: &str = "__auth_got";
pub const SECTION_NAME_DATA_CONST_CONST: &str = "__const";

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
/// Mach-O 64-bit magic number (`MH_MAGIC_64` = 0xfeedfacf).
#[derive(Debug, PartialEq)]
pub struct FileHeader {
    /// The target CPU architecture.
    pub cpu_type: CpuType,

    /// The target CPU sub-architecture flag.
    /// - CPU_SUBTYPE_ARM64_ALL
    /// - CPU_SUBTYPE_ARM64E (2) | CPU_SUBTYPE_LIB64 (0x8000_0000)
    /// - CPU_SUBTYPE_ARM64E_X1 (12)
    /// - CPU_SUBTYPE_X86_64_ALL (3)
    pub cpu_subtype: u32,

    /// The Mach-O file type (e.g., Object, Executable, Dylib).
    pub file_type: FileType,

    /// The number of load commands.
    pub load_commands_count: usize,

    /// The total byte size of all load commands.
    pub load_commands_size: u32,

    /// Mach-O header flags, e.g.
    /// - Object:
    ///   - MH_SUBSECTIONS_VIA_SYMBOLS
    /// - Executable:
    ///   - MH_NOUNDEFS
    ///   - MH_DYLDLINK
    ///   - MH_TWOLEVEL
    ///   - MH_PIE
    /// - Dylib:
    ///   - MH_NOUNDEFS
    ///   - MH_DYLDLINK
    ///   - MH_TWOLEVEL
    ///   - MH_NO_REEXPORTED_DYLIBS
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

    // /// Dynamic link editor (`MH_DYLINKER`).
    // Dylinker,
    // /// Dynamically bound bundle (`MH_BUNDLE`).
    // Bundle,
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
            // object::macho::MH_DYLINKER => FileType::Dylinker,
            // object::macho::MH_BUNDLE => FileType::Bundle,
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
            // FileType::Dylinker => object::macho::MH_DYLINKER,
            // FileType::Bundle => object::macho::MH_BUNDLE,
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
            // FileType::Dylinker => write!(f, "MH_DYLINKER"),
            // FileType::Bundle => write!(f, "MH_BUNDLE"),
            FileType::Other(val) => write!(f, "unknown ({})", val),
        }
    }
}

/// Fat header of a Mach-O universal binary.
///
/// Contains the magic number, the number of architectures, and a list of entries for each architecture.
/// Magic number is `FAT_CIGAM` (0xbebafeca).
#[derive(Debug, PartialEq)]
pub struct FatHeader {
    /// List of entries for each architecture in the universal binary.
    pub entries: Vec<FatHeaderEntry>,
}

#[derive(Debug, PartialEq)]
pub struct FatHeaderEntry {
    pub cputype: CpuType,
    pub cpusubtype: u32,

    /// Offset of the architecture in the universal binary.
    pub offset: u32,

    /// Size of the architecture in the universal binary.
    pub size: u32,

    /// Alignment of the architecture in the universal binary.
    /// 16KB (16384) by default
    pub align: u32,
}

#[derive(Debug, PartialEq)]
pub struct LoadCommand<'data> {
    pub variant: LoadCommandVariant<'data>,

    /// Size of the load command in bytes.
    /// Load commands are variable in size, and the `size` field indicates the total size of this command,
    /// including the load command structure itself and directly associated data. e.g.
    ///
    /// - A segment command (`LC_SEGMENT_64`) with 4 sections would has:
    ///   72 (segment command structure) + 80 (section structure) * 4 = 392 bytes.
    pub size: u32,
}

#[derive(Debug, PartialEq)]
/// Mach-O load command command type (`cmd`).
pub enum LoadCommandVariant<'data> {
    /// 64-bit segment command (`LC_SEGMENT_64`).
    Segment64(SegmentCommand<'data>),
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

// impl From<object::macho::LoadCommandType> for LoadCommandVariant {
//     fn from(value: object::macho::LoadCommandType) -> Self {
//         match value {
//             object::macho::LC_SEGMENT_64 => LoadCommandVariant::Segment64,
//             object::macho::LC_SYMTAB => LoadCommandVariant::Symtab,
//             object::macho::LC_DYSYMTAB => LoadCommandVariant::Dysymtab,
//             object::macho::LC_LOAD_DYLINKER => LoadCommandVariant::LoadDylinker,
//             object::macho::LC_LOAD_DYLIB => LoadCommandVariant::LoadDylib,
//             object::macho::LC_MAIN => LoadCommandVariant::Main,
//             object::macho::LC_BUILD_VERSION => LoadCommandVariant::BuildVersion,
//             object::macho::LC_SOURCE_VERSION => LoadCommandVariant::SourceVersion,
//             object::macho::LC_DATA_IN_CODE => LoadCommandVariant::DataInCode,
//             object::macho::LC_FUNCTION_STARTS => LoadCommandVariant::FunctionStarts,
//             object::macho::LC_CODE_SIGNATURE => LoadCommandVariant::CodeSignature,
//             other => LoadCommandVariant::Other(other.0),
//         }
//     }
// }

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

    // /// File offset of the segment data (`fileoff`).
    // pub file_offset: u64,

    // /// File size of the segment data (`filesize`).
    // pub file_size: u64,
    /// Maximum VM protection (`maxprot`).
    ///
    /// e.g.
    ///
    /// `__PAGEZERO` segment:
    /// - VM_PROT_NONE (0x0000_0000)
    ///
    /// `__TEXT` segment:
    /// - VM_PROT_READ (0x0000_0001)
    /// - VM_PROT_EXECUTE (0x0000_0004)
    ///
    /// `__DATA` segment:
    /// - VM_PROT_READ (0x0000_0001)
    /// - VM_PROT_WRITE (0x0000_0002)
    ///
    /// `__LINKEDIT` segment:
    /// - VM_PROT_READ (0x0000_0001)
    pub max_protection: VmProtection,

    /// Initial VM protection (`initprot`).
    /// The value is usually the same as `max_protection`.
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

    /// Memory size of the section in bytes (`size`).
    ///
    /// Some sections may have a memory size while having
    /// a file size of zero (zero-filled sections, e.g., `__DATA, __common` which
    /// is similar to the `.bss` sections in ELF files).
    pub memory_size: u64,

    // /// Offset of the section data in the file (`offset`).
    // pub offset: u32,
    /// Required alignment in bytes (computed from `2^align`).
    /// e.g.
    /// `align = 2` in the original Mach-O file means the section is aligned to `2^2 = 4` bytes,
    /// and `align = 3` means `2^3 = 8` bytes.
    pub align: u64,

    // /// File offset to relocation entries (`reloff`).
    // pub relocation_offset: u32,

    // /// Number of relocation entries (`nreloc`).
    // pub relocation_count: u32,
    pub relocations: Vec<Relocation>,

    /// Raw flags field containing section type and attributes.
    /// e.g.
    ///
    /// `__TEXT, __text` section:
    /// - `S_REGULAR` (0x0000_0000) for regular sections.
    /// - `S_ATTR_PURE_INSTRUCTIONS` (0x8000_0000) for pure instructions.
    /// - `S_ATTR_SOME_INSTRUCTIONS` (0x0000_0400) for sections containing some instructions.
    ///
    /// `__TEXT, __const` and `__DATA, __data` section:
    /// - `S_REGULAR` (0x0000_0000) for regular sections.
    ///
    /// `__TEXT, __stubs` and `__TEXT, __auth_stubs` section:
    /// - `S_SYMBOL_STUBS` (0x0000_0008) for symbol stubs.
    /// - `S_ATTR_PURE_INSTRUCTIONS` (0x8000_0000) for pure instructions.
    /// - `S_ATTR_SOME_INSTRUCTIONS` (0x0000_0400) for sections containing some instructions.
    ///
    /// `__TEXT, __cstring` section:
    /// - `S_CSTRING_LITERALS` (0x0000_0002) for C-string literals.
    ///
    /// `__DATA, __common` section:
    /// - `S_ZEROFILL` (0x0000_0100) for zero-filled sections.
    ///
    /// `__DATA, __thread_vars` section:
    /// - `S_THREAD_LOCAL_VARIABLES` (0x0000_0013) for thread-local variable structures.
    ///
    /// `__DATA, __thread_data` section:
    /// - `S_THREAD_LOCAL_REGULAR` (0x0000_0011) for thread-local regular data.
    ///
    /// `__DATA, __thread_bss` section:
    /// - `S_THREAD_LOCAL_ZEROFILL` (0x0000_0012) for thread-local zero-fill data.
    ///
    /// `__DATA_CONST, __got` and `__DATA, __auth_got` section:
    /// - `S_NON_LAZY_SYMBOL_POINTERS` (0x0000_0006) for non-lazy symbol pointers.
    ///
    /// `__DATA_CONST, __const` section:
    /// - `S_REGULAR` (0x0000_0000) for regular sections.
    pub flags: u32,

    /// Binary content of the section.
    pub binary: &'data [u8],
}

// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
// /// Mach-O section type (extracted from low 8 bits of section flags `SECTION_TYPE`).
// pub enum SectionType {
//     /// Regular section containing data or instructions (`S_REGULAR`).
//     Regular,
//     /// Zero-filled section in memory (`S_ZEROFILL`).
//     ZeroFill,
//     /// Literal C-string section (`S_CSTRING_LITERALS`).
//     CStringLiterals,
//     /// 4-byte literal constants (`S_4BYTE_LITERALS`).
//     FourByteLiterals,
//     /// 8-byte literal constants (`S_8BYTE_LITERALS`).
//     EightByteLiterals,
//     /// Literal pointers (`S_LITERAL_POINTERS`).
//     LiteralPointers,
//     /// Non-lazy symbol pointers / GOT (`S_NON_LAZY_SYMBOL_POINTERS`).
//     NonLazySymbolPointers,
//     /// Lazy symbol pointers (`S_LAZY_SYMBOL_POINTERS`).
//     LazySymbolPointers,
//     /// Symbol stubs (`S_SYMBOL_STUBS`).
//     SymbolStubs,
//     /// Thread-local regular data (`S_THREAD_LOCAL_REGULAR`).
//     ThreadLocalRegular,
//     /// Thread-local zero-fill data (`S_THREAD_LOCAL_ZEROFILL`).
//     ThreadLocalZeroFill,
//     /// Thread-local variable structures (`S_THREAD_LOCAL_VARIABLES`).
//     ThreadLocalVariables,
//     /// An unrecognized section type.
//     Other(u8),
// }

// impl From<u8> for SectionType {
//     fn from(value: u8) -> Self {
//         match value {
//             0x00 => SectionType::Regular,
//             0x01 => SectionType::ZeroFill,
//             0x02 => SectionType::CStringLiterals,
//             0x03 => SectionType::FourByteLiterals,
//             0x04 => SectionType::EightByteLiterals,
//             0x05 => SectionType::LiteralPointers,
//             0x06 => SectionType::NonLazySymbolPointers,
//             0x07 => SectionType::LazySymbolPointers,
//             0x08 => SectionType::SymbolStubs,
//             0x11 => SectionType::ThreadLocalRegular,
//             0x12 => SectionType::ThreadLocalZeroFill,
//             0x13 => SectionType::ThreadLocalVariables,
//             other => SectionType::Other(other),
//         }
//     }
// }

#[derive(Debug, PartialEq, Clone)]
/// A Mach-O relocation entry (`relocation_info`).
pub struct Relocation {
    /// Byte offset in the target section where relocation is applied (`r_address`).
    pub offset: u64,

    /// Referenced symbol index (if `is_extern` is true) or section index (if `is_extern` is false).
    ///
    /// The following fields are combined into one 32-bit value which structure is:
    ///
    /// ```
    /// |--------|---------|-------------|
    /// | 0..23  | 24 bits | r_symbolnum |
    /// | 24     |  1 bit  | r_pcrel     |
    /// | 25..26 |  2 bits | r_length    |
    /// | 27     |  1 bit  | r_extern    |
    /// | 28..31 |  4 bits | r_type      |
    /// ```
    pub symbol_index: usize,

    /// True if PC-relative relocation (`r_pcrel`).
    pub is_pcrel: bool,

    /// Size of field to relocate in bytes: 1, 2, 4, or 8 (derived from `r_length`).
    ///
    /// The original `r_length` value is:
    /// - 0: indicates 1 byte
    /// - 1: indicates 2 bytes
    /// - 2: indicates 4 bytes
    /// - 3: indicates 8 bytes
    pub length: u8,

    /// True if symbol index refers to external symbol table, false if section ordinal.
    pub is_extern: bool,

    /// The architecture-specific relocation type.
    pub relocation_type: RelocationType,
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

/// A symbol in a Mach-O file.
///
/// Reference: https://alexdremov.me/mystery-of-mach-o-object-file-builders/#symbol-table
#[derive(Debug, PartialEq)]
pub struct Symbol {
    /// Position of the symbol's name in the string table.
    ///
    /// For example, for given a string table "\0_print\0_puts\0\0",
    /// the index of “_print” in the string table above is 1,
    /// the index of "_puts" is 8 (= len("\0_print\0"));
    /// It is the position of the first letter from the start of the string table.
    pub string_position: u32,

    /// A type of symbol. Defines the meaning of the symbol. There are essential values:
    /// - `N_TYPE (0x0e)` – These bits define the type of the symbol.
    /// - `N_UNDF (0x0)` – The symbol is undefined. Undefined symbols are symbols referenced
    ///   in this module but defined in a different module. The n_sect field is set to NO_SECT.
    /// - `N_ABS (0x2)` – The symbol is absolute. The linker does not change the value of
    ///   an absolute symbol. The n_sect field is set to NO_SECT.
    /// - `N_SECT (0xe)` – The symbol is defined in the section number given in n_sect.
    /// - `N_PBUD (0xc)` – The symbol is undefined and the image is using a prebound value
    ///   for the symbol. The n_sect field is set to NO_SECT.
    /// - `N_INDR (0xa)` – The symbol is defined to be the same as another symbol.
    ///   The n_value field is an index into the string table specifying the name of the other symbol.
    ///   When that symbol is linked, both this and the other symbol have the same defined type and value.
    /// - `N_EXT  (0x01)` – If this bit is on, this symbol is external, a symbol that is
    ///   either defined outside this file or that is defined in this file but can be referenced by other files.
    /// - `N_STAB (0xe0)` – If any of these 3 bits are set, the symbol is a symbolic debugging
    ///   table (stab) entry. In that case, the entire n_type field is interpreted as a stabvalue.
    pub type_: u8,

    /// An integer specifying the number of the section that this symbol can be found in,
    /// or NO_SECT if the symbol is not to be found in any section.
    pub section_number: u8,

    /// Provides additional information about the nature of this symbol for non-stab
    /// symbols (not N_STAB). The reference flags can be accessed using the REFERENCE_TYPE mask (0xF).
    /// Usually, REFERENCE_FLAG_UNDEFINED_NON_LAZY used for external symbols.
    /// If the symbol is defined in the section (N_SECT), use REFERENCE_FLAG_DEFINED + N_EXT if
    /// you want to make it available from other files or REFERENCE_FLAG_PRIVATE_DEFINED without
    /// specifying N_EXT if not. The most used values are:
    /// - `REFERENCE_FLAG_UNDEFINED_NON_LAZY (0x0)` - This symbol is a reference to an
    ///    external non-lazy (data) symbol.
    /// - `REFERENCE_FLAG_UNDEFINED_LAZY (0x1)` - This symbol is a reference to an external
    ///    lazy symbol—that is, to a function call.
    /// - `REFERENCE_FLAG_DEFINED (0x2)` - This symbol is defined in this module.
    /// - `REFERENCE_FLAG_PRIVATE_DEFINED (0x3)` - This symbol is defined in this module
    ///    and is visible only to modules within this shared library.
    /// - `REFERENCE_FLAG_PRIVATE_UNDEFINED_NON_LAZY (0x4)` - This symbol is defined in
    ///    another module in this file, is a non-lazy (data) symbol, and is visible
    ///    only to modules within this shared library.
    /// - `REFERENCE_FLAG_PRIVATE_UNDEFINED_LAZY (0x5)` - This symbol is defined
    ///    in another module in this file, is a lazy (function) symbol,
    ///    and is visible only to modules within this shared library.
    pub description: u16,

    /// Information about this symbol.
    /// The format of this value is different for each type of symbol table entry
    /// (as specified by the n_type field). For the N_SECT symbol type, n_value is
    /// the address of the symbol – offset from the start of the segment.
    /// For N_UNDF | N_EXT it is not used.
    pub value: u64,
}

// #[derive(Debug, PartialEq)]
// /// A symbol in a Mach-O file.
// pub enum Symbol {
//     Null,

//     /// A symbol defined in a section of the Mach-O file (`N_SECT`).
//     Defined {
//         /// Symbol name (including leading underscore if present, e.g. `_main`).
//         name: String,

//         /// Binding / scope of the symbol (`Local`, `Global`, or `Weak`).
//         bind: SymbolBind,

//         /// Symbol type.
//         symbol_type: SymbolType,

//         /// 1-based section index in the Mach-O file (`n_sect`).
//         section_index: usize,

//         /// Symbol address or section-relative offset.
//         value: u64,
//     },

//     /// An absolute symbol (`N_ABS`).
//     Absolute {
//         /// Symbol name.
//         name: String,

//         /// Symbol binding.
//         bind: SymbolBind,

//         /// Absolute value.
//         value: u64,
//     },

//     /// An undefined external symbol (`N_UNDF`).
//     External(String),

//     /// A symbol representing a source file name (`N_SO`).
//     File(String),

//     /// Other symbol types (e.g. debugging stabs).
//     Other,
// }

// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
// /// Symbol binding / visibility scope in Mach-O.
// pub enum SymbolBind {
//     /// Local symbol.
//     Local,
//     /// Global / external symbol (`N_EXT`).
//     Global,
//     /// Weak definition or reference (`N_WEAK_DEF` / `N_WEAK_REF`).
//     Weak,
//     /// Other binding.
//     Other(u16),
// }

// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
// /// Symbol entity type in Mach-O.
// pub enum SymbolType {
//     /// No specific type.
//     Notype,
//     /// Global data object.
//     Object,
//     /// Function code.
//     Func,
//     /// Section symbol.
//     Section,
//     /// Thread-local symbol.
//     TLS,
//     /// Other symbol type.
//     Other(u8),
// }

pub fn get_load_address_base(cpu_type: CpuType) -> u64 {
    match cpu_type {
        // Typical base address for macOS 64-bit executables (above __PAGEZERO segment of 4GB)
        CpuType::ARM64 => 0x100000000,
        _ => unimplemented!("Unsupported CPU architecture: {}", cpu_type),
    }
}

pub fn get_segment_align_page_size(cpu_type: CpuType) -> u64 {
    match cpu_type {
        // Apple Silicon macOS uses 16KB pages
        CpuType::ARM64 => PAGE_SIZE_ARM64,
        _ => unimplemented!("Unsupported CPU architecture: {}", cpu_type),
    }
}

pub fn get_section_align_text(cpu_type: CpuType) -> u64 {
    match cpu_type {
        CpuType::ARM64 => SECTION_ALIGN_TEXT,
        _ => unimplemented!("Unsupported CPU architecture: {}", cpu_type),
    }
}
