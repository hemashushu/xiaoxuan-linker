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

pub struct LoongArch64RelocationResolver;

impl RelocationResolver for LoongArch64RelocationResolver {
    fn resolve(
        layout: &MergedFileLayout,
        modules: &[ResolvedModule],
    ) -> Result<Vec<PatchModule>, LinkerError> {
        modules
            .iter()
            .map(|module| {
                let mut sections = HashMap::new();
                for FragmentRelocationSection {
                    target_section_name,
                    relocations,
                } in &module.relocation_sections
                {
                    sections.insert(
                        *target_section_name,
                        resolve_section(
                            &module.name,
                            layout,
                            &module.sections,
                            target_section_name,
                            relocations,
                            &module.symbols,
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
    let value = |r: &Relocation| -> Result<usize, LinkerError> {
        match &symbols[r.symbol_index] {
            ResolvedSymbol::VirtualAddress(v) => Ok(v.wrapping_add(r.addend as usize)),
            ResolvedSymbol::Absolute(v) => Ok((*v as usize).wrapping_add(r.addend as usize)),
            _ => Err(LinkerError::Message(format!(
                "Symbol at index {} in module {} can not be used for relocation",
                r.symbol_index, module_name
            ))),
        }
    };

    let mut patches = Vec::new();
    for r in relocations {
        let offset = r.offset;
        let ins = read(binary, offset);
        let target = value(r)?;
        let place = section.virtual_address + offset;
        let patch = match r.relocation_type {
            RelocationType::R_LARCH_64 => PatchItem::from_u64(offset, target as u64),
            RelocationType::R_LARCH_PCALA_HI20 => {
                let delta = (target as isize).wrapping_sub(place as isize) as i64;
                let hi = ((delta + 0x800) >> 12) as u32;
                PatchItem::from_u32(offset, (ins & !(0xfffff << 5)) | ((hi & 0xfffff) << 5))
            }
            RelocationType::R_LARCH_PCALA_LO12 => PatchItem::from_u32(
                offset,
                (ins & !(0xfff << 10)) | ((target as u32 & 0xfff) << 10),
            ),
            RelocationType::R_LARCH_B26 => {
                let displacement = (target as isize).wrapping_sub(place as isize) as i64;
                let immediate = (displacement >> 2) as u32;
                let patched = (ins & !0x03ff_ffff)
                    | ((immediate & 0xffff) << 10)
                    | ((immediate >> 16) & 0x3ff);
                PatchItem::from_u32(offset, patched)
            }
            RelocationType::R_LARCH_CALL36 => {
                let imm = (target as isize).wrapping_sub(place as isize) as i64 >> 2;
                let next = read(binary, offset + 4);
                let first = (ins & !(0xfffff << 5)) | ((((imm >> 16) as u32) & 0xfffff) << 5);
                let second = (next & !(0xffff << 10)) | (((imm as u32) & 0xffff) << 10);
                PatchItem::new(offset, [first.to_le_bytes(), second.to_le_bytes()].concat())
            }
            _ => unreachable!(
                "Relocation type {:?} is not supported for LoongArch architecture",
                r.relocation_type
            ),
        };
        patches.push(patch);
    }
    Ok(patches)
}

fn read(binary: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(binary[offset..offset + 4].try_into().unwrap())
}
