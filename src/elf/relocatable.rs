// Copyright (c) 2026 Hemashushu <hippospark@gmail.com>, All rights reserved.
//
// This Source Code Form is subject to the terms of
// the Mozilla Public License version 2.0 and additional exceptions.
// For more details, see the LICENSE, LICENSE.additional, and CONTRIBUTING files.

use object::read::elf::FileHeader;

use crate::{
    elf::module::{Relocation, SymbolBind},
    error::LinkerError,
};

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
pub struct RelocatableModule {
    pub section_size: SectionSize,
    pub section_index_table: SectionIndexTable,

    /// The symbol table of the module, which contains the symbols defined in the module.
    ///
    /// This list is translated from the symbol table in the object file directly,
    /// and the first symbol is always the null symbol, which is a special symbol
    /// that represents the absence of a symbol.
    pub symbols: Vec<ResolveSymbol>,

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

    pub section_binary: SectionBinary,

    /// The section offsets in the final executable, which are calculated during the linking process.
    pub section_offsets: SectionOffset,

    /// The section virtual addresses in the final executable, which are calculated during the linking process.
    pub section_virtual_addresses: SectionVirtualAddress,
}

impl RelocatableModule {
    pub fn new() -> Self {
        Self {
            section_size: SectionSize::default(),
            section_index_table: SectionIndexTable::default(),
            symbols: Vec::new(),
            relocations_text: Vec::new(),
            relocations_rodata: Vec::new(),
            relocations_data: Vec::new(),
            relocations_tdata: Vec::new(),
            section_binary: SectionBinary::default(),
            section_offsets: SectionOffset::default(),
            section_virtual_addresses: SectionVirtualAddress::default(),
        }
    }

    pub fn has_tls(&self) -> bool {
        !self.section_binary.tdata.is_empty() || self.section_size.tbss > 0
    }
}

#[derive(Debug, Default)]
pub struct SectionSize {
    pub text: usize,
    pub rodata: usize,
    pub tdata: usize,
    pub tbss: usize, // this is the memory size of the .tbss section
    pub data: usize,
    pub bss: usize, // this is the memory size of the .bss section
}

/// Sections that a symbol can belong to.
#[derive(Debug, Default)]
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

// impl SectionBinary {
//     pub fn new() -> Self {
//         Self {
//             text: Vec::new(),
//             rodata: Vec::new(),
//             tdata: Vec::new(),
//             data: Vec::new(),
//         }
//     }
// }

/// The index of sections
///
/// This table is used to determine the section where a symbol
/// is defined based on the `st_shndx` field in the symbol table.
/// This table does not hold complete sections, but only the sesstions
/// which can define symbols (e.g. `.text`, `.rodata`, `.tdata`,
/// `.tbss`, `.data`, and `.bss`).
#[derive(Debug, Default)]
pub struct SectionIndexTable {
    pub code: usize,
    pub rodata: usize,
    pub tdata: usize,
    pub tbss: usize,
    pub data: usize,
    pub bss: usize,
}

impl SectionIndexTable {
    pub fn get_symbol_section_type(&self, index: usize) -> Option<SymbolSectionType> {
        if index == self.code {
            Some(SymbolSectionType::Text)
        } else if index == self.rodata {
            Some(SymbolSectionType::RoData)
        } else if index == self.tdata {
            Some(SymbolSectionType::TData)
        } else if index == self.tbss {
            Some(SymbolSectionType::TBss)
        } else if index == self.data {
            Some(SymbolSectionType::Data)
        } else if index == self.bss {
            Some(SymbolSectionType::Bss)
        } else {
            None
        }
    }
}

/// Sections that a symbol can belong to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolSectionType {
    Text,   // `.text` section
    RoData, // `.rodata` section
    TData,  // `.tdata` section
    TBss,   // `.tbss` section
    Data,   // `.data` section
    Bss,    // `.bss` section
}

