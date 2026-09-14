// Copyright (c) 2026 Hemashushu <hippospark@gmail.com>, All rights reserved.
//
// This Source Code Form is subject to the terms of
// the Mozilla Public License version 2.0 and additional exceptions.
// For more details, see the LICENSE, LICENSE.additional, and CONTRIBUTING files.

use std::collections::HashMap;

use crate::{
    error::LinkerError,
    macho::{
        external_symbol_resolver::{ResolvedModule, ResolvedSymbol},
        merger::{FragmentSectionBinary, MergedFileLayout, SectionName},
        module::RelocationType,
    },
};

pub struct PatchModule {
    pub patch_sections: HashMap<SectionName, Vec<PatchItem>>,
}

pub struct PatchItem {
    pub offset: u64,
    pub patch_type: PatchType,
}

pub enum PatchType {
    U32(u32),
    U64(u64),
}

pub struct AArch64RelocationResolver;

impl AArch64RelocationResolver {
    pub fn resolve(
        _merged_file_layout: &MergedFileLayout,
        resolved_modules: &[ResolvedModule],
    ) -> Result<Vec<PatchModule>, LinkerError> {
        let mut patch_modules = Vec::new();

        for resolved_module in resolved_modules {
            let mut patch_sections: HashMap<SectionName, Vec<PatchItem>> = HashMap::new();

            for reloc_section in &resolved_module.relocation_sections {
                let target_section_name = reloc_section.target_section_name;
                let target_section = match resolved_module.sections.get(&target_section_name) {
                    Some(s) => s,
                    None => continue,
                };

                let target_base_vaddr = target_section.virtual_address;

                let source_binary = match &target_section.binary {
                    FragmentSectionBinary::Referenced(d) => *d,
                    FragmentSectionBinary::Owned(d) => d.as_slice(),
                    FragmentSectionBinary::None => &[],
                };

                let mut idx = 0;
                while idx < reloc_section.relocations.len() {
                    let reloc = &reloc_section.relocations[idx];

                    // Check for SUBTRACTOR + UNSIGNED pair
                    if reloc.relocation_type == RelocationType::ARM64_RELOC_SUBTRACTOR
                        && idx + 1 < reloc_section.relocations.len()
                        && reloc_section.relocations[idx + 1].relocation_type
                            == RelocationType::ARM64_RELOC_UNSIGNED
                    {
                        let sub_reloc = reloc;
                        let add_reloc = &reloc_section.relocations[idx + 1];

                        let sub_target_vaddr =
                            get_target_vaddr(resolved_module, sub_reloc, target_section);
                        let add_target_vaddr =
                            get_target_vaddr(resolved_module, add_reloc, target_section);

                        let offset = sub_reloc.offset.saturating_sub(target_section.virtual_address);

                        if sub_reloc.length == 8 || add_reloc.length == 8 {
                            let value = (add_target_vaddr as i64 - sub_target_vaddr as i64) as u64;
                            patch_sections.entry(target_section_name).or_default().push(PatchItem {
                                offset,
                                patch_type: PatchType::U64(value),
                            });
                        } else {
                            let value = (add_target_vaddr as i64 - sub_target_vaddr as i64) as u32;
                            patch_sections.entry(target_section_name).or_default().push(PatchItem {
                                offset,
                                patch_type: PatchType::U32(value),
                            });
                        }

                        idx += 2;
                        continue;
                    }

                    let target_vaddr = get_target_vaddr(resolved_module, reloc, target_section);
                    let reloc_offset_in_section =
                        reloc.offset.saturating_sub(target_section.virtual_address);
                    let site_vaddr = target_base_vaddr + reloc_offset_in_section;
                    let site_offset = reloc_offset_in_section as usize;

                    match reloc.relocation_type {
                        RelocationType::ARM64_RELOC_BRANCH26 => {
                            if site_offset + 4 <= source_binary.len() {
                                let mut insn = u32::from_le_bytes(
                                    source_binary[site_offset..site_offset + 4]
                                        .try_into()
                                        .unwrap(),
                                );
                                let delta = (target_vaddr as i64 - site_vaddr as i64) / 4;
                                let imm26 = (delta & 0x03ffffff) as u32;
                                insn = (insn & !0x03ffffff) | imm26;

                                patch_sections.entry(target_section_name).or_default().push(PatchItem {
                                    offset: reloc_offset_in_section,
                                    patch_type: PatchType::U32(insn),
                                });
                            }
                        }
                        RelocationType::ARM64_RELOC_PAGE21 => {
                            if site_offset + 4 <= source_binary.len() {
                                let mut insn = u32::from_le_bytes(
                                    source_binary[site_offset..site_offset + 4]
                                        .try_into()
                                        .unwrap(),
                                );
                                let target_page = target_vaddr & !0xfff;
                                let site_page = site_vaddr & !0xfff;
                                let page_delta = (target_page as i64 - site_page as i64) >> 12;

                                let immlo = ((page_delta & 3) as u32) << 29;
                                let immhi = (((page_delta >> 2) & 0x7ffff) as u32) << 5;

                                insn = (insn & !(0x3 << 29) & !(0x7ffff << 5)) | immlo | immhi;

                                patch_sections.entry(target_section_name).or_default().push(PatchItem {
                                    offset: reloc_offset_in_section,
                                    patch_type: PatchType::U32(insn),
                                });
                            }
                        }
                        RelocationType::ARM64_RELOC_GOT_LOAD_PAGE21 => {
                            if site_offset + 4 <= source_binary.len() {
                                let mut insn = u32::from_le_bytes(
                                    source_binary[site_offset..site_offset + 4]
                                        .try_into()
                                        .unwrap(),
                                );
                                let target_page = target_vaddr & !0xfff;
                                let site_page = site_vaddr & !0xfff;
                                let page_delta =
                                    (target_page as i64 - site_page as i64) >> 12;

                                let immlo = ((page_delta & 3) as u32) << 29;
                                let immhi = (((page_delta >> 2) & 0x7ffff) as u32) << 5;

                                insn = (insn & !(0x3 << 29) & !(0x7ffff << 5)) | immlo | immhi;

                                patch_sections.entry(target_section_name).or_default().push(PatchItem {
                                    offset: reloc_offset_in_section,
                                    patch_type: PatchType::U32(insn),
                                });
                            }
                        }
                        RelocationType::ARM64_RELOC_PAGEOFF12 => {
                            if site_offset + 4 <= source_binary.len() {
                                let mut insn = u32::from_le_bytes(
                                    source_binary[site_offset..site_offset + 4]
                                        .try_into()
                                        .unwrap(),
                                );
                                let page_offset = (target_vaddr & 0xfff) as u32;

                                let imm12 = if (insn & 0x3b000000) == 0x39000000 {
                                    // ldr/str unsigned immediate
                                    let size = (insn >> 30) & 0x3;
                                    page_offset >> size
                                } else {
                                    page_offset
                                };

                                insn = (insn & !(0xfff << 10)) | ((imm12 & 0xfff) << 10);

                                patch_sections.entry(target_section_name).or_default().push(PatchItem {
                                    offset: reloc_offset_in_section,
                                    patch_type: PatchType::U32(insn),
                                });
                            }
                        }
                        RelocationType::ARM64_RELOC_GOT_LOAD_PAGEOFF12 => {
                            if site_offset + 4 <= source_binary.len() {
                                let insn = u32::from_le_bytes(
                                    source_binary[site_offset..site_offset + 4]
                                        .try_into()
                                        .unwrap(),
                                );
                                let page_offset = (target_vaddr & 0xfff) as u32;

                                // GOT Relaxation: convert ldr xD, [xN, #off] to add xD, xN, #off
                                let add_insn = 0x91000000
                                    | (insn & 0x03ff)
                                    | ((page_offset & 0xfff) << 10);

                                patch_sections.entry(target_section_name).or_default().push(PatchItem {
                                    offset: reloc_offset_in_section,
                                    patch_type: PatchType::U32(add_insn),
                                });
                            }
                        }
                        RelocationType::ARM64_RELOC_UNSIGNED => {
                            if reloc.length == 8 {
                                let implicit_addend = if site_offset + 8 <= source_binary.len() {
                                    u64::from_le_bytes(
                                        source_binary[site_offset..site_offset + 8]
                                            .try_into()
                                            .unwrap(),
                                    )
                                } else {
                                    0
                                };
                                let final_vaddr = target_vaddr.wrapping_add(implicit_addend);
                                // On Mach-O 64-bit with dyld rebase, store image-relative offset
                                let rebase_val = if final_vaddr >= 0x100000000 {
                                    final_vaddr - 0x100000000
                                } else {
                                    final_vaddr
                                };
                                patch_sections.entry(target_section_name).or_default().push(PatchItem {
                                    offset: reloc_offset_in_section,
                                    patch_type: PatchType::U64(rebase_val),
                                });
                            } else {
                                patch_sections.entry(target_section_name).or_default().push(PatchItem {
                                    offset: reloc_offset_in_section,
                                    patch_type: PatchType::U32(target_vaddr as u32),
                                });
                            }
                        }
                        _ => {}
                    }

                    idx += 1;
                }
            }

            patch_modules.push(PatchModule { patch_sections });
        }

        Ok(patch_modules)
    }
}

fn get_target_vaddr(
    resolved_module: &ResolvedModule,
    reloc: &crate::macho::module::Relocation,
    _target_section: &crate::macho::merger::FragmentSection,
) -> u64 {
    if reloc.is_extern {
        if reloc.symbol_index < resolved_module.symbols.len() {
            match &resolved_module.symbols[reloc.symbol_index] {
                ResolvedSymbol::VirtualAddress(addr) => *addr,
                ResolvedSymbol::Absolute(val) => *val,
                ResolvedSymbol::Other => 0,
            }
        } else {
            0
        }
    } else {
        // When is_extern is 0, symbol_index is 1-based section index in module
        if let Some(&target_sect_name) = resolved_module.section_ordinals.get(&reloc.symbol_index) {
            resolved_module
                .sections
                .get(&target_sect_name)
                .map(|s| s.virtual_address)
                .unwrap_or(0)
        } else {
            0
        }
    }
}
