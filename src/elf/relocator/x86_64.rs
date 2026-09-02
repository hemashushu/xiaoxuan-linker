// Copyright (c) 2026 Hemashushu <hippospark@gmail.com>, All rights reserved.
//
// This Source Code Form is subject to the terms of
// the Mozilla Public License version 2.0 and additional exceptions.
// For more details, see the LICENSE, LICENSE.additional, and CONTRIBUTING files.

use std::collections::HashMap;

use crate::{
    elf::{
        merger::{
            MergedFileLayout, MergedModule, MergedRelocationSection, MergedSection, MergedSymbol,
            SectionName,
        },
        module::{Relocation, RelocationType},
        relocator::{PatchItem, PatchModule, RelocationResolver},
    },
    error::LinkerError,
};

pub struct X86_64RelocationResolver;

impl RelocationResolver for X86_64RelocationResolver {
    fn resolve(
        merged_file_layout: &MergedFileLayout,
        merged_modules: &[MergedModule],
    ) -> Result<Vec<PatchModule>, LinkerError> {
        let mut patch_modules = Vec::new();

        for merged_module in merged_modules {
            let symbols = &merged_module.symbols;
            let merged_sections = &merged_module.sections;

            let mut patch_sections: HashMap<SectionName, Vec<PatchItem>> = HashMap::new();

            for MergedRelocationSection {
                target_section_name,
                relocations,
            } in &merged_module.relocation_sections
            {
                let patch_items = resolve_section(
                    merged_file_layout,
                    merged_sections,
                    target_section_name,
                    relocations,
                    symbols,
                )?;

                patch_sections.insert(*target_section_name, patch_items);
            }

            let patch_module = PatchModule { patch_sections };
            patch_modules.push(patch_module);
        }

        Ok(patch_modules)
    }
}

fn resolve_section(
    merged_file_layout: &MergedFileLayout,
    merged_sections: &HashMap<SectionName, MergedSection>,
    target_section_name: &SectionName,
    relocations: &[Relocation],
    symbols: &[MergedSymbol],
) -> Result<Vec<PatchItem>, LinkerError> {
    let mut patch_items = Vec::new();

    let mut iter = relocations.iter();

    while let Some(relocation) = iter.next() {
        // Process each relocation here

        let relocation_type = relocation.relocation_type;
        let placeholder_offset = relocation.placeholder_offset;
        let addend = relocation.addend;

        let merged_symbol = &symbols[relocation.symbol_index];

        let MergedSymbol::Effective {
            // offset_in_section: symbol_offset_in_section,
            virtual_address: symbol_virtual_address,
        } = merged_symbol
        else {
            return Err(LinkerError::Message(format!(
                "Symbol at index {} can not be used for relocation",
                relocation.symbol_index,
            )));
        };

        let patch_item = match relocation_type {
            RelocationType::R_X86_64_64 => {
                // R_X86_64_64: S + A
                let relocated_value = symbol_virtual_address.wrapping_add(addend as usize);

                // Patch the relocated value into the code section at the placeholder offset.
                // Note that the placeholder is usually 8 bytes (for 64-bit relocations), so we need to write 8 bytes.
                PatchItem::from_u64(placeholder_offset, relocated_value as u64)
            }
            RelocationType::R_X86_64_32 => {
                // R_X86_64_32: S + A
                let relocated_value = symbol_virtual_address.wrapping_add(addend as usize);
                PatchItem::from_u32(placeholder_offset, relocated_value as u32)
            }
            RelocationType::R_X86_64_PC32 | RelocationType::R_X86_64_PLT32 => {
                // R_X86_64_PC32: S + A - P
                let merged_section = merged_sections.get(target_section_name).unwrap();
                let p = merged_section.virtual_address + placeholder_offset;
                let relocated_value = symbol_virtual_address
                    .wrapping_add(addend as usize)
                    .wrapping_sub(p);

                PatchItem::from_u32(placeholder_offset, relocated_value as u32)
            }
            RelocationType::R_X86_64_TPOFF32 => {
                // R_X86_64_TPOFF32: S + A - TP
                // The formula for calculating the value to be written at the relocation site is:
                // TPOFF(sym) = symbol_offset_in_tls_block − tls_block_size
                let file_sections = &merged_file_layout.file_sections;
                let file_section_tdata = file_sections.get(&SectionName::TData).unwrap();
                let file_section_tbss = file_sections.get(&SectionName::TBSS).unwrap();

                let tls_block_size = file_section_tbss.virtual_address
                    - file_section_tdata.virtual_address
                    + file_section_tbss.size;
                let symbol_offset_in_tls_block =
                    symbol_virtual_address - file_section_tdata.virtual_address;

                let relocated_value = symbol_offset_in_tls_block
                    .wrapping_add(addend as usize)
                    .wrapping_sub(tls_block_size);

                PatchItem::from_u32(placeholder_offset, relocated_value as u32)
            }
            _ => {
                unimplemented!()
            }
        };

        patch_items.push(patch_item);
    }

    Ok(patch_items)
}
