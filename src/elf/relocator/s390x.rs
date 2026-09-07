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
        layout: &MergedFileLayout,
        modules: &[ResolvedModule],
    ) -> Result<Vec<PatchModule>, LinkerError> {
        modules
            .iter()
            .map(|m| {
                let mut sections = HashMap::new();
                for FragmentRelocationSection {
                    target_section_name,
                    relocations,
                } in &m.relocation_sections
                {
                    sections.insert(
                        *target_section_name,
                        resolve_section(
                            &m.name,
                            layout,
                            &m.sections,
                            target_section_name,
                            relocations,
                            &m.symbols,
                        )?,
                    );
                }
                Ok(PatchModule {
                    patch_sections: sections,
                })
            })
            .collect()
    }
}

fn resolve_section(
    module_name: &str,
    _layout: &MergedFileLayout,
    sections: &HashMap<SectionName, FragmentSection>,
    name: &SectionName,
    relocations: &[Relocation],
    symbols: &[ResolvedSymbol],
) -> Result<Vec<PatchItem>, LinkerError> {
    let section = sections.get(name).unwrap();
    let FragmentSectionBinary::Referenced(binary) = section.binary else {
        return Err(LinkerError::Message(format!(
            "Section {} does not have a referenced binary",
            name
        )));
    };

    let value = |r: &Relocation| -> Result<u64, LinkerError> {
        match &symbols[r.symbol_index] {
            ResolvedSymbol::VirtualAddress(v) => Ok(v.wrapping_add(r.addend as usize) as u64),
            ResolvedSymbol::Absolute(v) => Ok((*v as usize).wrapping_add(r.addend as usize) as u64),
            _ => Err(LinkerError::Message(format!(
                "Symbol at index {} in module {} can not be used for relocation",
                r.symbol_index, module_name
            ))),
        }
    };

    let mut patches = Vec::new();
    for r in relocations {
        let target = value(r)?;
        let place = section.virtual_address + r.offset;
        let patch = match r.relocation_type {
            RelocationType::R_390_64 => PatchItem::from_u64_big_endian(r.offset, target),
            RelocationType::R_390_PC32DBL | RelocationType::R_390_PLT32DBL => {
                let delta = target.wrapping_sub(place as u64) >> 1;
                PatchItem::from_u32_big_endian(r.offset, delta as u32)
            }
            _ => unreachable!(
                "Relocation type {:?} is not supported for S390 architecture",
                r.relocation_type
            ),
        };
        patches.push(patch);
    }
    let _ = binary;
    Ok(patches)
}
