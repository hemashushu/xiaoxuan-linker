// Copyright (c) 2026 Hemashushu <hippospark@gmail.com>, All rights reserved.
//
// This Source Code Form is subject to the terms of
// the Mozilla Public License version 2.0 and additional exceptions.
// For more details, see the LICENSE, LICENSE.additional, and CONTRIBUTING files.

use std::collections::HashMap;

use crate::{
    elf::{
        module::{RelocationType, SymbolBind},
        relocatable::{
            RelocatableModule, RelocatableSectionType, RelocatableSymbol,
            RelocationEntrySectionType, SectionBinary,
        },
    },
    error::LinkerError,
};

// ELF64 header size is fixed at 64 bytes
pub const ELF_HEADER_SIZE: usize = 64;

// ELF64 program header entry size is fixed at 56 bytes
pub const PROGRAM_HEADER_ENTRY_SIZE: usize = 56;

// All executable file has `PHDR`, `metadata`, and `code` segements,
// and optionally `read-only data`, `writable data`, and `TLS` segments.
pub const BASE_PROGRAM_HEADER_COUNT: usize = 3;

// typical base address for x86_64 executables (ET_EXEC),
// by a contrast, PIE/DSO (ET_DYN) usually has a base address of 0.
pub const LOAD_ADDR_BASE: usize = 0x400000;

// Some load segments must be page-aligned in memory:
// - code segment (includes .text and .init/.finit)
// - read-only data segment (includes .rodata)
// - writable data segment (includes .tdata, .tbss, .data and .bss)
pub const PAGE_SIZE: usize = 0x1000;

pub const PHDR_SEGMENT_ALIGN: usize = 0x8;
pub const TLS_SEGMENT_ALIGN: usize = 0x8;

// code sections are usually 16-byte aligned, it is also used for
// merging .text sections from different modules
pub const TEXT_ALIGN: usize = 16;

// .rodata, .data and .bss sections are 8-byte aligned. This is used for
// merging data sections from different modules
pub const DATA_ALIGN: usize = 8;

// The symbol table section is 8-byte aligned
pub const SYMTAB_ALIGN: usize = 8;

#[derive(Debug, Clone, PartialEq)]
pub struct LinkResult {
    pub has_read_only_data: bool,

    pub has_writable_data: bool,

    /// Indicates whether the final executable contains TLS segments.
    pub has_tls: bool,

    /// The number of program headers in the final executable.
    ///
    /// There are `PHDR, metadata, code, read-only data, writable data` five program headers,
    /// and an additional TLS segment if there is TLS.
    pub program_header_count: usize,

    /// The virtual address of the entry point (the `_start` symbol) in the final executable.
    pub entry_point: usize,

