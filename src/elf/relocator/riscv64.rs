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

/// This linker does not support changing code size (e.g., relaxation of the RISC-V instruction set),
/// so a relocation resolver only needs to resolve the relocation entries and generate the corresponding patch items.
impl RelocationResolver for RiscV64RelocationResolver {
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
    let FragmentSectionBinary::Referenced(binary) = target_fragment_section.binary else {
        return Err(LinkerError::Message(format!(
            "Section {} in module {} does not have binary data",
            target_section_name, module_name
        )));
    };

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

    let mut hi_targets = HashMap::new();
    for relocation in relocations {
        if relocation.relocation_type == RelocationType::R_RISCV_PCREL_HI20 {
            let p = target_fragment_section.virtual_address + relocation.offset;
            hi_targets.insert(p , get_symbol_value(relocation)?);
        }
    }

    let mut patch_items = Vec::new();

    for relocation in relocations {
        let relocation_type = relocation.relocation_type;
        let placeholder_offset = relocation.offset;
        let addend = relocation.addend;

        let instruction = read_u32(binary, placeholder_offset);
        let symbol_value = get_symbol_value(relocation)?;
        let target = symbol_value.wrapping_add(addend as u64);
        let place = target_fragment_section.virtual_address + placeholder_offset ;

        let patch_item = match relocation_type {
            RelocationType::R_RISCV_64 => PatchItem::from_u64(placeholder_offset, target),
            RelocationType::R_RISCV_PCREL_HI20 | RelocationType::R_RISCV_HI20 => {
                let delta = if relocation_type == RelocationType::R_RISCV_PCREL_HI20 {
                    target.wrapping_sub(place) as i32
                } else {
                    target as i32
                };
                let hi = ((delta + 0x800) >> 12) as u32;
                PatchItem::from_u32(placeholder_offset, (instruction & 0xfff) | (hi << 12))
            }
            RelocationType::R_RISCV_PCREL_LO12_I => {
                let hi_address = target;
                let hi_target = *hi_targets.get(&hi_address).ok_or_else(|| {
                    LinkerError::Message(format!(
                        "RISC-V PC-relative HI20/LO12 relocation pair mismatch at offset {}, section {} in module {}",
                        placeholder_offset, target_section_name, module_name))
                })?;
                let delta = hi_target.wrapping_sub(hi_address) as u32;
                PatchItem::from_u32(
                    placeholder_offset,
                    (instruction & 0x000f_ffff) | ((delta & 0xfff) << 20),
                )
            }
            RelocationType::R_RISCV_LO12_I => PatchItem::from_u32(
                placeholder_offset,
                (instruction & 0x000f_ffff) | ((target as u32 & 0xfff) << 20),
            ),
            RelocationType::R_RISCV_LO12_S => {
                let immediate = target as u32 & 0xfff;
                let patched = (instruction & 0x01fff07f)
                    | ((immediate & 0x1f) << 7)
                    | ((immediate >> 5) << 25);
                PatchItem::from_u32(placeholder_offset, patched)
            }
            RelocationType::R_RISCV_CALL_PLT => {
                let delta = target.wrapping_sub(place) as i32;
                let hi = ((delta + 0x800) >> 12) as u32;
                let lo = delta as u32 & 0xfff;
                let first = (instruction & 0xfff) | (hi << 12);
                let second = read_u32(binary, placeholder_offset + 4);
                let second2 = (second & 0x000f_ffff) | (lo << 20);
                PatchItem::new(
                    placeholder_offset,
                    [first.to_le_bytes(), second2.to_le_bytes()].concat(),
                )
            }
            _ => unreachable!(
                "Relocation type {:?} is not supported for RISC-V architecture",
                relocation_type
            ),
        };
        patch_items.push(patch_item);
    }
    Ok(patch_items)
}

fn read_u32(binary: &[u8], offset: u64) -> u32 {
    u32::from_le_bytes(
        binary[offset as usize..offset as usize + 4]
            .try_into()
            .unwrap(),
    )
}