#[derive(Debug)]
pub enum ResolveSymbol {
    Defined {
        // The name of the symbol
        // This name may be empty for symbols that represent sections (e.g. the symbol for the `.text` section).
        name: String,
        symbol_section_type: SymbolSectionType,
        scope: SymbolBind,

        // The offset of the symbol in its original section in the object file.
        offset_original: usize,

        offset_in_merged_section: usize, // calculated during linking, used for relocation
        virtual_address_in_merged_section: usize, // calculated during linking, used for relocation
    },
    External(/* name */ String),

    // Symbols that the linker does not care about.
    Other,
}

// impl ResolveSymbol {
//     pub fn new_defined(
//         name: &str,
//         symbol_section_type: SymbolSectionType,
//         scope: SymbolScope,
//         offset_original: usize,
//     ) -> Self {
//         Self::Defined {
//             name: name.to_string(),
//             symbol_section_type,
//             scope,
//             offset_original,
//             offset_in_merged_section: 0,
//             virtual_address_in_merged_section: 0,
//         }
//     }

//     pub fn new_external(name: &str) -> Self {
//         Self::External(name.to_string())
//     }

//     pub fn new_other() -> Self {
//         Self::Other
//     }
// }

#[derive(Debug, Default)]
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

// impl SectionOffset {
//     pub fn new() -> Self {
//         Self {
//             text: 0,
//             rodata: 0,
//             tdata: 0,
//             tbss: 0,
//             data: 0,
//             bss: 0,
//         }
//     }
// }

/// The virtual addresses of the sections in the final executable,
/// which are calculated during the linking process based on the section offsets and the load address.
///
/// For most sections, `virtual address = load address + section offset`,
/// but start from the `.data` section, the virtual address is also affected by the
/// size of the previous section `.bss` (which is not present in the file, but occupies space in memory).
#[derive(Debug, Default)]
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

// impl SectionVirtualAddress {
//     pub fn new() -> Self {
//         Self {
//             text: 0,
//             rodata: 0,
//             tdata: 0,
//             tbss: 0,
//             data: 0,
//             bss: 0,
//         }
//     }
// }

const SECTION_NAME_TEXT: &str = ".text";
const SECTION_NAME_RODATA: &str = ".rodata";

const SECTION_NAME_TDATA: &str = ".tdata";
const SECTION_NAME_TBSS: &str = ".tbss";
const SECTION_NAME_DATA: &str = ".data";
const SECTION_NAME_BSS: &str = ".bss";

const SECTION_NAME_SYMTAB: &str = ".symtab";
const SECTION_NAME_RELA_TEXT: &str = ".rela.text";
const SECTION_NAME_RELA_RODATA: &str = ".rela.rodata";
const SECTION_NAME_RELA_DATA: &str = ".rela.data";
const SECTION_NAME_RELA_TDATA: &str = ".rela.tdata";

pub fn read_relocatable(binary: &[u8]) -> Result<RelocatableModule, LinkerError> {
    let Ok(elf) = object::elf::FileHeader64::<object::Endianness>::parse(binary) else {
        return Err(LinkerError::new("Failed to parse ELF64 file"));
    };

    let Ok(endian) = elf.endian() else {
        return Err(LinkerError::new("Failed to determine endianness"));
    };

    // -------------------------------------------------------------------------
    // Read file header
    // -------------------------------------------------------------------------

    // ET_*, expect `ET_REL`
    if elf.e_type(endian) != object::elf::ET_REL {
        return Err(LinkerError::new("Unsupported ELF type, expected ET_REL"));
    }

    // EM_*, expect `EM_X86_64`, it determines the relocation types (e.g. R_X86_64_PC32, R_X86_64_PLT32)
    if elf.e_machine(endian) != object::elf::EM_X86_64 {
        return Err(LinkerError::new(
            "Unsupported ELF machine, expected EM_X86_64",
        ));
    }

    // let mut index_table = SectionIndexTable::new();
    let mut module = RelocatableModule::new();

    Ok(module)
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
