// Copyright (c) 2026 Hemashushu <hippospark@gmail.com>, All rights reserved.
//
// This Source Code Form is subject to the terms of
// the Mozilla Public License version 2.0 and additional exceptions.
// For more details, see the LICENSE, LICENSE.additional, and CONTRIBUTING files.

use std::collections::{HashMap, HashSet};

use crate::{
    error::LinkerError,
    macho::{
        external_symbol_resolver::ResolvedModule,
        merger::{FragmentSectionBinary, MergedFileLayout, SectionName},
        module::CpuType,
        relocator::aarch64::{AArch64RelocationResolver, PatchType},
    },
};

mod aarch64;

#[derive(Debug, PartialEq)]
pub struct RelocatedModule<'a> {
    pub sections: HashMap<SectionName, RelocatedSection<'a>>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct RelocatedSection<'a> {
    pub size: u64,
    pub binary: RelocatedSectionBinary<'a>,
}

#[derive(Debug, PartialEq, Clone)]
pub enum RelocatedSectionBinary<'a> {
    Owned(Vec<u8>),
    Referenced(&'a [u8]),
    None,
}

pub fn relocate<'a>(
    merged_file_layout: &MergedFileLayout,
    resolved_modules: Vec<ResolvedModule<'a>>,
    cpu_type: CpuType,
) -> Result<Vec<RelocatedModule<'a>>, LinkerError> {
    let patch_modules = match cpu_type {
        CpuType::ARM64 => AArch64RelocationResolver::resolve(merged_file_layout, &resolved_modules)?,
        _ => {
            return Err(LinkerError::Message(format!(
                "Relocation for CPU type {} is not supported",
                cpu_type
            )));
        }
    };

    let mut relocated_modules = Vec::new();

    for (resolved_module, patch_module) in resolved_modules.into_iter().zip(patch_modules) {
        let mut relocated_sections: HashMap<SectionName, RelocatedSection<'a>> = HashMap::new();

        let mut section_names: HashSet<SectionName> = resolved_module.sections.keys().cloned().collect();
        for name in patch_module.patch_sections.keys() {
            section_names.insert(*name);
        }

        for section_name in section_names {
            let (source_size, mut binary) = if let Some(sec) = resolved_module.sections.get(&section_name) {
                let bin = match &sec.binary {
                    FragmentSectionBinary::Referenced(source_data) => source_data.to_vec(),
                    FragmentSectionBinary::Owned(source_data) => source_data.clone(),
                    FragmentSectionBinary::None => Vec::new(),
                };
                (sec.size, bin)
            } else {
                let size = merged_file_layout.get_non_empty_section_info(section_name).map(|i| i.size).unwrap_or(0);
                (size, vec![0u8; size as usize])
            };

            if binary.len() < source_size as usize {
                binary.resize(source_size as usize, 0);
            }

            if let Some(patch_items) = patch_module.patch_sections.get(&section_name) {
                for patch_item in patch_items {
                    let offset = patch_item.offset as usize;
                    match patch_item.patch_type {
                        PatchType::U32(val) => {
                            if offset + 4 <= binary.len() {
                                binary[offset..offset + 4].copy_from_slice(&val.to_le_bytes());
                            }
                        }
                        PatchType::U64(val) => {
                            if offset + 8 <= binary.len() {
                                binary[offset..offset + 8].copy_from_slice(&val.to_le_bytes());
                            }
                        }
                    }
                }
            }

            relocated_sections.insert(
                section_name,
                RelocatedSection {
                    size: source_size,
                    binary: RelocatedSectionBinary::Owned(binary),
                },
            );
        }

        relocated_modules.push(RelocatedModule {
            sections: relocated_sections,
        });
    }

    Ok(relocated_modules)
}
