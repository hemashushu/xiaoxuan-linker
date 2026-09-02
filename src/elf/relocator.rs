// Copyright (c) 2026 Hemashushu <hippospark@gmail.com>, All rights reserved.
//
// This Source Code Form is subject to the terms of
// the Mozilla Public License version 2.0 and additional exceptions.
// For more details, see the LICENSE, LICENSE.additional, and CONTRIBUTING files.

use std::collections::HashMap;

use crate::{
    elf::{
        merger::{MergedFileLayout, MergedModule, MergedSectionBinary, SectionName},
        module::Machine,
    },
    error::LinkerError,
};

mod x86_64;

#[derive(Debug, PartialEq)]
pub struct LinkedModule<'a> {
    pub sections: HashMap<SectionName, LinkedSection<'a>>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct LinkedSection<'a> {
    /// The size of the section.
    /// For the `.bss` and `.tbss` sections, this is the memory size of the section,
    /// which is not present in the file, but occupies space in memory.
    pub size: usize,

    /// The binary data of the section.
    ///
    /// Note: only `.text`, `.rodata`, `.tdata`, and `.data` sections
    /// contain binary data in the object file,
    /// while `.bss` and `.tbss` sections do not contain binary data in the object file.
    pub binary: LinkedSectionBinary<'a>,
}

#[derive(Debug, PartialEq, Clone)]
pub enum LinkedSectionBinary<'a> {
    Owned(Vec<u8>),
    Referenced(&'a [u8]),
    None,
}

pub fn relocate<'a>(
    merged_file_layout: &MergedFileLayout,
    merged_modules: &[MergedModule<'a>],
    arch: &Machine,
) -> Result<Vec<LinkedModule<'a>>, LinkerError> {
    // Resolve relocations and generate patch modules
    let patch_modules = match arch {
        Machine::X86_64 => {
            x86_64::X86_64RelocationResolver::resolve(merged_file_layout, merged_modules)?
        }
        _ => {
            unimplemented!(
                "Relocation for architecture {:?} is not implemented yet",
                arch
            )
        }
    };

    // Apply the patch modules to the merged modules
    let mut linked_modules = Vec::new();
    for (merged_module, patch_module) in merged_modules.iter().zip(patch_modules) {
        let mut linked_sections: HashMap<SectionName, LinkedSection<'a>> = HashMap::new();

        for (section_name, section) in &merged_module.sections {
            if let Some(patch_items) = patch_module.patch_sections.get(section_name) {
                let MergedSectionBinary::Referenced(source_data) = section.binary else {
                    return Err(LinkerError::Message(format!(
                        "Section {:?} does not have a referenced binary",
                        section_name
                    )));
                };

                let mut binary = source_data.to_vec();
                for patch_item in patch_items {
                    binary.splice(
                        patch_item.offset..patch_item.offset + patch_item.data.len(),
                        patch_item.data.clone(),
                    );
                }

                linked_sections.insert(
                    *section_name,
                    LinkedSection {
                        size: binary.len(),
                        binary: LinkedSectionBinary::Owned(binary),
                    },
                );
            } else {
                match section.binary {
                    MergedSectionBinary::Referenced(source_data) => {
                        linked_sections.insert(
                            *section_name,
                            LinkedSection {
                                size: section.size,
                                binary: LinkedSectionBinary::Referenced(source_data),
                            },
                        );
                    }
                    MergedSectionBinary::None => {
                        linked_sections.insert(
                            *section_name,
                            LinkedSection {
                                size: section.size,
                                binary: LinkedSectionBinary::None,
                            },
                        );
                    }
                }
            }
        }

        linked_modules.push(LinkedModule {
            sections: linked_sections,
        });
    }

    Ok(linked_modules)
}

pub struct PatchItem {
    pub offset: usize,
    pub data: Vec<u8>,
}

impl PatchItem {
    pub fn new(offset: usize, data: Vec<u8>) -> Self {
        Self { offset, data }
    }

    pub fn from_u64(offset: usize, value: u64) -> Self {
        let data = value.to_le_bytes().to_vec();
        Self { offset, data }
    }

    pub fn from_u32(offset: usize, value: u32) -> Self {
        let data = value.to_le_bytes().to_vec();
        Self { offset, data }
    }

    pub fn from_u64_big_endian(offset: usize, value: u64) -> Self {
        let data = value.to_be_bytes().to_vec();
        Self { offset, data }
    }

    pub fn from_u32_big_endian(offset: usize, value: u32) -> Self {
        let data = value.to_be_bytes().to_vec();
        Self { offset, data }
    }
}

pub struct PatchModule {
    pub patch_sections: HashMap<SectionName, Vec<PatchItem>>,
}

/// A trait for resolving relocations in a merged module.
///
/// The linker does not support changing code size (e.g., the relaxation of the RISCV instruction set),
/// so the relocation resolver only needs to resolve the relocation entries and generate the corresponding patch items.
pub trait RelocationResolver {
    fn resolve(
        merged_file_layout: &MergedFileLayout,
        merged_modules: &[MergedModule],
    ) -> Result<Vec<PatchModule>, LinkerError>;
}
