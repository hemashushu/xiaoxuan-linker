// Copyright (c) 2026 Hemashushu <hippospark@gmail.com>, All rights reserved.
//
// This Source Code Form is subject to the terms of
// the Mozilla Public License version 2.0 and additional exceptions.
// For more details, see the LICENSE, LICENSE.additional, and CONTRIBUTING files.

use std::collections::HashMap;

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

pub struct RiscV64RelocationResolver;

impl RelocationResolver for RiscV64RelocationResolver {
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
) -> Result<Vec<PatchItem>, LinkerError> {

    let target_fragment_section = fragment_sections.get(target_section_name).unwrap();
    let FragmentSectionBinary::Referenced(binary) = target_fragment_section.binary else {
        return Err(LinkerError::Message(format!(
            "Section {} does not have a referenced binary",
            target_section_name
        )));
    };

    let get_symbol_value = |relocation: &Relocation| -> Result<usize, LinkerError> {
        match &symbols[relocation.symbol_index] {
            ResolvedSymbol::VirtualAddress(value) => {
                Ok(value.wrapping_add(relocation.addend as usize))
            }
            ResolvedSymbol::Absolute(value) => {
                Ok((*value as usize).wrapping_add(relocation.addend as usize))
            }
            _ => Err(LinkerError::Message(format!(
                "Symbol at index {} in module {} can not be used for relocation",
                relocation.symbol_index, module_name
            ))),
        }
    };

    let mut hi_targets = HashMap::new();
    for relocation in relocations {
        if relocation.relocation_type == RelocationType::R_RISCV_PCREL_HI20 {
            let p = target_fragment_section.virtual_address + relocation.offset;
            hi_targets.insert(p, get_symbol_value(relocation)?);
        }
    }

    let mut patche_items = Vec::new();
    for relocation in relocations {
        let offset = relocation.offset;
        let instruction = read_u32(binary, offset);
        let target = get_symbol_value(relocation)?;
        let patch = match relocation.relocation_type {
            RelocationType::R_RISCV_64 => PatchItem::from_u64(offset, target as u64),
            RelocationType::R_RISCV_PCREL_HI20 | RelocationType::R_RISCV_HI20 => {
                let delta = if relocation.relocation_type == RelocationType::R_RISCV_PCREL_HI20 {
                    (target as isize).wrapping_sub((target_fragment_section.virtual_address + offset) as isize)
                        as i64
                } else {
                    target as i64
                };
                let hi = ((delta + 0x800) >> 12) as u32;
                PatchItem::from_u32(offset, (instruction & 0xfff) | (hi << 12))
            }
            RelocationType::R_RISCV_PCREL_LO12_I => {
                let hi_address = target;
                let hi_target = *hi_targets.get(&hi_address).ok_or_else(|| {
                    LinkerError::Message(format!(
                        "RISC-V PC-relative HI20/LO12 relocation pair mismatch at offset {offset}"
                    ))
                })?;
                let delta = (hi_target as isize).wrapping_sub(hi_address as isize) as u32;
                PatchItem::from_u32(
                    offset,
                    (instruction & 0x000f_ffff) | ((delta & 0xfff) << 20),
                )
            }
            RelocationType::R_RISCV_LO12_I => PatchItem::from_u32(
                offset,
                (instruction & 0x000f_ffff) | ((target as u32 & 0xfff) << 20),
            ),
            RelocationType::R_RISCV_LO12_S => {
                let immediate = target as u32 & 0xfff;
                let patched = (instruction & 0x01fff07f)
                    | ((immediate & 0x1f) << 7)
                    | ((immediate >> 5) << 25);
                PatchItem::from_u32(offset, patched)
            }
            RelocationType::R_RISCV_CALL_PLT => {
                let delta = (target as isize)
                    .wrapping_sub((target_fragment_section.virtual_address + offset) as isize)
                    as i64;
                let hi = ((delta + 0x800) >> 12) as u32;
                let lo = delta as u32 & 0xfff;
                let first = (instruction & 0xfff) | (hi << 12);
                let second = read_u32(binary, offset + 4);
                let second = (second & 0x000f_ffff) | (lo << 20);
                PatchItem::new(offset, [first.to_le_bytes(), second.to_le_bytes()].concat())
            }
            _ => unreachable!(
                "Relocation type {:?} is not supported for RISC-V architecture",
                relocation.relocation_type
            ),
        };
        patche_items.push(patch);
    }
    Ok(patche_items)
}

fn read_u32(binary: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(binary[offset..offset + 4].try_into().unwrap())
}
