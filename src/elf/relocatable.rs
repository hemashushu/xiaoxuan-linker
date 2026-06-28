// Copyright (c) 2026 Hemashushu <hippospark@gmail.com>, All rights reserved.
//
// This Source Code Form is subject to the terms of
// the Mozilla Public License version 2.0 and additional exceptions.
// For more details, see the LICENSE, LICENSE.additional, and CONTRIBUTING files.

//! The `RelocatableModule` is a simplified representation of an object file,
//! and it is not intended to be a complete representation of all the details of an object file.
//! It assumes that an object file contains only:
//!
//! - At most one code section `.text`
//! - At most one read-only data section `.rodata`
//! - At most one thread local data section `.tdata`
//! - At most one thread local uninitialized section `.tbss`
//! - At most one data section `.data`
//! - At most one uninitialized data section `.bss`
//! - At most one symbol table `.symtab`
//! - At most one relocation table `.rela.text`
//! - At most one relocation table `.rela.rodata`
//! - At most one relocation table `.rela.data`
//! - At most one relocation table `.rela.tdata`
//! - At most one string table `.strtab` (for symbol names)
//! - One section header string table `.shstrtab` (for section names)
//!
//! Other sections and details of the object file are ignored without notice.

use std::collections::HashMap;

use crate::{
    elf::{
        module::{FileType, Relocation, Symbol, SymbolBind},
        reader::{
            read_file, read_file_header, read_relocation_sections, read_section_headers,
            read_symbols,
        },
    },
    error::LinkerError,
};

// The names of the sections that are relevant for merging.
const SECTION_NAME_TEXT: &str = ".text";
const SECTION_NAME_RODATA: &str = ".rodata";
const SECTION_NAME_TDATA: &str = ".tdata";
const SECTION_NAME_TBSS: &str = ".tbss";
const SECTION_NAME_DATA: &str = ".data";
const SECTION_NAME_BSS: &str = ".bss";

// The names of the relocation sections that are relevant for relocation.
const SECTION_NAME_RELA_TEXT: &str = ".rela.text";
const SECTION_NAME_RELA_RODATA: &str = ".rela.rodata";
const SECTION_NAME_RELA_DATA: &str = ".rela.data";
const SECTION_NAME_RELA_TDATA: &str = ".rela.tdata";

/// A module represents essential elements of an object file,
/// which contains code, data, symbols, and relocation.
///
/// In addition, this module is also used for storing the calculated values
/// during the linking process, such as the section offsets in the merged sections.
#[derive(Debug, PartialEq)]
pub struct RelocatableModule<'a> {
    /// The relevant sections of the module
    pub sections: HashMap<RelocatableSectionType, RelocatableSection<'a>>,

    /// The symbol table of the module, which contains the symbols defined in the module.
    ///
    /// This list is translated from the symbol table in the object file directly,
    /// and the first symbol is always the null symbol, which is a special symbol
    /// that represents the absence of a symbol.
    ///
    /// Note that `Relocation.symbol_index` is the index of the symbol in this list.
    pub symbols: Vec<RelocatableSymbol>,

    /// The relocation entries of the module, which contain the information about
    /// how to adjust the code and data when linking.
    pub relocations: HashMap<RelocationEntrySectionType, Vec<Relocation>>,
}

impl<'a> RelocatableModule<'a> {
    pub fn has_read_only_data(&self) -> bool {
        let existing_rodata = matches!(self.sections.get(&RelocatableSectionType::RoData),
        Some(section) if section.size > 0);

        existing_rodata
    }

    pub fn has_writable_data(&self) -> bool {
        let existing_data = matches!(self.sections.get(&RelocatableSectionType::Data),
        Some(section) if section.size > 0);

        let existing_bss = matches!(self.sections.get(&RelocatableSectionType::Bss),
        Some(section) if section.size > 0);

        existing_data || existing_bss || self.has_tls()
    }

