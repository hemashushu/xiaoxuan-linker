// Copyright (c) 2026 Hemashushu <hippospark@gmail.com>, All rights reserved.
//
// This Source Code Form is subject to the terms of
// the Mozilla Public License version 2.0 and additional exceptions.
// For more details, see the LICENSE, LICENSE.additional, and CONTRIBUTING files.

/// A module represents essential elements of an object file,
/// which contains code, data, symbols, and relocation.
///
/// In addition, this module is also used for storing the calculated values
/// during the linking process, such as the section offsets in the final executable.
///
/// This module is a simplified representation of an object file,
/// and it is not intended to be a complete representation of all the details of an object file.
/// It assumes that an object file contains only:
///
/// - At most one code section `.text`
/// - At most one read-only data section `.rodata`
/// - At most one data section `.data`/`.tdata`
/// - At most one bss section `.bss`/`.tbss`
/// - At most one symbol table `.symtab`
/// - At most one relocation table `.rela.text`/`.rela.rodata`/`.rela.data`/`.rela.tdata`
/// - At most one string table `.strtab` (for symbol names)
/// - One section header table `.shstrtab` (for section names)
///
/// Other sections and details of the object file are ignored without notice.
#[derive(Debug)]
pub struct Module {
    pub section_binary: SectionBinary,

    /// The thread-local uninitialized data section of the module, which contains uninitialized thread-local variables.
    pub tbss_size: usize,

    /// The bss section of the module, which contains uninitialized data.
    pub bss_size: usize,

    /// The symbol table of the module, which contains the symbols defined in the module.
    ///
    /// This list is translated from the symbol table in the object file directly,
    /// and the first symbol is always the null symbol, which is a special symbol
    /// that represents the absence of a symbol.
    pub symbols: Vec<Symbol>,

    /// The relocation entries of the module, which contain the information about how to adjust the code and data when linking.
    ///
    /// This list is translated from the relocation table in the object file directly,
    /// and the `Relocation.symbol_index` field refers to the index of the symbol in the `symbols` list.
    pub relocations_text: Vec<Relocation>,

    /// The relocation entries for the read-only data section.
    ///
    /// Relocations on the data sections (`.rodata`, `.data`, `.tdata`) are
    /// usually the pointers to symbols (other data or functions).
    pub relocations_rodata: Vec<Relocation>,

    /// The relocation entries for the data section.
    pub relocations_data: Vec<Relocation>,

    /// The relocation entries for the thread-local data section.
    pub relocations_tdata: Vec<Relocation>,

    /// The section offsets in the final executable, which are calculated during the linking process.
    pub section_offsets: SectionOffset,

    /// The section virtual addresses in the final executable, which are calculated during the linking process.
    pub section_virtual_addresses: SectionVirtualAddress,
}

impl Module {
    pub fn new() -> Self {
        Self {
            section_binary: SectionBinary::new(),
            tbss_size: 0,
            bss_size: 0,
            symbols: Vec::new(),
            relocations_text: Vec::new(),
            relocations_rodata: Vec::new(),
            relocations_data: Vec::new(),
            relocations_tdata: Vec::new(),
            section_offsets: SectionOffset::new(),
            section_virtual_addresses: SectionVirtualAddress::new(),
        }
    }

    pub fn has_tls(&self) -> bool {
        !self.section_binary.tdata.is_empty() || self.tbss_size > 0
    }
}

/// Sections that a symbol can belong to.
#[derive(Debug, Clone, PartialEq)]
pub struct SectionBinary {
    /// The code section of the module, which contains the machine code instructions.
    pub text: Vec<u8>,
    /// The read-only data section of the module, which contains constants and other immutable data.
    pub rodata: Vec<u8>,
    /// The thread-local data section of the module, which contains initialized thread-local variables.
    pub tdata: Vec<u8>,
    /// The data section of the module, which contains initialized data.
    pub data: Vec<u8>,
}

