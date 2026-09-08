// Copyright (c) 2026 Hemashushu <hippospark@gmail.com>, All rights reserved.
//
// This Source Code Form is subject to the terms of
// the Mozilla Public License version 2.0 and additional exceptions.
// For more details, see the LICENSE, LICENSE.additional, and CONTRIBUTING files.

use crate::{
    elf::{
        external_symbol_resolver::{ResolvedModule, ResolvedSymbol},
        merger::{
            FragmentRelocationSection, FragmentSection, FragmentSectionBinary, MergedFileLayout,
            SectionName,
        },
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
                let mut patch_sections = HashMap::new();

                for FragmentRelocationSection {
                    target_section_name,
                    relocations,
                } in &resolved_module.relocation_sections
                {
                    patch_sections.insert(
                        *target_section_name,
                        resolve_section(
                            &resolved_module.name,
                            merged_file_layout,
                            &resolved_module.sections,
                            target_section_name,
                            relocations,
                            &resolved_module.symbols,
                        )?,
                    );
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
    let FragmentSectionBinary::Referenced(binary) = target_fragment_section.binary else {
        return Err(LinkerError::Message(format!(
            "Section {} does not have a referenced binary",
            target_section_name
        )));
    };

    let get_symbol_value = |r: &Relocation| -> Result<u64, LinkerError> {
        match &symbols[r.symbol_index] {
            ResolvedSymbol::VirtualAddress(v) => Ok(v.wrapping_add(r.addend as usize) as u64),
            ResolvedSymbol::Absolute(v) => Ok((*v as usize).wrapping_add(r.addend as usize) as u64),
            _ => Err(LinkerError::Message(format!(
                "Symbol at index {} in module {} can not be used for relocation",
                r.symbol_index, module_name
            ))),
        }
    };

    let mut patche_items = Vec::new();
    for relocation in relocations {
        let target = get_symbol_value(relocation)?;
        let place = target_fragment_section.virtual_address + relocation.offset;
        let patch = match relocation.relocation_type {
            RelocationType::R_390_64 => PatchItem::from_u64_big_endian(relocation.offset, target),
            RelocationType::R_390_PC32DBL | RelocationType::R_390_PLT32DBL => {
                let delta = target.wrapping_sub(place as u64) >> 1;
                PatchItem::from_u32_big_endian(relocation.offset, delta as u32)
            }
            _ => unreachable!(
                "Relocation type {:?} is not supported for S390 architecture",
                relocation.relocation_type
            ),
        };
        patche_items.push(patch);
    }
    let _ = binary;
    Ok(patche_items)
}
