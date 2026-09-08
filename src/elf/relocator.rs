// Copyright (c) 2026 Hemashushu <hippospark@gmail.com>, All rights reserved.
//
// This Source Code Form is subject to the terms of
// the Mozilla Public License version 2.0 and additional exceptions.
// For more details, see the LICENSE, LICENSE.additional, and CONTRIBUTING files.

use std::collections::HashMap;

use crate::{
    elf::{
        external_symbol_resolver::ResolvedModule,
        merger::{FragmentSectionBinary, MergedFileLayout, SectionName},
        module::Machine,
    },
    error::LinkerError,
};

mod aarch64;
mod loongarch64;
mod powerpc64le;
mod riscv64;
mod s390x;
mod x86_64;

#[derive(Debug, PartialEq)]
pub struct RelocatedModule<'a> {
    pub sections: HashMap<SectionName, RelocatedSection<'a>>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct RelocatedSection<'a> {
    /// The size of the section.
    /// For the `.bss` and `.tbss` sections, this is the memory size of the section,
    /// which is not present in the file, but occupies space in memory.
    pub size: usize,

    /// The binary data of the section.
    ///
    /// File-backed sections contain binary data. Synthetic sections, such as
    /// the static GOT, may own generated binary data; `.bss` and `.tbss` do
    /// not contain file-backed data.
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
    arch: Machine,
) -> Result<Vec<RelocatedModule<'a>>, LinkerError> {
    // Resolve relocations and generate patch modules
    let patch_modules = match arch {
        Machine::AArch64 => {
            aarch64::AArch64RelocationResolver::resolve(merged_file_layout, &resolved_modules)?
        }
        Machine::RiscV => {
            riscv64::RiscV64RelocationResolver::resolve(merged_file_layout, &resolved_modules)?
        }
        Machine::LoongArch => loongarch64::LoongArch64RelocationResolver::resolve(
            merged_file_layout,
            &resolved_modules,
        )?,
        Machine::PowerPC64 => powerpc64le::PowerPC64LERelocationResolver::resolve(
            merged_file_layout,
            &resolved_modules,
        )?,
        Machine::S390 => {
            s390x::S390xRelocationResolver::resolve(merged_file_layout, &resolved_modules)?
        }
        Machine::X86_64 => {
            x86_64::X86_64RelocationResolver::resolve(merged_file_layout, &resolved_modules)?
        }
        _ => {
            unimplemented!(
                "Relocation for architecture {} is not implemented yet",
                arch
            )
        }
    };

    // Apply the patch modules to the merged modules
    let mut relocated_modules = Vec::new();

    for (resolved_module, patch_module) in resolved_modules.into_iter().zip(patch_modules) {
        let mut relocated_sections: HashMap<SectionName, RelocatedSection<'a>> = HashMap::new();

        for (section_name, section) in resolved_module.sections {
            if let Some(patch_items) = patch_module.patch_sections.get(&section_name) {
                // If there are patch items for this section, we need to apply them to the binary data
                let mut binary = match &section.binary {
                    FragmentSectionBinary::Referenced(source_data) => source_data.to_vec(),
                    FragmentSectionBinary::Owned(source_data) => source_data.clone(),
                    FragmentSectionBinary::None => {
                        return Err(LinkerError::Message(format!(
                            "Section {} does not have binary data",
                            section_name
                        )));
                    }
                };
                for patch_item in patch_items {
                    binary.splice(
                        patch_item.offset..patch_item.offset + patch_item.data.len(),
                        patch_item.data.clone(),
                    );
                }

                relocated_sections.insert(
                    section_name,
                    RelocatedSection {
                        size: binary.len(),
                        binary: RelocatedSectionBinary::Owned(binary),
                    },
                );
            } else {
                // If there are no patch items for this section, we can directly use the original binary data
                match section.binary {
                    FragmentSectionBinary::Referenced(source_data) => {
                        relocated_sections.insert(
                            section_name,
                            RelocatedSection {
                                size: section.size,
                                binary: RelocatedSectionBinary::Referenced(source_data),
                            },
                        );
                    }
                    FragmentSectionBinary::Owned(source_data) => {
                        relocated_sections.insert(
                            section_name,
                            RelocatedSection {
                                size: section.size,
                                binary: RelocatedSectionBinary::Owned(source_data),
                            },
                        );
                    }
                    FragmentSectionBinary::None => {
                        relocated_sections.insert(
                            section_name,
                            RelocatedSection {
                                size: section.size,
                                binary: RelocatedSectionBinary::None,
                            },
                        );
                    }
                }
            }
        }

        relocated_modules.push(RelocatedModule {
            sections: relocated_sections,
        });
    }

    Ok(relocated_modules)
}

