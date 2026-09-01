// Copyright (c) 2026 Hemashushu <hippospark@gmail.com>, All rights reserved.
//
// This Source Code Form is subject to the terms of
// the Mozilla Public License version 2.0 and additional exceptions.
// For more details, see the LICENSE, LICENSE.additional, and CONTRIBUTING files.

use std::collections::HashMap;

use crate::{
    elf::{
        merger::{MergedModule, SectionName},
        module::Machine,
    },
    error::LinkerError,
};

mod x86_64;

pub fn relocate(merged_modules: &mut [MergedModule], arch: &Machine) -> Result<(), LinkerError> {
    // Resolve relocations and generate patch modules
    let patch_modules = match arch {
        Machine::X86_64 => x86_64::X86_64RelocationResolver::resolve(merged_modules)?,
        _ => {
            unimplemented!(
                "Relocation for architecture {:?} is not implemented yet",
                arch
            )
        }
    };

    // Apply the patch modules to the merged modules
    for (merged_module, patch_module) in merged_modules.iter_mut().zip(patch_modules) {
        for (section_name, patch_items) in patch_module.patch_sections {
            if let Some(section) = merged_module.sections.get_mut(&section_name) {
                for patch_item in patch_items {
                    if let Some(binary) = &mut section.binary {
                        binary.splice(
                            patch_item.offset..patch_item.offset + patch_item.data.len(),
                            patch_item.data,
                        );
                    }
                }
            }
        }
    }
    Ok(())
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
    fn resolve(merged_modules: &mut [MergedModule]) -> Result<Vec<PatchModule>, LinkerError>;
}
