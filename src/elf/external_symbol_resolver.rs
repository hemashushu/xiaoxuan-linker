// Copyright (c) 2026 Hemashushu <hippospark@gmail.com>, All rights reserved.
//
// This Source Code Form is subject to the terms of
// the Mozilla Public License version 2.0 and additional exceptions.
// For more details, see the LICENSE, LICENSE.additional, and CONTRIBUTING files.

use std::collections::HashMap;

use crate::{
    elf::{
        merger::{
            FragmentModule, FragmentRelocationSection, FragmentSection, GlobalSymbolMapEntry,
            GlobalSymbolValue, MergedSymbol, SectionName,
        },
        module::SymbolBind,
    },
    error::LinkerError,
};

#[derive(Debug, PartialEq)]
pub struct ResolvedModule<'a> {
    /// The identifier or name of the module, typically derived from the input file name.
    pub name: String,

    /// The relevant sections of the module
    pub sections: HashMap<SectionName, FragmentSection<'a>>,

    /// The symbol table with the external symbols resolved.
    pub symbols: Vec<ResolvedSymbol>,

    /// The relocation entries of the module, which contain the information about
    /// how to adjust the code and data when linking.
    pub relocation_sections: Vec<FragmentRelocationSection>,
}

/// Symbol represents a symbol in the merged module
#[derive(Debug, PartialEq)]
pub enum ResolvedSymbol {
    VirtualAddress(u64),
    Absolute(u64),
    Other,
}

#[derive(Debug, PartialEq)]
pub struct ResolvedAsset<'a> {
    pub resolved_modules: Vec<ResolvedModule<'a>>,
    pub global_symbols: HashMap<String, GlobalSymbolMapEntry>,
}

pub fn resolve<'a>(
    fragment_modules: Vec<FragmentModule<'a>>,

    // A predefined global symbol map
    //
    // This map contains linker-generated symbols such as `_edata` and `__bss_start`,
    // and user-defined symbols such as `__global_pointer$` in RISC-V,
    // which are required to successfully link the final executable.
    linker_generated_symbols: &HashMap<String, GlobalSymbolMapEntry>,
) -> Result<ResolvedAsset<'a>, LinkerError> {
    let mut global_symbols = linker_generated_symbols.clone();

    // Extract global symbols from all modules
    for fragment_module in &fragment_modules {
        for merged_symbol in &fragment_module.symbols {
            if let MergedSymbol::Defined {
                name,
                bind,
                section_name,
                virtual_address,
                ..
            } = merged_symbol
            {
                match bind {
                    SymbolBind::Global => {
                        if global_symbols.contains_key(name) {
                            return Err(LinkerError::Message(format!(
                                "Duplicate global symbol \"{}\" defined in module \"{}\"",
                                name, fragment_module.name
                            )));
                        }

                        let value =
                            GlobalSymbolValue::from_defined(*section_name, *virtual_address);

                        global_symbols
                            .insert(name.clone(), GlobalSymbolMapEntry::new(value, false));
                    }
                    SymbolBind::Weak if !global_symbols.contains_key(name) => {
                        // Append weak symbols to the global symbol map only if they are not already present,
                        // no matter if they are strong or weak symbols, the first one wins.

                        let value =
                            GlobalSymbolValue::from_defined(*section_name, *virtual_address);

                        global_symbols.insert(name.clone(), GlobalSymbolMapEntry::new(value, true));
                    }
                    _ => {
                        // Local symbols are not added to the global symbol map
                    }
                }
            }
        }
    }

    // Resolve the external symbol in MergedSymbol and generate ResolvedSymbol
    let mut resolved_symbolss: Vec<Vec<ResolvedSymbol>> = vec![];

    for fragment_module in &fragment_modules {
        let mut resolved_symbols = Vec::new();

        for merged_symbol in &fragment_module.symbols {
            match merged_symbol {
                MergedSymbol::Defined {
                    virtual_address, ..
                } => {
                    let resolved_symbol = ResolvedSymbol::VirtualAddress(*virtual_address);
                    resolved_symbols.push(resolved_symbol);
                }
                MergedSymbol::Absolute { value, .. } => {
                    let resolved_symbol = ResolvedSymbol::Absolute(*value);
                    resolved_symbols.push(resolved_symbol);
                }
                MergedSymbol::External(name) => {
                    // Look up the symbol in the global symbol map
                    if let Some(global_symbol) = global_symbols.get(name) {
                        match global_symbol.value {
                            GlobalSymbolValue::Defined {
                                virtual_address, ..
                            } => {
                                let resolved_symbol =
                                    ResolvedSymbol::VirtualAddress(virtual_address);
                                resolved_symbols.push(resolved_symbol);
                            }
                            GlobalSymbolValue::Absolute(value) => {
                                let resolved_symbol = ResolvedSymbol::Absolute(value);
                                resolved_symbols.push(resolved_symbol);
                            }
                        }
                    } else {
                        return Err(LinkerError::Message(format!(
                            "Unresolved external symbol \"{}\" in module \"{}\"",
                            name, fragment_module.name
                        )));
                    }
                }
                MergedSymbol::Other => {
                    resolved_symbols.push(ResolvedSymbol::Other);
                }
            }
        }
        resolved_symbolss.push(resolved_symbols);
    }

    let resolved_modules = fragment_modules
        .into_iter()
        .zip(resolved_symbolss)
        .map(|(fragment_module, resolved_symbols)| ResolvedModule {
            name: fragment_module.name,
            sections: fragment_module.sections,
            symbols: resolved_symbols,
            relocation_sections: fragment_module.relocation_sections,
        })
        .collect::<Vec<ResolvedModule>>();

    let resolved_asset = ResolvedAsset {
        resolved_modules,
        global_symbols,
    };

    Ok(resolved_asset)
}

