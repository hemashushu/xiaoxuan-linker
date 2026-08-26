// Copyright (c) 2026 Hemashushu <hippospark@gmail.com>, All rights reserved.
//
// This Source Code Form is subject to the terms of
// the Mozilla Public License version 2.0 and additional exceptions.
// For more details, see the LICENSE, LICENSE.additional, and CONTRIBUTING files.

use crate::elf::module::{Relocation, SymbolBind};

// The names of the supported sections
pub const SECTION_NAME_TEXT: &str = ".text";
pub const SECTION_NAME_RODATA: &str = ".rodata";
pub const SECTION_NAME_TDATA: &str = ".tdata";
pub const SECTION_NAME_TBSS: &str = ".tbss";
pub const SECTION_NAME_DATA: &str = ".data";
pub const SECTION_NAME_BSS: &str = ".bss";

// The names of the supported relocation sections
pub const SECTION_NAME_RELA_TEXT: &str = ".rela.text";
pub const SECTION_NAME_RELA_RODATA: &str = ".rela.rodata";
pub const SECTION_NAME_RELA_DATA: &str = ".rela.data";
pub const SECTION_NAME_RELA_TDATA: &str = ".rela.tdata";

/// A merged module represents essential elements of an object file,
/// which are intended to be merged into a single executable file.
///
/// The `MergedModule` is part of an object file,
/// and it is not a complete representation of all the details of an object file.
/// It assumes that an object file contains only:
///
/// - At most one code section `.text`
/// - At most one read-only data section `.rodata`
/// - At most one thread local data section `.tdata`
/// - At most one thread local uninitialized section `.tbss`
/// - At most one data section `.data`
/// - At most one uninitialized data section `.bss`
/// - At most one symbol table `.symtab`
/// - At most one relocation table `.rela.text`
/// - At most one relocation table `.rela.rodata`
/// - At most one relocation table `.rela.data`
/// - At most one relocation table `.rela.tdata`
/// - At most one string table `.strtab` (for symbol names)
/// - One section header string table `.shstrtab` (for section names)
///
/// Other sections and details of the object file are ignored without notice.
///
/// Note:
/// GCC in modern Linux distributions (e.g., Ubuntu 22.04) generates PIE (Position Independent Executable) by default,
/// which means that the `.data.rel.local` section is generated instead of the `.data` section,
/// and the `.rela.data.rel.local` section is generated instead of the `.rela.data` section.
/// As well as `.data.rel.ro.local` and `.rela.data.rel.ro.local` sections are generated
/// instead of the `.rodata` and `.rela.rodata` sections.
/// However, the current implementation of the linker does not support PIE, so we need to use a non-PIE object file for testing.
#[derive(Debug, PartialEq)]
pub struct MergedModule<'a> {
    /// The relevant sections of the module
    pub sections: MergedSection<'a>,

    /// The symbol table of the module, which contains the symbols defined in the module.
    pub symbols: Vec<MergedSymbol>,

    /// The relocation entries of the module, which contain the information about
    /// how to adjust the code and data when linking.
    pub relocation_sections: Vec<MergedRelocationSection>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SectionName {
    Text,
    RoData,
    TData,
    TBss,
    Data,
    Bss,
}

#[derive(Debug, PartialEq)]
pub enum MergedSection<'a> {
    Essential(MergedEssentialSection<'a>),
    Other,
}

/// Section represents a section in the merged module
#[derive(Debug, PartialEq)]
pub struct MergedEssentialSection<'a> {
    pub section_name: SectionName,

    /// The size of the section.
    /// For the `.bss` and `.tbss` sections, this is the memory size of the section,
    /// which is not present in the file, but occupies space in memory.
    pub size: usize,

    /// The binary data of the section.
    pub binary: Option<&'a [u8]>,

    /// The section offset in the final executable, which are calculated during the linking process.
    pub offset: usize,

    /// The virtual addresses of the sections in the final executable,
    /// which are calculated during the linking process based on the section offsets and the load address.
    ///
    /// For most sections, `virtual address = load address + section offset`,
    /// but start from the `.data` section, the virtual address is also affected by the
    /// size of the previous section `.bss` (which is not present in the file, but occupies space in memory).
    pub virtual_address: usize,
}

// /// Section binary data
// #[derive(Debug, PartialEq)]
// pub enum SectionBinary<'a> {
//     Reference(&'a [u8]),

//     Owned(Vec<u8>),

//     /// Only `.text`, `.rodata`, `.tdata`, and `.data` sections
//     /// are relevant for linking (merging and relocation).
//     None,
// }

/// Symbol represents a symbol in the merged module
#[derive(Debug, PartialEq)]
pub enum MergedSymbol {
    Defined {
        /// The name of the symbol
        /// This name may be empty for symbols that represent sections
        /// (e.g. the symbol which represents a section).
        name: String,

        /// The binding of the symbol, which determines the linkage of the symbol.
        bind: SymbolBind,

        /// The section that the symbol belongs to.
        section_index: usize,

        /// The offset of the symbol in the merged section in the final executable,
        offset: usize,

        /// The virtual address of the symbol in the merged section in the final executable,
        virtual_address: usize,
    },

    /// The symbol is defined in another module, and the linker needs to resolve it.
    External(/* name */ String),

    /// Symbols that the linker does not care about.
    Other,
}

impl TryFrom<&str> for SectionName {
    type Error = ();

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            SECTION_NAME_TEXT => Ok(SectionName::Text),
            SECTION_NAME_RODATA => Ok(SectionName::RoData),
            SECTION_NAME_TDATA => Ok(SectionName::TData),
            SECTION_NAME_TBSS => Ok(SectionName::TBss),
            SECTION_NAME_DATA => Ok(SectionName::Data),
            SECTION_NAME_BSS => Ok(SectionName::Bss),
            _ => Err(()),
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct MergedRelocationSection {
    /// The section which these relocations apply to.
    ///
    /// Currently, only the following sections are supported:
    /// - Text
    /// - RoData
    /// - Data
    /// - TData
    pub target_section_index: usize,

    /// The relocation entries in this section.
    pub relocations: Vec<Relocation>,
}