impl SectionBinary {
    pub fn new() -> Self {
        Self {
            text: Vec::new(),
            rodata: Vec::new(),
            tdata: Vec::new(),
            data: Vec::new(),
        }
    }
}

/// Sections that a symbol can belong to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolSection {
    Text,   // `.text` section
    RoData, // `.rodata` section
    TData,  // `.tdata` section
    TBss,   // `.tbss` section
    Data,   // `.data` section
    Bss,    // `.bss` section
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolScope {
    Local,
    Global,
    Weak,
}

#[derive(Debug)]
pub enum Symbol {
    Defined {
        // The name of the symbol
        // This name may be empty for symbols that represent sections (e.g. the symbol for the `.text` section).
        name: String,
        symbol_section: SymbolSection,
        scope: SymbolScope,

        // The offset of the symbol in its original section in the object file.
        offset_original: usize,

        offset_in_merged_section: usize, // calculated during linking, used for relocation
        virtual_address_in_merged_section: usize, // calculated during linking, used for relocation
    },
    External(/* name */ String),

    // Symbols that the linker does not care about.
    Other,
}

impl Symbol {
    pub fn new_defined(
        name: &str,
        symbol_section: SymbolSection,
        scope: SymbolScope,
        offset_original: usize,
    ) -> Self {
        Self::Defined {
            name: name.to_string(),
            symbol_section,
            scope,
            offset_original,
            offset_in_merged_section: 0,
            virtual_address_in_merged_section: 0,
        }
    }

    pub fn new_external(name: &str) -> Self {
        Self::External(name.to_string())
    }

    pub fn new_other() -> Self {
        Self::Other
    }
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

#[derive(Debug)]
pub struct Relocation {
    pub relocation_type: RelocationType,
    pub placeholder_offset: usize,
    pub symbol_index: usize,
    pub addend: isize,
}

impl Relocation {
    pub fn new(
        relocation_type: RelocationType,
        placeholder_offset: usize,
        symbol_index: usize,
        addend: isize,
    ) -> Self {
        Self {
            relocation_type,
            placeholder_offset,
            symbol_index,
            addend,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SectionOffset {
    /// The offset of the code section in the final executable,
    /// which is calculated during the linking process.
    pub text: usize,

    /// The offset of the read-only data section in the final executable.
    pub rodata: usize,

    /// The offset of the thread-local data section in the final executable.
    pub tdata: usize,

    /// The offset of the thread-local uninitialized data section in the final executable.
    pub tbss: usize,

    /// The offset of the data section in the final executable.
    pub data: usize,

    /// The offset of the bss section in the final executable.
    pub bss: usize,
}

impl SectionOffset {
    pub fn new() -> Self {
        Self {
            text: 0,
            rodata: 0,
            tdata: 0,
            tbss: 0,
            data: 0,
            bss: 0,
        }
    }
}

/// The virtual addresses of the sections in the final executable,
/// which are calculated during the linking process based on the section offsets and the load address.
///
/// For most sections, `virtual address = load address + section offset`,
/// but start from the `.data` section, the virtual address is also affected by the
/// size of the previous section `.bss` (which is not present in the file, but occupies space in memory).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SectionVirtualAddress {
    /// The virtual address of the code section in the final executable.
    pub text: usize,

    /// The virtual address of the read-only data section in the final executable.
    pub rodata: usize,

    /// The virtual address of the thread-local data section in the final executable.
    pub tdata: usize,

    /// The virtual address of the thread-local uninitialized data section in the final executable.
    pub tbss: usize,

    /// The virtual address of the data section in the final executable.
    pub data: usize,

    /// The virtual address of the bss section in the final executable.
    pub bss: usize,
}

impl SectionVirtualAddress {
    pub fn new() -> Self {
        Self {
            text: 0,
            rodata: 0,
            tdata: 0,
            tbss: 0,
            data: 0,
            bss: 0,
        }
    }
}