    pub fn has_tls(&self) -> bool {
        let existing_tdata = matches!(self.sections.get(&RelocatableSectionType::TData),
        Some(section) if section.size > 0);

        let existing_tbss = matches!(self.sections.get(&RelocatableSectionType::TBss),
        Some(section) if section.size > 0);

        existing_tdata || existing_tbss
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RelocatableSectionType {
    Text,
    RoData,
    TData,
    TBss,
    Data,
    Bss,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RelocationEntrySectionType {
    Text,
    RoData,
    Data,
    TData,
}

#[derive(Debug, PartialEq)]
pub struct RelocatableSection<'a> {
    /// The index of the section in the section header table of the object file.
    pub index: usize,

    /// The size of the section.
    /// For the `.bss` and `.tbss` sections, this is the memory size of the section,
    /// which is not present in the file, but occupies space in memory.
    pub size: usize,

    /// The binary data of the section.
    pub binary: SectionBinary<'a>,

    /// The section offset in the final executable, which are calculated during the linking process.
    pub resolved_offset: usize,

    /// The virtual addresses of the sections in the final executable,
    /// which are calculated during the linking process based on the section offsets and the load address.
    ///
    /// For most sections, `virtual address = load address + section offset`,
    /// but start from the `.data` section, the virtual address is also affected by the
    /// size of the previous section `.bss` (which is not present in the file, but occupies space in memory).
    pub resolved_virtual_address: usize,
}

/// Section binary data
#[derive(Debug, PartialEq)]
pub enum SectionBinary<'a> {
    Reference(&'a [u8]),

    Owned(Vec<u8>),

    /// Only `.text`, `.rodata`, `.tdata`, and `.data` sections
    /// are relevant for linking (merging and relocation).
    None,
}

#[derive(Debug, PartialEq)]
pub enum RelocatableSymbol {
    Defined {
        /// The name of the symbol
        /// This name may be empty for symbols that represent sections
        /// (e.g. the symbol which represents a section).
        name: String,

        /// The binding of the symbol, which determines the linkage of the symbol.
        bind: SymbolBind,

        /// The section that the symbol belongs to.
        section_type: RelocatableSectionType,

        /// The offset of the symbol in its original section in the object file.
        offset: usize,

        /// The offset of the symbol in the merged section in the final executable,
        resolved_offset: usize,

        /// The virtual address of the symbol in the merged section in the final executable,
        resolved_virtual_address: usize,
    },
    External(/* name */ String),

    // Symbols that the linker does not care about.
    Other,
}

/// This function is used to determine the section where a symbol
/// is defined based on the `st_shndx` field of the symbol.
fn get_section_type(
    sections: &HashMap<RelocatableSectionType, RelocatableSection>,
    section_index: usize,
) -> Option<RelocatableSectionType> {
    sections
        .iter()
        .find(|(_, section)| section.index == section_index)
        .map(|(section_type, _)| *section_type)
}

pub fn read_relocatable<'a>(binary: &'a [u8]) -> Result<RelocatableModule<'a>, LinkerError> {
    let elf = read_file(binary)?;
    let file_header = read_file_header(elf)?;

    if file_header.file_type != FileType::Relocatable {
        return Err(LinkerError::new(
            "Unsupported ELF type, expected relocatable (ET_REL) file",
        ));
    }

    let mut relocatable_sections: HashMap<RelocatableSectionType, RelocatableSection<'a>> =
        HashMap::new();
    let section_headers = read_section_headers(elf, binary)?;

    // Iterate over the section headers and read the sections that are relevant for linking.
    for (section_index, section_header) in section_headers.iter().enumerate() {
        match section_header.name.as_str() {
            SECTION_NAME_TEXT | SECTION_NAME_RODATA | SECTION_NAME_TDATA | SECTION_NAME_DATA => {
                // Read the `.text`, `.rodata`, `.tdata`, and `.data` sections
                // These sections contain the actual code and data of the module.
                let relocatable_section = RelocatableSection {
                    index: section_index,
                    size: section_header.size,
                    binary: SectionBinary::Reference(section_header.binary),
                    resolved_offset: 0,
                    resolved_virtual_address: 0,
                };
                let relocatable_section_type = match section_header.name.as_str() {
                    SECTION_NAME_TEXT => RelocatableSectionType::Text,
                    SECTION_NAME_RODATA => RelocatableSectionType::RoData,
                    SECTION_NAME_TDATA => RelocatableSectionType::TData,
                    SECTION_NAME_DATA => RelocatableSectionType::Data,
                    _ => unreachable!(),
                };
                relocatable_sections.insert(relocatable_section_type, relocatable_section);
            }
            SECTION_NAME_TBSS | SECTION_NAME_BSS => {
                // Read the `.tbss` and `.bss` sections
                let relocatable_section = RelocatableSection {
                    index: section_index,
                    size: section_header.size,
                    binary: SectionBinary::None,
                    resolved_offset: 0,
                    resolved_virtual_address: 0,
                };
                let relocatable_section_type = match section_header.name.as_str() {
                    SECTION_NAME_TBSS => RelocatableSectionType::TBss,
                    SECTION_NAME_BSS => RelocatableSectionType::Bss,
                    _ => unreachable!(),
                };
                relocatable_sections.insert(relocatable_section_type, relocatable_section);
            }
            _ => {
                // Ignore other sections
            }
        }
    }

