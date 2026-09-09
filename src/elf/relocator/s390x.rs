// Copyright (c) 2026 Hemashushu <hippospark@gmail.com>, All rights reserved.
//
// This Source Code Form is subject to the terms of
// the Mozilla Public License version 2.0 and additional exceptions.
// For more details, see the LICENSE, LICENSE.additional, and CONTRIBUTING files.

use crate::{
    elf::{
        external_symbol_resolver::{ResolvedModule, ResolvedSymbol},
        merger::{FragmentRelocationSection, FragmentSection, MergedFileLayout, SectionName},
        module::{Relocation, RelocationType},
        relocator::{PatchItem, PatchModule, RelocationResolver},
    },
    error::LinkerError,
};
use std::collections::HashMap;

pub struct S390xRelocationResolver;

impl RelocationResolver for S390xRelocationResolver {
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

                let mut patch_sections = HashMap::new();

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
    _merged_file_layout: &MergedFileLayout,
    fragment_sections: &HashMap<SectionName, FragmentSection>,
    target_section_name: &SectionName,
    relocations: &[Relocation],
    symbols: &[ResolvedSymbol],
) -> Result<Vec<PatchItem>, LinkerError> {
    let target_fragment_section = fragment_sections.get(target_section_name).unwrap();

    let get_symbol_value = |r: &Relocation| -> Result<u64, LinkerError> {
        match &symbols[r.symbol_index] {
            ResolvedSymbol::VirtualAddress(v) => Ok(*v),
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
        let place = target_fragment_section.virtual_address + placeholder_offset;

        let patch_item = match relocation_type {
            RelocationType::R_390_64 => PatchItem::from_u64_big_endian(placeholder_offset, target),
            RelocationType::R_390_PC32DBL | RelocationType::R_390_PLT32DBL => {
                let delta = target.wrapping_sub(place ) >> 1;
                PatchItem::from_u32_big_endian(placeholder_offset, delta as u32)
            }
            _ => unreachable!(
                "Relocation type {:?} is not supported for S390 architecture",
                relocation_type
            ),
        };
        patch_items.push(patch_item);
    }

    Ok(patch_items)
}