/// A patch item represents a modification to be made to a section's binary data.
///
/// Note that this linker does not support changing code size (e.g., relaxation of the RISC-V instruction set),
/// so a patch item only modifies the binary data of a section without changing its size.
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
/// This linker does not support changing code size (e.g., relaxation of the RISC-V instruction set),
/// so the relocation resolver only needs to resolve the relocation entries and generate the corresponding patch items.
pub trait RelocationResolver {
    fn resolve(
        merged_file_layout: &MergedFileLayout,
        resolved_modules: &[ResolvedModule],
    ) -> Result<Vec<PatchModule>, LinkerError>;
}

#[cfg(test)]
mod tests {

    use std::{collections::HashMap, fmt::Display};

    use crate::elf::{
        external_symbol_resolver::{ResolvedAsset, resolve},
        merger::{
            GlobalSymbolMapEntry, GlobalSymbolValue, MergedAsset, MergedFileLayout, SectionName,
            merge,
        },
        module::{Machine, RelocatableModule, get_load_address_base},
        reader::read_relocatable_module,
        relocator::relocate,
    };

    #[derive(Debug, PartialEq, Clone, Copy)]
    enum SourceType {
        Assembly,

        #[allow(dead_code)]
        #[allow(clippy::upper_case_acronyms)]
        GCC,
    }

