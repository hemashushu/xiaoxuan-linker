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
        merged_file_layout: &MergedFileLayout,
        resolved_modules: &[ResolvedModule],
    ) -> Result<Vec<PatchModule>, LinkerError> {
        resolved_modules
            .iter()
            .map(|resolved_module| {
                let mut patch_sections = HashMap::new();
                let mut got_slots = HashMap::new();

                for relocation_section in &resolved_module.relocation_sections {
                    for relocation in &relocation_section.relocations {
                        if matches!(
                            relocation.relocation_type,
                            RelocationType::R_LARCH_GOT_PC_HI20
                                | RelocationType::R_LARCH_GOT_PC_LO12
                        ) {
                            let next_slot = got_slots.len() * 8;
                            got_slots
                                .entry(relocation.symbol_index)
                                .or_insert(next_slot);
                        }
                    }
                }

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
                            &got_slots,
                        )?,
                    );
                }

                if let Some(got_section) = resolved_module.sections.get(&SectionName::TOC) {
                    let mut got_patches = Vec::new();
                    for (symbol_index, offset) in &got_slots {
                        let target = match &resolved_module.symbols[*symbol_index] {
                            ResolvedSymbol::VirtualAddress(value) => *value,
                            ResolvedSymbol::Absolute(value) => *value as usize,
                            _ => {
                                return Err(LinkerError::Message(format!(
                                    "Symbol at index {} in module {} can not initialize GOT",
                                    symbol_index, resolved_module.name
                                )));
                            }
                        };
                        got_patches.push(PatchItem::from_u64(*offset, target as u64));
                    }
                    if !got_patches.is_empty() {
                        patch_sections.insert(SectionName::TOC, got_patches);
                    }
                    let _ = got_section;
                }

                Ok(PatchModule {
                    patch_sections,
                })
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
    got_slots: &HashMap<usize, usize>,
) -> Result<Vec<PatchItem>, LinkerError> {
    let section = fragment_sections.get(target_section_name).unwrap();
    let FragmentSectionBinary::Referenced(binary) = section.binary else {
        return Err(LinkerError::Message(format!(
            "Section {} does not have a referenced binary",
            target_section_name
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
            RelocationType::R_LARCH_GOT_PC_HI20 | RelocationType::R_LARCH_GOT_PC_LO12 => {
                let slot = *got_slots.get(&r.symbol_index).ok_or_else(|| {
                    LinkerError::Message(format!(
                        "Missing LoongArch GOT slot for symbol {}",
                        r.symbol_index
                    ))
                })?;
                let got_section = fragment_sections.get(&SectionName::TOC).ok_or_else(|| {
                    LinkerError::Message("Missing synthetic LoongArch GOT section".to_string())
                })?;
                let got_address = got_section.virtual_address + slot;
                let delta = (got_address as isize).wrapping_sub(place as isize) as i64;
                if r.relocation_type == RelocationType::R_LARCH_GOT_PC_HI20 {
                    let hi = ((delta + 0x800) >> 12) as u32;
                    PatchItem::from_u32(offset, (ins & !(0xfffff << 5)) | ((hi & 0xfffff) << 5))
                } else {
                    PatchItem::from_u32(
                        offset,
                        (ins & !(0xfff << 10)) | ((got_address as u32 & 0xfff) << 10),
                    )
                }
            }
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
                let displacement = (target as isize).wrapping_sub(place as isize) as i64;
                let next = read(binary, offset + 4);
                let high = ((displacement + 0x8000) >> 16) as u32;
                let first = (ins & !(0xfffff << 5)) | ((high & 0xfffff) << 5);
                let low = displacement >> 2;
                let second = (next & !(0xffff << 10)) | ((low as u32 & 0xffff) << 10);
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