    pub merged_section_size: MergedSectionSize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MergedSectionSize {
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
pub fn link(modules: &mut [RelocatableModule]) -> Result<LinkResult, LinkerError> {
    let exported_symbols = get_exported_symbols(modules);
    if let Some((symbol_name, module_indices)) =
        find_duplicated_strong_global_symbol(&exported_symbols)
    {
        return Err(LinkerError::Message(format!(
            "Duplicated exported symbol '{}' found in modules {:?}",
            symbol_name, module_indices
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

    let mut program_header_count = BASE_PROGRAM_HEADER_COUNT;

    let has_read_only_data = modules.iter().any(|m| m.has_read_only_data());
    let has_writable_data = modules.iter().any(|m| m.has_writable_data());
    let has_tls = modules.iter().any(|m| m.has_tls());

    if has_read_only_data {
        program_header_count += 1;
    }

    if has_writable_data {
        program_header_count += 1;
    }

    if has_tls {
        program_header_count += 1;
    }

    // The first section in file is started from the end of the program headers.
    let file_header_and_program_headers_size =
        ELF_HEADER_SIZE + program_header_count * PROGRAM_HEADER_ENTRY_SIZE;

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

    // merging `.text` sections
    let mut offset = align_up(file_header_and_program_headers_size, PAGE_SIZE); // code segment must be page-aligned
    let merged_section_offset_text = offset;
    for module in modules.iter_mut() {
        if let Some(section) = module.sections.get_mut(&RelocatableSectionType::Text) {
            offset = align_up(offset, TEXT_ALIGN);
            section.resolved_offset = offset;
            section.resolved_virtual_address = LOAD_ADDR_BASE + offset;
            offset += section.size;
        }
    }
    let merged_section_size_text = offset - merged_section_offset_text;

    // merging `.rodata` sections
    offset = align_up(offset, PAGE_SIZE); // read-only segment must be page-aligned
    let merged_section_offset_rodata = offset;
    for module in modules.iter_mut() {
        if let Some(section) = module.sections.get_mut(&RelocatableSectionType::RoData) {
            offset = align_up(offset, DATA_ALIGN);
            section.resolved_offset = offset;
            section.resolved_virtual_address = LOAD_ADDR_BASE + offset;
            offset += section.size;
        }
    }
    let merged_section_size_rodata = offset - merged_section_offset_rodata;

    // merging `.tdata` sections
    //
    // Note that the `.tdata`, `.tbss`, `.data`, and `.bss` sections will be merged into
    // the same writable segment, so we need to calculate their offsets and virtual addresses together.
    offset = align_up(offset, PAGE_SIZE); // writable segment must be page-aligned
    let merged_section_offset_tdata = offset;
    for module in modules.iter_mut() {
        if let Some(section) = module.sections.get_mut(&RelocatableSectionType::TData) {
            offset = align_up(offset, DATA_ALIGN);
            section.resolved_offset = offset;
            section.resolved_virtual_address = LOAD_ADDR_BASE + offset;
            offset += section.size;
        }
    }
    let merged_section_size_tdata = offset - merged_section_offset_tdata;

    // merging `.tbss` sections
    offset = align_up(offset, DATA_ALIGN);
    // Introduce a new variable `virtual_address` to keep track of the virtual address for `.tbss` section,
    // because `.tbss` is a NOBITS section and does not occupy space in the file,
    // so we cannot simply use `offset` to calculate the virtual address for `.tbss`.
    let mut virtual_address = LOAD_ADDR_BASE + offset;

    let merged_section_virtual_address_tbss = virtual_address;
    for module in modules.iter_mut() {
        if let Some(section) = module.sections.get_mut(&RelocatableSectionType::TBss) {
            // Only the `virtual_address` needs to be aligned to `DATA_ALIGN` before merging the `.tbss` section,
            virtual_address = align_up(virtual_address, DATA_ALIGN);

            section.resolved_offset = offset;
            section.resolved_virtual_address = virtual_address;

            // Note that `.tbss` is a NOBITS section, so it does not occupy space in the file,
            // but it does occupy space in memory.
            // Therefore, we need to increase the `virtual_address` by
            // the size of the `.tbss` section, but we do not increase the `offset`.
            virtual_address += section.size;
        }
    }
    let merged_section_size_tbss = virtual_address - merged_section_virtual_address_tbss;

    // merging `.data` sections
    offset = align_up(offset, DATA_ALIGN);
    virtual_address = align_up(virtual_address, DATA_ALIGN);

    let merged_section_virtual_address_data = virtual_address;
    for module in modules.iter_mut() {
        if let Some(section) = module.sections.get_mut(&RelocatableSectionType::Data) {
            // Both `offset` and `virtual_address` need to be aligned to `DATA_ALIGN` before merging the `.data` section.
            offset = align_up(offset, DATA_ALIGN);
            virtual_address = align_up(virtual_address, DATA_ALIGN);

            section.resolved_offset = offset;
            section.resolved_virtual_address = virtual_address;

            // Both `offset` and `virtual_address` need to be increased by the size of the `.data` section,
            offset += section.size;
            virtual_address += section.size;
        }
    }
    let merged_section_size_data = virtual_address - merged_section_virtual_address_data;

    // The linker-generated symbol `_edata` points to the end of the initialized data segment.
    let symbol_edata_offset = offset;
    let symbol_edata_virtual_address = virtual_address;

    // merging `.bss`
    offset = align_up(offset, DATA_ALIGN);
    virtual_address = align_up(virtual_address, DATA_ALIGN);

    // The linker-generated symbol `__bss_start` points to the start of the uninitialized data segment.
    let symbol_bss_start_offset = offset;
    let symbol_bss_start_virtual_address = virtual_address;

    let merged_section_virtual_address_bss = virtual_address;
    for module in modules.iter_mut() {
        if let Some(section) = module.sections.get_mut(&RelocatableSectionType::Bss) {
            // Only the `virtual_address` needs to be aligned to `DATA_ALIGN` before merging the `.bss` section,
            virtual_address = align_up(virtual_address, DATA_ALIGN);

            section.resolved_offset = offset;
            section.resolved_virtual_address = virtual_address;

            // Note that `.bss` is a NOBITS section, so it does not occupy space in the file,
            // but it does occupy space in memory.
            // Therefore, we need to increase the `virtual_address` by
            // the size of the `.bss` section, but we do not increase the `offset`.
            virtual_address += section.size;
        }
    }
    let merged_section_size_bss = virtual_address - merged_section_virtual_address_bss;

    // The linker-generated symbol `_end` points to the end of the uninitialized data segment.
    let symbol_end_offset = offset;
    let symbol_end_virtual_address = virtual_address;

    // Generate linker-generated symbols for the final executable.
    // Unlike GNU ld, we do not write the linker-generated symbols into the symbol table of the final executable,
    // but we still need to resolve their offsets and virtual addresses for relocations.
    let mut linker_generated_symbols: HashMap<String, FoundSymbol> = HashMap::new();
    linker_generated_symbols.insert(
        "_edata".to_string(),
        FoundSymbol {
            section_type: RelocatableSectionType::Bss,
            resolved_offset: symbol_edata_offset,
            resolved_virtual_address: symbol_edata_virtual_address,
        },
    );
    linker_generated_symbols.insert(
        "__bss_start".to_string(),
        FoundSymbol {
            section_type: RelocatableSectionType::Bss,
            resolved_offset: symbol_bss_start_offset,
            resolved_virtual_address: symbol_bss_start_virtual_address,
        },
    );
    linker_generated_symbols.insert(
        "_end".to_string(),
        FoundSymbol {
            section_type: RelocatableSectionType::Bss,
            resolved_offset: symbol_end_offset,
            resolved_virtual_address: symbol_end_virtual_address,
        },
    );

    // Resolve the offsets and virtual addresses for symbols.
    for module in modules.iter_mut() {
        for symbol in module.symbols.iter_mut() {
            if let RelocatableSymbol::Defined {
                name,
                section_type,
                offset,
                resolved_offset,
                resolved_virtual_address,
                ..
            } = symbol
            {
                if let Some(section) = module.sections.get(section_type) {
                    let offset_in_original_section = *offset;
                    *resolved_offset = section.resolved_offset + offset_in_original_section;
                    *resolved_virtual_address =
                        section.resolved_virtual_address + offset_in_original_section;
                } else {
                    return Err(LinkerError::Message(format!(
                        "Symbol {} is defined in section {:?}, but the section is not present in the module",
                        name, section_type
                    )));
                }
            }
        }
    }

    // Calculate the values to be patched for all relocations and store them in a list of patch items.
    let mut patch_items = Vec::new();

    for (module_index, module) in modules.iter().enumerate() {
        for (patch_section_type, relocations) in module.relocations.iter() {
            for (relocation_index, relocation) in relocations.iter().enumerate() {
                let relocation_type = relocation.relocation_type;
                let placeholder_offset = relocation.placeholder_offset;
                let addend = relocation.addend;

                let symbol_index = relocation.symbol_index;

                // This symbol may be a defined symbol in the current module,
                // or an external symbol that needs to be resolved from other modules.
                let symbol = &module.symbols[symbol_index];

                // Resolve the actual defined symbol that the relocation refers to.
                let target_symbol = match symbol {
                    RelocatableSymbol::Defined {
                        name,
                        bind,
                        section_type,
                        resolved_offset,
                        resolved_virtual_address,
                        ..
                    } => {
                        let mut target_symbol = FoundSymbol {
                            section_type: *section_type,
                            resolved_offset: *resolved_offset,
                            resolved_virtual_address: *resolved_virtual_address,
                        };

                        // If the symbol is weak, we need to check if there is a strong symbol with the same name in other modules.
                        if bind == &SymbolBind::Weak
                            && let Some(found_symbol) = find_global_symbol(name, modules, true)
                        {
                            target_symbol = found_symbol;
                        }

                        target_symbol
                    }
                    RelocatableSymbol::External(name) => {
                        if let Some(linker_generated_symbol) = linker_generated_symbols.get(name) {
                            linker_generated_symbol.clone()
                        } else if let Some(found_symbol) = find_global_symbol(name, modules, false)
                        {
                            found_symbol
                        } else {
                            return Err(LinkerError::Message(format!(
                                "Undefined symbol '{}' referenced in .text section, module: {}",
                                name, module_index
                            )));
                        }
                    }
                    RelocatableSymbol::Other => {
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
                            .resolved_virtual_address
                            .wrapping_add(addend as usize);

                        // Patch the relocated value into the code section at the placeholder offset.
                        // Note that the placeholder is usually 8 bytes (for 64-bit relocations), so we need to write 8 bytes.
                        PatchItem {
                            module_index,
                            patch_section_type: *patch_section_type,
                            patch_offset: placeholder_offset,
                            patch_value: PatchValue::Value64(relocated_value as u64),
                        }
                    }
                    RelocationType::R_X86_64_32 => {
                        // R_X86_64_32: S + A
                        let relocated_value = target_symbol
                            .resolved_virtual_address
                            .wrapping_add(addend as usize);
                        PatchItem {
                            module_index,
                            patch_section_type: *patch_section_type,
                            patch_offset: placeholder_offset,
                            patch_value: PatchValue::Value32(relocated_value as u32),
                        }
                    }
                    RelocationType::R_X86_64_PC32 => {
                        // R_X86_64_PC32: S + A - P
                        let section_text =
                            module.sections.get(&RelocatableSectionType::Text).unwrap();
                        let p = section_text.resolved_virtual_address + placeholder_offset;
                        let relocated_value = target_symbol
                            .resolved_virtual_address
                            .wrapping_add(addend as usize)
                            .wrapping_sub(p);

                        PatchItem {
                            module_index,
                            patch_section_type: *patch_section_type,
                            patch_offset: placeholder_offset,
                            patch_value: PatchValue::Value32(relocated_value as u32),
                        }
                    }
                    RelocationType::R_X86_64_TPOFF32 => {
                        // R_X86_64_TPOFF32: S + A - TP
                        // The formula for calculating the value to be written at the relocation site is:
                        // TPOFF(sym) = symbol_offset_in_tls_block − tls_block_size
                        let section_tdata =
                            module.sections.get(&RelocatableSectionType::TData).unwrap();
                        let section_tbss =
                            module.sections.get(&RelocatableSectionType::TBss).unwrap();

                        let tls_block_size = section_tbss.resolved_virtual_address
                            - section_tdata.resolved_virtual_address
                            + section_tbss.size;
                        let symbol_offset_in_tls_block = target_symbol.resolved_offset;

                        let relocated_value = symbol_offset_in_tls_block
                            .wrapping_add(addend as usize)
                            .wrapping_sub(tls_block_size);

                        PatchItem {
                            module_index,
                            patch_section_type: *patch_section_type,
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
        let relocatable_section_type = match patch_item.patch_section_type {
            RelocationEntrySectionType::Text => RelocatableSectionType::Text,
            RelocationEntrySectionType::RoData => RelocatableSectionType::RoData,
            RelocationEntrySectionType::TData => RelocatableSectionType::TData,
            RelocationEntrySectionType::Data => RelocatableSectionType::Data,
        };
        if let Some(section) = module.sections.get_mut(&relocatable_section_type) {
            let patch_section_data = match section.binary {
                SectionBinary::Reference(slice) => {
                    // If the section data is a reference, we need to convert it to
                    // an owned vector so that we can modify it.
                    section.binary = SectionBinary::Owned(slice.to_vec());

                    let SectionBinary::Owned(ref mut data) = section.binary else {
                        unreachable!();
                    };

                    data
                }
                SectionBinary::Owned(ref mut data) => data,
                SectionBinary::None => {
                    return Err(LinkerError::Message(format!(
                        "Section {:?} has no data in module {}",
                        relocatable_section_type, patch_item.module_index
                    )));
                }
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
        } else {
            return Err(LinkerError::Message(format!(
                "Section {:?} not found in module {}",
                relocatable_section_type, patch_item.module_index
            )));
        }
    }

    // Find the entry point symbol `_start` and get its virtual address.
    let entry_point = if let Some(entry_symbol) = find_global_symbol("_start", modules, false) {
        entry_symbol.resolved_virtual_address
    } else {
        return Err(LinkerError::Message(
            "Entry point symbol '_start' not found".to_string(),
        ));
    };

    let merged_section_size = MergedSectionSize {
        text: merged_section_size_text,
        rodata: merged_section_size_rodata,
        tdata: merged_section_size_tdata,
        tbss: merged_section_size_tbss,
        data: merged_section_size_data,
        bss: merged_section_size_bss,
    };

    let link_result = LinkResult {
        has_read_only_data,
        has_writable_data,
        has_tls,
        program_header_count,
        entry_point,
        merged_section_size,
    };

    Ok(link_result)
}

#[derive(Debug, Clone, PartialEq)]
struct PatchItem {
    module_index: usize,

    /// The patch target section type, which is the section
    /// that contains the placeholder to be patched.
    patch_section_type: RelocationEntrySectionType,

    /// The offset in the section where the relocation needs to be patched.
    /// Note that this offset is relative to the original section, not the merged section.
    patch_offset: usize,

    patch_value: PatchValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PatchValue {
    Value64(u64),
    Value32(u32),
}

fn align_up(val: usize, align: usize) -> usize {
    (val + align - 1) & !(align - 1)
}

#[derive(Debug, PartialEq)]
struct ExportedSymbolInfo {
    module_index: usize,
    is_strong: bool,
}

/// Get all exported symbols from the given modules.
fn get_exported_symbols(modules: &[RelocatableModule]) -> HashMap<String, Vec<ExportedSymbolInfo>> {
    let mut exported_symbols: HashMap</* symbol_name */ String, Vec<ExportedSymbolInfo>> =
        HashMap::new();

    for (module_index, module) in modules.iter().enumerate() {
        for symbol in &module.symbols {
            if let RelocatableSymbol::Defined { name, bind, .. } = symbol {
                if bind == &SymbolBind::Local {
                    continue; // Local symbols are not visible across modules, so we can skip them.
                }

                let is_strong = bind == &SymbolBind::Global;
                let exported_symbol_info = ExportedSymbolInfo {
                    module_index,
                    is_strong,
                };

                exported_symbols
                    .entry(name.clone())
                    .or_default()
                    .push(exported_symbol_info);
            }
        }
    }

    exported_symbols
}

/// Find the first duplicated "strong" global symbols across modules.
fn find_duplicated_strong_global_symbol(
    exported_symbols: &HashMap<String, Vec<ExportedSymbolInfo>>,
) -> Option<(
    /* symbol_name */ String,
    /* module_indices */ Vec<usize>,
)> {
    for (symbol_name, infos) in exported_symbols {
        let module_indices: Vec<usize> = infos
            .iter()
            .filter(|info| info.is_strong)
            .map(|info| info.module_index)
            .collect();

        if module_indices.len() > 1 {
            // Found a duplicated strong global symbol
            return Some((symbol_name.clone(), module_indices));
        }
    }
    None
}

#[derive(Debug, Clone, PartialEq)]
struct FoundSymbol {
    /// The section that the symbol belongs to.
    section_type: RelocatableSectionType,

    /// Offset in the merged section of the final executable.
    resolved_offset: usize,

    /// Virtual address in the merged section of the final executable.
    resolved_virtual_address: usize,
}

fn find_global_symbol(
    expected_name: &str,
    modules: &[RelocatableModule],
    require_strong: bool,
) -> Option<FoundSymbol> {
    let mut found_symbol_opt: Option<(FoundSymbol, /* is_strong */ bool)> = None;

    for module in modules {
        for symbol in &module.symbols {
            if let RelocatableSymbol::Defined {
                name,
                bind,
                section_type,
                resolved_offset,
                resolved_virtual_address,
                ..
            } = symbol
                && name == expected_name
                && bind != &SymbolBind::Local
            {
                if let Some((_, existing_is_strong)) = found_symbol_opt {
                    if bind == &SymbolBind::Global {
                        if existing_is_strong {
                            // We have found a duplicated strong symbol, which is an error.
                            // but we have already checked duplicated strong symbols in the
                            // `check_duplicated_strong_global_symbol` function, so we can just
                            // assume that this case won't happen here.
                            unreachable!(
                                "Duplicated strong global symbol '{}' found",
                                expected_name
                            );
                        } else {
                            // Override the existing weak symbol with the strong symbol.
                            found_symbol_opt = Some((
                                FoundSymbol {
                                    section_type: *section_type,
                                    resolved_offset: *resolved_offset,
                                    resolved_virtual_address: *resolved_virtual_address,
                                },
                                true,
                            ));
                        }
                    }
                } else {
                    found_symbol_opt = Some((
                        FoundSymbol {
                            section_type: *section_type,
                            resolved_offset: *resolved_offset,
                            resolved_virtual_address: *resolved_virtual_address,
                        },
                        bind == &SymbolBind::Global,
                    ));
                }
            }
        }
    }

    if let Some((found_symbol, is_strong)) = found_symbol_opt {
        if require_strong && !is_strong {
            None
        } else {
            Some(found_symbol)
        }
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::elf::{
        linker::link,
        relocatable::{RelocatableModule, read_relocatable},
    };

    fn get_example_file_binary(file_name: &str) -> Vec<u8> {
        let file_path = std::env::current_dir()
            .unwrap()
            .join("resources/examples/x86_64-linux")
            .join(file_name);

        fs::read(file_path).unwrap()
    }

    fn get_example_file_binaries(file_names: &[&str]) -> Vec<Vec<u8>> {
        file_names
            .iter()
            .map(|file_name| get_example_file_binary(file_name))
            .collect()
    }

    fn get_example_file_module<'a>(file_binary: &'a [u8]) -> RelocatableModule<'a> {
        read_relocatable(file_binary).unwrap()
    }

    fn get_example_file_modules<'a>(file_binaries: &[&'a [u8]]) -> Vec<RelocatableModule<'a>> {
        file_binaries
            .iter()
            .map(|file_binary| get_example_file_module(file_binary))
            .collect()
    }

    #[test]
    fn test_link_minimal() {
        let file_binaries = get_example_file_binaries(&["minimal.o"]);
        let file_binaries_ref: Vec<&[u8]> = file_binaries.iter().map(|b| b.as_slice()).collect();
        let mut modules = get_example_file_modules(&file_binaries_ref);
        let result = link(&mut modules).unwrap();
        println!("Module after linking: {:#?}", modules);
        println!("Link result: {:#?}", result);
    }

    #[test]
    fn test_link_function() {
        let file_binaries = get_example_file_binaries(&["function.o"]);
        let file_binaries_ref: Vec<&[u8]> = file_binaries.iter().map(|b| b.as_slice()).collect();
        let mut modules = get_example_file_modules(&file_binaries_ref);
        let result = link(&mut modules).unwrap();

        println!("Module after linking: {:#?}", modules);
        println!("Link result: {:#?}", result);
    }

    #[test]
    fn test_link_data() {
        let file_binaries = get_example_file_binaries(&["data.o"]);
        let file_binaries_ref: Vec<&[u8]> = file_binaries.iter().map(|b| b.as_slice()).collect();
        let mut modules = get_example_file_modules(&file_binaries_ref);
        let result = link(&mut modules).unwrap();

        println!("Module after linking: {:#?}", modules);
        println!("Link result: {:#?}", result);
    }

    #[test]
    fn test_link_symbol() {
        let file_binaries = get_example_file_binaries(&["symbol-export.o", "symbol-import.o"]);
        let file_binaries_ref: Vec<&[u8]> = file_binaries.iter().map(|b| b.as_slice()).collect();
        let mut modules = get_example_file_modules(&file_binaries_ref);
        let result = link(&mut modules).unwrap();

        println!("Module after linking: {:#?}", modules);
        println!("Link result: {:#?}", result);
    }

    #[test]
    fn test_link_override() {
        let file_binaries = get_example_file_binaries(&["override-weak.o", "override-strong.o"]);
        let file_binaries_ref: Vec<&[u8]> = file_binaries.iter().map(|b| b.as_slice()).collect();
        let mut modules = get_example_file_modules(&file_binaries_ref);
        let result = link(&mut modules).unwrap();

        println!("Module after linking: {:#?}", modules);
        println!("Link result: {:#?}", result);
    }

    #[test]
    fn test_link_relocate_within_data() {
        let file_binaries = get_example_file_binaries(&["relocate-within-data.o"]);
        let file_binaries_ref: Vec<&[u8]> = file_binaries.iter().map(|b| b.as_slice()).collect();
        let mut modules = get_example_file_modules(&file_binaries_ref);
        let result = link(&mut modules).unwrap();

        println!("Module after linking: {:#?}", modules);
        println!("Link result: {:#?}", result);
    }
}