    impl Display for SourceType {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                SourceType::Assembly => f.write_str("asm"),
                SourceType::GCC => f.write_str("gcc"),
            }
        }
    }

    const IMPLEMENTED_ARCHS: [Machine; 6] = [
        Machine::X86_64,
        Machine::AArch64,
        Machine::RiscV,
        Machine::LoongArch,
        Machine::PowerPC64,
        Machine::S390,
    ];

    fn get_arch_dir_name(arch: Machine) -> &'static str {
        match arch {
            Machine::X86_64 => "x86_64",
            Machine::AArch64 => "aarch64",
            Machine::RiscV => "riscv64",
            Machine::LoongArch => "loongarch64",
            Machine::PowerPC64 => "powerpc64le",
            Machine::S390 => "s390x",
            Machine::Other(_) => unimplemented!(),
        }
    }

    fn get_example_file_binary(source_type: SourceType, arch: Machine, file_name: &str) -> Vec<u8> {
        let file_path = std::env::current_dir()
            .unwrap()
            .join("resources/examples/elf")
            .join(source_type.to_string())
            .join(get_arch_dir_name(arch))
            .join(file_name);

        std::fs::read(file_path).unwrap()
    }

    fn get_example_file_binaries(
        source_type: SourceType,
        arch: Machine,
        file_names: &[&str],
    ) -> Vec<Vec<u8>> {
        file_names
            .iter()
            .map(|file_name| get_example_file_binary(source_type, arch, file_name))
            .collect()
    }

    fn get_example_file_module<'a>(name: &str, file_binary: &'a [u8]) -> RelocatableModule<'a> {
        read_relocatable_module(name, file_binary).unwrap()
    }

    fn get_example_file_modules<'a>(
        names: &[&str],
        file_binaries: &[&'a [u8]],
    ) -> Vec<RelocatableModule<'a>> {
        names
            .iter()
            .zip(file_binaries.iter())
            .map(|(name, file_binary)| get_example_file_module(name, file_binary))
            .collect()
    }

    fn add_additional_linker_generated_symbols(
        arch: Machine,
        linker_generated_symbols: &mut HashMap<String, GlobalSymbolMapEntry>,
        merged_file_layout: &MergedFileLayout,
    ) {
        match arch {
            Machine::RiscV => {
                linker_generated_symbols.insert(
                    "__global_pointer$".to_string(),
                    GlobalSymbolMapEntry::new(
                        GlobalSymbolValue::from_defined(SectionName::Text, 0x1000),
                        false,
                    ),
                );
            }
            Machine::PowerPC64 => {
                let toc_address = merged_file_layout
                    .get_non_empty_section_info(SectionName::TOC)
                    .map(|section| section.virtual_address)
                    .unwrap_or_else(|| get_load_address_base(arch));

                linker_generated_symbols.insert(
                    ".TOC.".to_string(),
                    GlobalSymbolMapEntry::new(
                        GlobalSymbolValue::Absolute((toc_address + 0x8000) as u64),
                        false,
                    ),
                );
            }
            _ => {
                // No additional linker-generated symbols for other architectures
            }
        }
    }

    #[test]
    fn test_relocate_minimal() {
        for arch in IMPLEMENTED_ARCHS {
            let file_binary = get_example_file_binary(SourceType::Assembly, arch, "minimal.o");
            let module = get_example_file_module("minimal.o", &file_binary);
            let modules = vec![module];

            let MergedAsset {
                fragment_modules,
                mut linker_generated_symbols,
                merged_file_layout,
            } = merge(modules, arch).unwrap();

            add_additional_linker_generated_symbols(
                arch,
                &mut linker_generated_symbols,
                &merged_file_layout,
            );

            let ResolvedAsset {
                resolved_modules,
                global_symbols: _,
            } = resolve(fragment_modules, &linker_generated_symbols).unwrap();

            let relocate_result = relocate(&merged_file_layout, resolved_modules, arch);

            assert!(relocate_result.is_ok());
        }
    }

    #[test]
    fn test_relocate_data() {
        for arch in IMPLEMENTED_ARCHS {
            let file_binary = get_example_file_binary(SourceType::Assembly, arch, "data.o");
            let module = get_example_file_module("data.o", &file_binary);
            let modules = vec![module];

            let MergedAsset {
                fragment_modules,
                mut linker_generated_symbols,
                merged_file_layout,
            } = merge(modules, arch).unwrap();

            add_additional_linker_generated_symbols(
                arch,
                &mut linker_generated_symbols,
                &merged_file_layout,
            );

            let ResolvedAsset {
                resolved_modules,
                global_symbols: _,
            } = resolve(fragment_modules, &linker_generated_symbols).unwrap();

            let relocate_result = relocate(&merged_file_layout, resolved_modules, arch);

            assert!(relocate_result.is_ok());
        }
    }

    #[test]
    fn test_relocate_symbol_export_and_import() {
        for arch in IMPLEMENTED_ARCHS {
            let file_binaries = get_example_file_binaries(
                SourceType::Assembly,
                arch,
                &["symbol-import.o", "symbol-export.o"],
            );
            let file_binaries_ref: Vec<&[u8]> =
                file_binaries.iter().map(|b| b.as_slice()).collect();
            let modules = get_example_file_modules(
                &["symbol-import.o", "symbol-export.o"],
                &file_binaries_ref,
            );

            let MergedAsset {
                fragment_modules,
                mut linker_generated_symbols,
                merged_file_layout,
            } = merge(modules, arch).unwrap();

            add_additional_linker_generated_symbols(
                arch,
                &mut linker_generated_symbols,
                &merged_file_layout,
            );

            let ResolvedAsset {
                resolved_modules,
                global_symbols: _,
            } = resolve(fragment_modules, &linker_generated_symbols).unwrap();

            let relocate_result = relocate(&merged_file_layout, resolved_modules, arch);

            assert!(relocate_result.is_ok());
        }
    }
}
