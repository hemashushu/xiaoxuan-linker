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
                        &got_slots,
                    )?;

                    patch_sections.insert(*target_section_name, patch_items);
                }

                if fragment_sections.get(&SectionName::GOT).is_some() {
                    let mut got_patches = Vec::new();

                    for (symbol_index, offset) in &got_slots {
                        let target = match &resolved_symbols[*symbol_index] {
                            ResolvedSymbol::VirtualAddress(value) => *value,
                            ResolvedSymbol::Absolute(value) => *value as usize,
                            _ => {
                                return Err(LinkerError::Message(format!(
                                    "Symbol at index {} in module {} can not initialize GOT",
                                    symbol_index, module_name
                                )));
                            }
                        };
                        got_patches.push(PatchItem::from_u64(*offset, target as u64));
                    }

                    if !got_patches.is_empty() {
                        patch_sections.insert(SectionName::GOT, got_patches);
                    }
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
    got_slots: &HashMap<usize, usize>,
) -> Result<Vec<PatchItem>, LinkerError> {
    let fragment_section = fragment_sections.get(target_section_name).unwrap();
    let FragmentSectionBinary::Referenced(binary) = fragment_section.binary else {
        return Err(LinkerError::Message(format!(
            "Section {} in module {} does not have binary data",
            target_section_name, module_name
        )));
    };

    let get_symbol_value = |r: &Relocation| -> Result<usize, LinkerError> {
        match &symbols[r.symbol_index] {
            ResolvedSymbol::VirtualAddress(v) => Ok(v.wrapping_add(r.addend as usize)),
            ResolvedSymbol::Absolute(v) => Ok((*v as usize).wrapping_add(r.addend as usize)),
            _ => Err(LinkerError::Message(format!(
                "Symbol at index {} in module {} can not be used for relocation",
                r.symbol_index, module_name
            ))),
        }
    };

    let mut patch_items = Vec::new();

    for relocation in relocations {
        let relocation_type = relocation.relocation_type;
        let placeholder_offset = relocation.offset;
        let addend = relocation.addend;

        let ins = read32(binary, placeholder_offset);
        let symbol_value = get_symbol_value(relocation)?;
        let target = symbol_value.wrapping_add(addend as usize);
        let place = fragment_section.virtual_address + placeholder_offset;

        let patch_item = match relocation_type {
            RelocationType::R_LARCH_64 => PatchItem::from_u64(placeholder_offset, target as u64),
            RelocationType::R_LARCH_GOT_PC_HI20 | RelocationType::R_LARCH_GOT_PC_LO12 => {
                let slot = *got_slots.get(&relocation.symbol_index).ok_or_else(|| {
                    LinkerError::Message(format!(
                        "Missing LoongArch GOT slot for symbol {} in module {}",
                        relocation.symbol_index, module_name
                    ))
                })?;

                let got_section = fragment_sections.get(&SectionName::GOT).ok_or_else(|| {
                    LinkerError::Message(format!(
                        "Missing synthetic LoongArch GOT section in module {}",
                        module_name
                    ))
                })?;

                let got_address = got_section.virtual_address + slot;
                let delta = (got_address as isize).wrapping_sub(place as isize) as i64;

                if relocation_type == RelocationType::R_LARCH_GOT_PC_HI20 {
                    let hi = ((delta + 0x800) >> 12) as u32;
                    PatchItem::from_u32(
                        placeholder_offset,
                        (ins & !(0xfffff << 5)) | ((hi & 0xfffff) << 5),
                    )
                } else {
                    PatchItem::from_u32(
                        placeholder_offset,
                        (ins & !(0xfff << 10)) | ((got_address as u32 & 0xfff) << 10),
                    )
                }
            }
            RelocationType::R_LARCH_PCALA_HI20 => {
                let delta = (target as isize).wrapping_sub(place as isize) as i64;
                let hi = ((delta + 0x800) >> 12) as u32;
                PatchItem::from_u32(
                    placeholder_offset,
                    (ins & !(0xfffff << 5)) | ((hi & 0xfffff) << 5),
                )
            }
            RelocationType::R_LARCH_PCALA_LO12 => PatchItem::from_u32(
                placeholder_offset,
                (ins & !(0xfff << 10)) | ((target as u32 & 0xfff) << 10),
            ),
            RelocationType::R_LARCH_B26 => {
                let displacement = (target as isize).wrapping_sub(place as isize) as i64;
                let immediate = (displacement >> 2) as u32;
                let patched = (ins & !0x03ff_ffff)
                    | ((immediate & 0xffff) << 10)
                    | ((immediate >> 16) & 0x3ff);
                PatchItem::from_u32(placeholder_offset, patched)
            }
            RelocationType::R_LARCH_CALL36 => {
                let displacement = (target as isize).wrapping_sub(place as isize) as i64;
                let next = read32(binary, placeholder_offset + 4);
                let high = ((displacement + 0x8000) >> 16) as u32;
                let first = (ins & !(0xfffff << 5)) | ((high & 0xfffff) << 5);
                let low = displacement >> 2;
                let second = (next & !(0xffff << 10)) | ((low as u32 & 0xffff) << 10);
                PatchItem::new(
                    placeholder_offset,
                    [first.to_le_bytes(), second.to_le_bytes()].concat(),
                )
            }
            _ => unreachable!(
                "Relocation type {:?} is not supported for LoongArch architecture",
                relocation_type
            ),
        };
        patch_items.push(patch_item);
    }
    Ok(patch_items)
}

fn read32(binary: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(binary[offset..offset + 4].try_into().unwrap())
}