    let mut relocatable_symbols: Vec<RelocatableSymbol> = Vec::new();
    let symbols = read_symbols(elf, binary)?;

    // Translate the symbols from the object file to the `RelocatableSymbol` representation.
    for symbol in symbols {
        let relocatable_symbol = match symbol {
            Symbol::Defined {
                name,
                bind,
                symbol_type: _,
                section_index,
                offset,
            } => {
                let section_type_opt = get_section_type(&relocatable_sections, section_index);
                let Some(section_type) = section_type_opt else {
                    return Err(LinkerError::new(&format!(
                        "Symbol '{}' is defined in an unsupported section, section index {}",
                        name, section_index
                    )));
                };
                RelocatableSymbol::Defined {
                    name,
                    section_type,
                    bind,
                    offset,
                    resolved_offset: 0,
                    resolved_virtual_address: 0,
                }
            }
            Symbol::External(name) => RelocatableSymbol::External(name),
            Symbol::Other => RelocatableSymbol::Other,
        };
        relocatable_symbols.push(relocatable_symbol);
    }

    let mut relocatable_relocations: HashMap<RelocationEntrySectionType, Vec<Relocation>> =
        HashMap::new();
    let relocation_sections = read_relocation_sections(elf, binary)?;

    // Translate the relocation sections from the object file to HashMap.
    for relocation_section in relocation_sections {
        let section_type = match relocation_section.name.as_str() {
            SECTION_NAME_RELA_TEXT => RelocationEntrySectionType::Text,
            SECTION_NAME_RELA_RODATA => RelocationEntrySectionType::RoData,
            SECTION_NAME_RELA_DATA => RelocationEntrySectionType::Data,
            SECTION_NAME_RELA_TDATA => RelocationEntrySectionType::TData,
            _ => {
                return Err(LinkerError::new(&format!(
                    "Unsupported relocation section '{}'",
                    relocation_section.name
                )));
            }
        };
        relocatable_relocations.insert(section_type, relocation_section.relocations);
    }

    let relocatable_module = RelocatableModule {
        sections: relocatable_sections,
        symbols: relocatable_symbols,
        relocations: relocatable_relocations,
    };
    Ok(relocatable_module)
}

#[cfg(test)]
mod tests {
    use crate::elf::relocatable::read_relocatable;

    fn get_example_file_binary(file_name: &str) -> Vec<u8> {
        let file_path = std::env::current_dir()
            .unwrap()
            .join("resources/examples/x86_64-linux")
            .join(file_name);

        std::fs::read(file_path).unwrap()
    }

    #[test]
    fn test_read_minimal() {
        let binary = get_example_file_binary("minimal.o");
        let module = read_relocatable(&binary).unwrap();
        println!("{:#?}", module);
    }

    #[test]
    fn test_read_function() {
        let binary = get_example_file_binary("function.o");
        let module = read_relocatable(&binary).unwrap();
        println!("{:#?}", module);
    }

    #[test]
    fn test_read_data() {
        let binary = get_example_file_binary("data.o");
        let module = read_relocatable(&binary).unwrap();
        println!("{:#?}", module);
    }

    #[test]
    fn test_read_symbol_export() {
        let binary = get_example_file_binary("symbol-export.o");
        let module = read_relocatable(&binary).unwrap();
        println!("{:#?}", module);
    }

    #[test]
    fn test_read_symbol_import() {
        let binary = get_example_file_binary("symbol-import.o");
        let module = read_relocatable(&binary).unwrap();
        println!("{:#?}", module);
    }

    #[test]
    fn test_read_override_weak() {
        let binary = get_example_file_binary("override-weak.o");
        let module = read_relocatable(&binary).unwrap();
        println!("{:#?}", module);
    }

    #[test]
    fn test_read_override_strong() {
        let binary = get_example_file_binary("override-strong.o");
        let module = read_relocatable(&binary).unwrap();
        println!("{:#?}", module);
    }

    #[test]
    fn test_read_relocate_within_data() {
        let binary = get_example_file_binary("relocate-within-data.o");
        let module = read_relocatable(&binary).unwrap();
        println!("{:#?}", module);
    }
}