pub fn find_entry_point(
    global_symbols: &HashMap<String, GlobalSymbolMapEntry>,
) -> Result<u64, LinkerError> {
    if let Some(entry_symbol) = global_symbols.get("_start") {
        match entry_symbol.value {
            GlobalSymbolValue::Defined {
                virtual_address, ..
            } => Ok(virtual_address),
            GlobalSymbolValue::Absolute(value) => Ok(value),
        }
    } else {
        Err(LinkerError::Message(
            "Entry point symbol \"_start\" not found in the global symbols".to_string(),
        ))
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {

    use pretty_assertions::assert_eq;
    use std::{collections::HashMap, fmt::Display};

    use crate::elf::{
        external_symbol_resolver::resolve,
        merger::{
            GlobalSymbolMapEntry, GlobalSymbolValue, MergedAsset, MergedFileLayout, SectionName,
            merge,
        },
        module::{Machine, RelocatableModule, get_load_address_base},
        reader::read_relocatable_module,
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
                        GlobalSymbolValue::Absolute(toc_address + 0x8000),
                        false,
                    ),
                );
            }
            _ => {
                // No additional linker-generated symbols for other architectures
            }
        }
    }

    fn assert_contains_all(strs: &[&str], expected: &[&str]) {
        for &expected_str in expected {
            assert!(
                strs.contains(&expected_str),
                "Expected string '{}' not found in the list: {:?}",
                expected_str,
                strs
            );
        }
    }

    #[test]
    fn test_merge_symbol_import_and_export() {
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

            let merged_asset_result = merge(modules, arch);
            assert!(merged_asset_result.is_ok());

            let MergedAsset {
                fragment_modules: merged_modules,
                mut linker_generated_symbols,
                merged_file_layout,
            } = merged_asset_result.unwrap();

            add_additional_linker_generated_symbols(
                arch,
                &mut linker_generated_symbols,
                &merged_file_layout,
            );

            let resolved_asset_result = resolve(merged_modules, &linker_generated_symbols);
            assert!(resolved_asset_result.is_ok());

            let resolved_asset = resolved_asset_result.unwrap();

            // Check the merged modules
            let merged_modules = &resolved_asset.resolved_modules;
            assert_eq!(merged_modules.len(), 2);

            // Check the linker-generated symbols
            let global_symbol_map = &resolved_asset.global_symbols;
            let keys = global_symbol_map
                .keys()
                .map(|s| s.as_str())
                .collect::<Vec<_>>();
            assert_contains_all(&keys, &["a", "b", "x", "y", "foo", "bar", "inc", "dec"]);
        }
    }
}
