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

pub struct AArch64RelocationResolver;

impl RelocationResolver for AArch64RelocationResolver {
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
            ResolvedSymbol::VirtualAddress(v) => Ok(*v as u64),
            ResolvedSymbol::Absolute(v) => Ok(*v),
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

        let symbol_value = get_symbol_value(relocation)?;
        let target = symbol_value.wrapping_add(addend as u64);

        let patch_item = match relocation_type {
            RelocationType::R_AARCH64_ABS64 => PatchItem::from_u64(placeholder_offset, target),
            RelocationType::R_AARCH64_ADR_PREL_PG_HI21 => {
                // ADRP: Page(S + A) - Page(P), encoded as immhi:immlo.
                let instruction = read_instruction(binary, placeholder_offset);
                let place = target_fragment_section.virtual_address + placeholder_offset;
                let page_delta = ((target & !0xfff) as i64).wrapping_sub((place & !0xfff) as i64);
                let immediate = ((page_delta >> 12) as u32) & 0x1f_ffff;
                let patched = (instruction & !0x60ff_ffe0)
                    | ((immediate & 0x3) << 29)
                    | ((immediate >> 2) << 5);

                PatchItem::from_u32(placeholder_offset, patched)
            }
            RelocationType::R_AARCH64_ADD_ABS_LO12_NC => {
                // ADD: bits [11:0] of S + A are stored in instruction bits [21:10].
                let instruction = read_instruction(binary, placeholder_offset);
                let patched = (instruction & !0x003f_fc00) | ((target as u32 & 0xfff) << 10);

                PatchItem::from_u32(placeholder_offset, patched)
            }
            RelocationType::R_AARCH64_LDST64_ABS_LO12_NC => {
                // LD/ST 64-bit: bits [11:3] of S + A are stored in instruction bits [21:10].
                let instruction = read_instruction(binary, placeholder_offset);
                let patched = (instruction & !0x003f_fc00) | (((target as u32 & 0xfff) >> 3) << 10);

                PatchItem::from_u32(placeholder_offset, patched)
            }
            RelocationType::R_AARCH64_CALL26 => {
                // CALL26: S + A - P, divided by four, is stored in instruction bits [25:0].
                let instruction = read_instruction(binary, placeholder_offset);
                let place = target_fragment_section.virtual_address + placeholder_offset;
                let immediate = target.wrapping_sub(place as u64) >> 2;
                let patched = (instruction & !0x03ff_ffff) | (immediate as u32 & 0x03ff_ffff);

                PatchItem::from_u32(placeholder_offset, patched)
            }
            _ => {
                unreachable!(
                    "Relocation type {:?} is not supported for AArch64 architecture",
                    relocation_type
                );
            }
        };

        patch_items.push(patch_item);
    }

    Ok(patch_items)
}

fn read_instruction(binary: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(binary[offset..offset + 4].try_into().unwrap())
}
