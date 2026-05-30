// Copyright (c) 2026 Hemashushu <hippospark@gmail.com>, All rights reserved.
//
// This Source Code Form is subject to the terms of
// the Mozilla Public License version 2.0 and additional exceptions.
// For more details, see the LICENSE, LICENSE.additional, and CONTRIBUTING files.

use std::collections::HashMap;

use crate::{
    elf::{
        consts::{
            DATA_ALIGN, ELF_HEADER_SIZE, LOAD_ADDR_BASE, PAGE_SIZE, PROGRAM_HEADER_ENTRY_SIZE,
            TEXT_ALIGN,
        },
        module::{Module, RelocationType, Symbol, SymbolScope, SymbolSection},
    },
    error::LinkerError,
};

#[derive(Debug, Clone, PartialEq)]
pub struct LinkResult {
    /// Indicates whether the final executable contains TLS segments.
    pub existing_tls: bool,

    /// The number of program headers in the final executable.
    ///
    /// There are `PHDR, metadata, code, read-only data, writable data` five program headers,
    /// and an additional TLS segment if there is TLS.
    pub number_of_program_headers: usize,

    /// The virtual address of the entry point (the `_start` symbol) in the final executable.
    pub entry_point: usize,

    pub section_size: SectionSize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SectionSize {
    pub text: usize,
    pub rodata: usize,
    pub tdata: usize,
    pub tbss: usize,
    pub data: usize,
    pub bss: usize,
}

/// Link multiple relocatable modules into a single executable module.
///
/// This process involves merging sections, resolving symbols, and applying relocations.
///
/// THe parameter `modules` need to be mutable because we will patch all relocations in-place.
pub fn link(modules: &mut [Module]) -> Result<LinkResult, LinkerError> {
    let existing_tls = modules.iter().any(|m| m.has_tls());

    let number_of_program_headers = if existing_tls {
        6 // PHDR, metadata, code, read-only data, writable data, and TLS
    } else {
        5 // PHDR, metadata, code, read-only data, writable data
    };

    if let Some((module_index, symbol_name)) = check_duplicated_strong_global_symbol(modules) {
        return Err(LinkerError::Message(format!(
            "Duplicated strong symbol '{}' found in module {}",
            symbol_name, module_index
        )));
    }

    // ELF file layout overall
    //
    // | Size   | Content         |
    // |--------|-----------------|
    // | 64     | ELF header      |
    // | m * 56 | program headers |
    // | ...    | section data    |
    // | n * 64 | section headers |

    // The first section in file is started from the end of the metadata,
    // which includes the ELF header and the program headers.
    let metadata_size = ELF_HEADER_SIZE + number_of_program_headers * PROGRAM_HEADER_ENTRY_SIZE;

    // Sections (in file order)
    //
    // | Name                       | Type         | Description                     | Align |
    // |----------------------------|--------------|---------------------------------|-------|
    // | NULL                       | SHT_NULL     | Null section header             | 0     |
    // | `.init`, `.text`, `.finit` | SHT_PROGBITS | Executable code                 | 16    |
    // | `.rodata`                  | SHT_PROGBITS | Read-only data (strings)        | 4/8   |
    // | `.tdata`                   | SHT_PROGBITS | Initialized thread-local data   | 4/8   |
    // | `.tbss`                    | SHT_NOBITS   | Uninitialized thread-local data | 4/8   |
    // | `.data`                    | SHT_PROGBITS | Initialized data                | 4/8   |
    // | `.bss`                     | SHT_NOBITS   | Uninitialized data              | 4/8   |
    // | `.symtab`                  | SHT_SYMTAB   | Symbol table                    | 8     |
    // | `.strtab`                  | SHT_STRTAB   | Strings for symbol names        | 1     |
    // | `.shstrtab`                | SHT_STRTAB   | Strings for section names       | 1     |
    //
    // Note that sections such as `.rela.*` are consumed by the linker and would not appear in the final executable.
    //
    // Program headers
    //
    // | Segment           | Sections                       | Type    | Flags | Alignment |
    // |-------------------|--------------------------------|---------|-------|-----------|
    // | 00 phdr           | program headers                | PT_PHDR | R     | 0x8       |
    // | 01 metadata       | data before first code section | PT_LOAD | R     | 0x1000    |
    // | 02 text           | .init`, .text, .finit          | PT_LOAD | R E   | 0x1000    |
    // | 03 read-only data | .rodata                        | PT_LOAD | R     | 0x1000    |
    // | 04 writable data  | .tdata, .tbss, .data, .bss     | PT_LOAD | R W   | 0x1000    |
    // | 05 tls            | .tdata, .tbss                  | PT_TLS  | R     | 0x8       |

    // merge `.text`
    let mut offset = align_up(metadata_size, PAGE_SIZE); // code segment must be page-aligned
    let mut section_size_text = offset;
    for module in modules.iter_mut() {
        offset = align_up(offset, TEXT_ALIGN);
        module.section_offsets.text = offset;
        module.section_virtual_addresses.text = LOAD_ADDR_BASE + offset;
        offset += module.section_binary.text.len();
    }
    section_size_text = offset - section_size_text;

    // merge `.rodata`
    offset = align_up(offset, PAGE_SIZE); // read-only segment must be page-aligned
    let mut section_size_rodata = offset;
    for module in modules.iter_mut() {
        offset = align_up(offset, DATA_ALIGN);
        module.section_offsets.rodata = offset;
        module.section_virtual_addresses.rodata = LOAD_ADDR_BASE + offset;
        offset += module.section_binary.rodata.len();
    }
    section_size_rodata = offset - section_size_rodata;

    // merge `.tdata`
    // Note that the `.tdata`, `.tbss`, `.data`, and `.bss` sections will be merged into
    // the same writable segment, so we need to calculate their offsets and virtual addresses together.
    offset = align_up(offset, PAGE_SIZE); // writable segment must be page-aligned
    let mut section_size_tdata = offset;
    for module in modules.iter_mut() {
        offset = align_up(offset, DATA_ALIGN);
        module.section_offsets.tdata = offset;
        module.section_virtual_addresses.tdata = LOAD_ADDR_BASE + offset;
        offset += module.section_binary.tdata.len();
    }
    section_size_tdata = offset - section_size_tdata;

    // merge `.tbss`
    offset = align_up(offset, DATA_ALIGN);
    // Introduce a new variable `virtual_address` to keep track of the virtual address for `.tbss` section,
    // because `.tbss` is a NOBITS section and does not occupy space in the file,
    // so we cannot simply use `offset` to calculate the virtual address for `.tbss`.
    let mut virtual_address = LOAD_ADDR_BASE + offset;
    let mut section_size_tbss = virtual_address;
    for module in modules.iter_mut() {
        virtual_address = align_up(virtual_address, DATA_ALIGN);
        module.section_offsets.tbss = offset;
        module.section_virtual_addresses.tbss = virtual_address;
        virtual_address += module.tbss_size;
    }
    section_size_tbss = virtual_address - section_size_tbss;

    // merge `.data`
    let mut section_size_data = offset;
    for module in modules.iter_mut() {
        offset = align_up(offset, DATA_ALIGN);
        virtual_address = align_up(virtual_address, DATA_ALIGN);
        module.section_offsets.data = offset;
        module.section_virtual_addresses.data = virtual_address;
        offset += module.section_binary.data.len();
        virtual_address += module.section_binary.data.len();
    }
    section_size_data = offset - section_size_data;

    // The symbol `edata` points to the end of the initialized data segment.
    let symbol_edata_vaddr = virtual_address;
    let symbol_edata_offset = offset;

    // merge `.bss`
    offset = align_up(offset, DATA_ALIGN);
    virtual_address = align_up(virtual_address, DATA_ALIGN);

    let symbol_bss_start_vaddr = virtual_address;
    let symbol_bss_start_offset = offset;

    let mut section_size_bss = virtual_address;
    for module in modules.iter_mut() {
        virtual_address = align_up(virtual_address, DATA_ALIGN);
        module.section_offsets.bss = offset;
        module.section_virtual_addresses.bss = virtual_address;
        virtual_address += module.bss_size;
    }
    section_size_bss = virtual_address - section_size_bss;

    let symbol_end_vaddr = virtual_address;
    let symbol_end_offset = offset;

    // Generate linker-generated symbols for the final executable.
    let mut linker_generated_symbols: HashMap<String, FindSymbolResult> = HashMap::new();
    linker_generated_symbols.insert(
        "_edata".to_string(),
        FindSymbolResult {
            symbol_section: SymbolSection::Bss,
            offset_in_merged_section: symbol_edata_offset,
            virtual_address_in_merged_section: symbol_edata_vaddr,
        },
    );
    linker_generated_symbols.insert(
        "__bss_start".to_string(),
        FindSymbolResult {
            symbol_section: SymbolSection::Bss,
            offset_in_merged_section: symbol_bss_start_offset,
            virtual_address_in_merged_section: symbol_bss_start_vaddr,
        },
    );
    linker_generated_symbols.insert(
        "_end".to_string(),
        FindSymbolResult {
            symbol_section: SymbolSection::Bss,
            offset_in_merged_section: symbol_end_offset,
            virtual_address_in_merged_section: symbol_end_vaddr,
        },
    );

    // Calculate the offsets and virtual addresses for symbols.
    for module in modules.iter_mut() {
        for symbol in module.symbols.iter_mut() {
            if let Symbol::Defined {
                symbol_section,
                offset_original: offset,
                offset_in_merged_section,
                virtual_address_in_merged_section,
                ..
            } = symbol
            {
                let (section_offset, section_virtual_address) = match symbol_section {
                    SymbolSection::Text => (
                        module.section_offsets.text,
                        module.section_virtual_addresses.text,
                    ),
                    SymbolSection::RoData => (
                        module.section_offsets.rodata,
                        module.section_virtual_addresses.rodata,
                    ),
                    SymbolSection::TData => (
                        module.section_offsets.tdata,
                        module.section_virtual_addresses.tdata,
                    ),
                    SymbolSection::TBss => (
                        module.section_offsets.tbss,
                        module.section_virtual_addresses.tbss,
                    ),
                    SymbolSection::Data => (
                        module.section_offsets.data,
                        module.section_virtual_addresses.data,
                    ),
                    SymbolSection::Bss => (
                        module.section_offsets.bss,
                        module.section_virtual_addresses.bss,
                    ),
                };

                *offset_in_merged_section = *offset + section_offset;
                *virtual_address_in_merged_section = *offset + section_virtual_address;
            }
        }
    }

    // Calculate the values to be patched for all relocations and store them in a list of patch items.
    let mut patch_items = Vec::new();
    for (module_index, module) in modules.iter().enumerate() {
        let relocation_sections = [
            (PatchSection::Text, &module.relocations_text),
            (PatchSection::RoData, &module.relocations_rodata),
            (PatchSection::TData, &module.relocations_tdata),
            (PatchSection::Data, &module.relocations_data),
        ];

        for (patch_section, relocations) in relocation_sections {
            for (relocation_index, relocation) in relocations.iter().enumerate() {
                let relocation_type = relocation.relocation_type;
                let placeholder_offset = relocation.placeholder_offset;
                let addend = relocation.addend;

                let symbol_index = relocation.symbol_index;
                let symbol = &module.symbols[symbol_index];

                let target_symbol = match symbol {
                    Symbol::Defined {
                        name,
                        scope,
                        symbol_section,
                        offset_in_merged_section,
                        virtual_address_in_merged_section,
                        ..
                    } => {
                        let mut target_symbol = FindSymbolResult {
                            symbol_section: *symbol_section,
                            offset_in_merged_section: *offset_in_merged_section,
                            virtual_address_in_merged_section: *virtual_address_in_merged_section,
                        };

                        // If the symbol is weak, we need to check if there is a strong symbol with the same name in other modules.
                        if scope == &SymbolScope::Weak {
                            let name = name.to_string();
                            if let Some(found_symbol) = find_global_symbol(&name, modules, true) {
                                target_symbol = found_symbol;
                            }
                        }

                        target_symbol
                    }
                    Symbol::External(name) => {
                        if let Some(linker_generated_symbol) = linker_generated_symbols.get(name) {
                            linker_generated_symbol.clone()
                        } else if let Some(target_symbol) = find_global_symbol(name, modules, false)
                        {
                            target_symbol
                        } else {
                            return Err(LinkerError::Message(format!(
                                "Undefined symbol '{}' referenced in .text section, module: {}",
                                name, module_index
                            )));
                        }
                    }
                    Symbol::Other => {
                        return Err(LinkerError::Message(format!(
                            "Invalid symbol referenced in .text section, module: {}, relocation index: {}, symbol index: {}",
                            module_index, relocation_index, symbol_index
                        )));
                    }
                };

                let patch_item = match relocation_type {
                    RelocationType::R_X86_64_64 => {
                        // R_X86_64_64: S + A
                        let relocated_value = target_symbol
                            .virtual_address_in_merged_section
                            .wrapping_add(addend as usize);

                        // Patch the relocated value into the code section at the placeholder offset.
                        // Note that the placeholder is usually 8 bytes (for 64-bit relocations), so we need to write 8 bytes.
                        PatchItem {
                            module_index,
                            patch_section,
                            patch_offset: placeholder_offset,
                            patch_value: PatchValue::Value64(relocated_value as u64),
                        }
                    }
                    RelocationType::R_X86_64_32 => {
                        // R_X86_64_32: S + A
                        let relocated_value = target_symbol
                            .virtual_address_in_merged_section
                            .wrapping_add(addend as usize);
                        PatchItem {
                            module_index,
                            patch_section,
                            patch_offset: placeholder_offset,
                            patch_value: PatchValue::Value32(relocated_value as u32),
                        }
                    }
                    RelocationType::R_X86_64_PC32 => {
                        // R_X86_64_PC32: S + A - P
                        let p = module.section_virtual_addresses.text + placeholder_offset;
                        let relocated_value = target_symbol
                            .virtual_address_in_merged_section
                            .wrapping_add(addend as usize)
                            .wrapping_sub(p);
                        PatchItem {
                            module_index,
                            patch_section,
                            patch_offset: placeholder_offset,
                            patch_value: PatchValue::Value32(relocated_value as u32),
                        }
                    }
                    RelocationType::R_X86_64_TPOFF32 => {
                        // R_X86_64_TPOFF32: S + A - TP
                        // The formula for calculating the value to be written at the relocation site is:
                        // TPOFF(sym) = symbol_offset_in_tls_block − tls_block_size
                        let tls_block_size = module.section_virtual_addresses.tbss
                            - module.section_virtual_addresses.tdata
                            + module.tbss_size;
                        let symbol_offset_in_tls_block = target_symbol.offset_in_merged_section;

                        let relocated_value = symbol_offset_in_tls_block
                            .wrapping_add(addend as usize)
                            .wrapping_sub(tls_block_size);
                        PatchItem {
                            module_index,
                            patch_section,
                            patch_offset: placeholder_offset,
                            patch_value: PatchValue::Value32(relocated_value as u32),
                        }
                    }
                };

                patch_items.push(patch_item);
            }
        }
    }

    // Patch relocations in-place for each module.
    for patch_item in patch_items {
        let module = &mut modules[patch_item.module_index];
        let patch_section_data = match patch_item.patch_section {
            PatchSection::Text => &mut module.section_binary.text,
            PatchSection::RoData => &mut module.section_binary.rodata,
            PatchSection::TData => &mut module.section_binary.tdata,
            PatchSection::Data => &mut module.section_binary.data,
        };
        match patch_item.patch_value {
            PatchValue::Value64(value) => {
                patch_section_data[patch_item.patch_offset..patch_item.patch_offset + 8]
                    .copy_from_slice(&value.to_le_bytes());
            }
            PatchValue::Value32(value) => {
                patch_section_data[patch_item.patch_offset..patch_item.patch_offset + 4]
                    .copy_from_slice(&value.to_le_bytes());
            }
        }
    }

    // Find the entry point symbol `_start` and get its virtual address.
    let entry_point = if let Some(entry_symbol) = find_global_symbol("_start", modules, false) {
        entry_symbol.virtual_address_in_merged_section
    } else {
        return Err(LinkerError::Message(
            "Entry point symbol '_start' not found".to_string(),
        ));
    };

    let section_size = SectionSize {
        text: section_size_text,
        rodata: section_size_rodata,
        tdata: section_size_tdata,
        tbss: section_size_tbss,
        data: section_size_data,
        bss: section_size_bss,
    };

    let link_result = LinkResult {
        existing_tls,
        number_of_program_headers,
        entry_point,
        section_size,
    };

    Ok(link_result)
}

#[derive(Debug, Clone, PartialEq)]
struct PatchItem {
    module_index: usize,
    patch_section: PatchSection,
    patch_offset: usize,
    patch_value: PatchValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PatchValue {
    Value64(u64),
    Value32(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PatchSection {
    Text,
    RoData,
    TData,
    Data,
}

fn align_up(val: usize, align: usize) -> usize {
    (val + align - 1) & !(align - 1)
}

/// Check duplicated "strong" global symbols across modules.
fn check_duplicated_strong_global_symbol(
    modules: &[Module],
) -> Option<(/* module_index */ usize, /* symbol_name */ String)> {
    let mut symbol_map: HashMap<String, (/* module_index */ usize, /* is_strong */ bool)> =
        HashMap::new();
    for (module_index, module) in modules.iter().enumerate() {
        for symbol in &module.symbols {
            if let Symbol::Defined { name, scope, .. } = symbol {
                if scope == &SymbolScope::Local {
                    continue; // Local symbols are not visible across modules, so we can skip them.
                }

                let is_strong = scope == &SymbolScope::Global;
                if let Some((existing_module_index, existing_is_strong)) = symbol_map.get_mut(name)
                {
                    if is_strong {
                        if *existing_is_strong {
                            return Some((*existing_module_index, name.clone()));
                        } else {
                            // The new strong symbol overrides the existing weak symbol.
                            *existing_module_index = module_index;
                            *existing_is_strong = true;
                        }
                    }
                } else {
                    symbol_map.insert(name.clone(), (module_index, is_strong));
                }
            }
        }
    }
    None
}

#[derive(Debug, Clone, PartialEq)]
struct FindSymbolResult {
    symbol_section: SymbolSection,
    offset_in_merged_section: usize,
    virtual_address_in_merged_section: usize,
}

fn find_global_symbol(
    symbol_name: &str,
    modules: &[Module],
    strong_only: bool,
) -> Option<FindSymbolResult> {
    let mut found_symbol: Option<(FindSymbolResult, /* is_strong */ bool)> = None;

    for module in modules {
        for symbol in &module.symbols {
            if let Symbol::Defined {
                name,
                scope,
                symbol_section,
                offset_in_merged_section: offset,
                virtual_address_in_merged_section: vaddr,
                ..
            } = symbol
                && name == symbol_name
                && scope != &SymbolScope::Local
            {
                if let Some((_, existing_is_strong)) = found_symbol {
                    if scope == &SymbolScope::Global {
                        if existing_is_strong {
                            // We have found a duplicated strong symbol, which is an error.
                            // but we have already checked duplicated strong symbols in the
                            // `check_duplicated_strong_global_symbol` function, so we can just
                            // assume that this case won't happen here.
                            unreachable!("Duplicated strong global symbol '{}' found", symbol_name);
                        } else {
                            // The new strong symbol overrides the existing weak symbol.
                            found_symbol = Some((
                                FindSymbolResult {
                                    symbol_section: *symbol_section,
                                    offset_in_merged_section: *offset,
                                    virtual_address_in_merged_section: *vaddr,
                                },
                                true,
                            ));
                        }
                    }
                } else {
                    found_symbol = Some((
                        FindSymbolResult {
                            symbol_section: *symbol_section,
                            offset_in_merged_section: *offset,
                            virtual_address_in_merged_section: *vaddr,
                        },
                        scope == &SymbolScope::Global,
                    ));
                }
            }
        }
    }

    if let Some((result, is_strong)) = found_symbol {
        if strong_only && !is_strong {
            None
        } else {
            Some(result)
        }
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::elf::{linker::link, relocatable_reader::read_relocatable};

    fn get_example_file_binary(file_name: &str) -> Vec<u8> {
        let file_path = std::env::current_dir()
            .unwrap()
            .join("resources/examples/x86_64-linux")
            .join(file_name);

        fs::read(file_path).unwrap()
    }

    #[test]
    fn test_link() {
        let binary_vec = get_example_file_binary("hello-world.o");
        let binary = binary_vec.as_slice();

        let module = read_relocatable(binary).unwrap();
        let mut modules = vec![module];
        let result = link(&mut modules).unwrap();

        println!("Module after linking: {:#?}", modules);
        println!("Link result: {:?}", result);
    }
}
