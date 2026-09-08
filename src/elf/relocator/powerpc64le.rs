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
        module::{Relocation, RelocationType, get_load_address_base},
        relocator::{PatchItem, PatchModule, RelocationResolver},
    },
    error::LinkerError,
};
use std::collections::HashMap;

pub struct PowerPC64LERelocationResolver;

impl RelocationResolver for PowerPC64LERelocationResolver {
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
    merged_file_layout: &MergedFileLayout,
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

    let toc_value = merged_file_layout
        .get_non_empty_section_info(SectionName::TOC)
        .map(|section| section.virtual_address)
        .unwrap_or_else(|| get_load_address_base(crate::elf::module::Machine::PowerPC64))
        + 0x8000;

    let mut patch_items = Vec::new();

    for relocation in relocations {
        let relocation_type = relocation.relocation_type;
        let placeholder_offset = relocation.offset;
        let addend = relocation.addend;

        let symbol_value = get_symbol_value(relocation)?;
        let target = symbol_value.wrapping_add(addend as u64);

        let place = target_fragment_section.virtual_address + placeholder_offset;
        let signed = match relocation_type {
            RelocationType::R_PPC64_TOC16_HA
            | RelocationType::R_PPC64_TOC16_LO
            | RelocationType::R_PPC64_TOC16_LO_DS => target.wrapping_sub(toc_value as u64),
            _ => target,
        };

        let value16 = match relocation_type {
            RelocationType::R_PPC64_ADDR16_HIGHEST => target >> 48,
            RelocationType::R_PPC64_ADDR16_HIGHER => target >> 32,
            RelocationType::R_PPC64_ADDR16_HI => target >> 16,
            RelocationType::R_PPC64_ADDR16_LO => target,
            RelocationType::R_PPC64_ADDR16_HIGHERA => (target.wrapping_add(0x8000)) >> 32,
            RelocationType::R_PPC64_ADDR16_HIGHESTA => (target.wrapping_add(0x8000)) >> 48,
            RelocationType::R_PPC64_REL16_HA => (symbol_value + 0x8000) >> 16,
            RelocationType::R_PPC64_REL16_LO => symbol_value,
            RelocationType::R_PPC64_TOC16_HA => signed.wrapping_add(0x8000) >> 16,
            RelocationType::R_PPC64_TOC16_LO | RelocationType::R_PPC64_TOC16_LO_DS => signed,
            _ => 0,
        } as u32;

        let patch_item = match relocation_type {
            RelocationType::R_PPC64_ADDR64 => PatchItem::from_u64(placeholder_offset, target),
            RelocationType::R_PPC64_REL24 => {
                let ins = read32(binary, placeholder_offset);
                let displacement = target.wrapping_sub(place as u64);
                PatchItem::from_u32(
                    placeholder_offset,
                    (ins & !0x03ff_fffc) | ((displacement as u32) & 0x03ff_fffc),
                )
            }
            _ => {
                let ins = read32(binary, placeholder_offset);
                let instruction = if relocation_type == RelocationType::R_PPC64_REL16_HA {
                    // Static ET_EXEC does not provide the ELFv2 r12 entry value.
                    // Use r0 as the addis base to construct the absolute TOC address.
                    ins & !(0x1f << 16)
                } else {
                    ins
                };
                PatchItem::from_u32(
                    placeholder_offset,
                    (instruction & 0xffff_0000) | (value16 & 0xffff),
                )
            }
        };
        patch_items.push(patch_item);
    }
    Ok(patch_items)
}

fn read32(binary: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(binary[offset..offset + 4].try_into().unwrap())
}
