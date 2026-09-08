// Copyright (c) 2026 Hemashushu <hippospark@gmail.com>, All rights reserved.
//
// This Source Code Form is subject to the terms of
// the Mozilla Public License version 2.0 and additional exceptions.
// For more details, see the LICENSE, LICENSE.additional, and CONTRIBUTING files.

use std::collections::HashMap;

use crate::{
    elf::{
        external_symbol_resolver::{ResolvedModule, ResolvedSymbol},
        merger::{FragmentRelocationSection, FragmentSection, MergedFileLayout, SectionName},
        module::{Relocation, RelocationType},
        relocator::{PatchItem, PatchModule, RelocationResolver},
    },
    error::LinkerError,
};

pub struct X86_64RelocationResolver;

/// This linker does not support changing code size (e.g., relaxation of the RISC-V instruction set),
/// so a relocation resolver only needs to resolve the relocation entries and generate the corresponding patch items.
impl RelocationResolver for X86_64RelocationResolver {
    fn resolve(
        merged_file_layout: &MergedFileLayout,
        resolved_modules: &[ResolvedModule],
    ) -> Result<Vec<PatchModule>, LinkerError> {
        resolved_modules
            .iter()
            .map(|resolved_module| {
                let module_name = &resolved_module.name;
                let fragment_sections = &resolved_module.sections;
                let resolved_symbols = &resolved_module.symbols;

                let mut patch_sections: HashMap<SectionName, Vec<PatchItem>> = HashMap::new();

                for FragmentRelocationSection {
                    target_section_name,
                    relocations,
                } in &resolved_module.relocation_sections
                {
                    let patch_items = resolve_section(
                        module_name,
                        merged_file_layout,
                        fragment_sections,
                        target_section_name,
                        relocations,
                        resolved_symbols,
                    )?;

                    patch_sections.insert(*target_section_name, patch_items);
                }

                Ok(PatchModule { patch_sections })
            })
            .collect()
    }
}

fn resolve_section(
    module_name: &str,
    merged_file_layout: &MergedFileLayout,
    fragment_sections: &HashMap<SectionName, FragmentSection>,
    target_section_name: &SectionName,
    relocations: &[Relocation],
    symbols: &[ResolvedSymbol],
) -> Result<Vec<PatchItem>, LinkerError> {
    let get_symbol_value = |r: &Relocation| -> Result<u64, LinkerError> {
        match &symbols[r.symbol_index] {
            ResolvedSymbol::VirtualAddress(v) => Ok(*v as u64),
            ResolvedSymbol::Absolute(v) => Ok(*v),
            _ => Err(LinkerError::Message(format!(
                "Symbol at index {} in module {} can not be used for relocation",
                r.symbol_index, module_name
            ))),
        }
    };

    let mut patch_items = Vec::new();

    // Process each relocation here
    for relocation in relocations {
        let relocation_type = relocation.relocation_type;
        let placeholder_offset = relocation.offset;
        let addend = relocation.addend;

        let symbol_value = get_symbol_value(relocation)?;
        let target = symbol_value.wrapping_add(addend as u64);

        let patch_item = match relocation_type {
            RelocationType::R_X86_64_64 => {
                // R_X86_64_64: S + A
                // Patch the relocated value into the code section at the placeholder offset.
                // Note that the placeholder is usually 8 bytes (for 64-bit relocations), so we need to write 8 bytes.
                PatchItem::from_u64(placeholder_offset, target)
            }
            RelocationType::R_X86_64_32 => {
                // R_X86_64_32: S + A
                PatchItem::from_u32(placeholder_offset, target as u32)
            }
            RelocationType::R_X86_64_PC32 | RelocationType::R_X86_64_PLT32 => {
                // R_X86_64_PC32: S + A - P
                let fragment_section = fragment_sections.get(target_section_name).unwrap();
                let p = fragment_section.virtual_address + placeholder_offset;
                let relocated_value = target.wrapping_sub(p as u64);

                PatchItem::from_u32(placeholder_offset, relocated_value as u32)
            }
            RelocationType::R_X86_64_TPOFF32 => {
                // R_X86_64_TPOFF32: S + A - TP
                //
                // The formula for calculating the value to be written at the relocation site is:
                // TPOFF(sym) = symbol_offset_in_tls_block − tls_block_size
                let file_sections = &merged_file_layout.merged_section_infos;
                let file_section_tdata = file_sections.get(&SectionName::TData).unwrap();
                let file_section_tbss = file_sections.get(&SectionName::TBSS).unwrap();

                let tls_block_size = file_section_tbss.virtual_address
                    - file_section_tdata.virtual_address
                    + file_section_tbss.size;
                let symbol_offset_in_tls_block =
                    symbol_value - file_section_tdata.virtual_address as u64;

                let relocated_value = symbol_offset_in_tls_block
                    .wrapping_add(addend as u64)
                    .wrapping_sub(tls_block_size as u64);

                PatchItem::from_u32(placeholder_offset, relocated_value as u32)
            }
            _ => {
                unreachable!(
                    "Relocation type {:?} is not supported for x86_64 architecture",
                    relocation_type
                );
            }
        };

        patch_items.push(patch_item);
    }

    Ok(patch_items)
}
