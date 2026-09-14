// Copyright (c) 2026 Hemashushu <hippospark@gmail.com>, All rights reserved.
//
// This Source Code Form is subject to the terms of
// the Mozilla Public License version 2.0 and additional exceptions.
// For more details, see the LICENSE, LICENSE.additional, and CONTRIBUTING files.

use std::collections::HashMap;

use crate::{
    error::LinkerError,
    macho::{
        merger::{
            FragmentModule, FragmentRelocationSection, FragmentSection, GlobalSymbolMapEntry,
            GlobalSymbolValue, MergedSymbol, SectionName,
        },
        module::SymbolBind,
    },
};

#[derive(Debug, PartialEq)]
pub struct ResolvedModule<'a> {
    pub name: String,
    pub sections: HashMap<SectionName, FragmentSection<'a>>,
    pub symbols: Vec<ResolvedSymbol>,
    pub relocation_sections: Vec<FragmentRelocationSection>,
    pub got_offsets: HashMap<usize, u64>,
    pub section_ordinals: HashMap<usize, SectionName>,
}

#[derive(Debug, PartialEq, Clone)]
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
    linker_generated_symbols: &HashMap<String, GlobalSymbolMapEntry>,
) -> Result<ResolvedAsset<'a>, LinkerError> {
    let mut global_symbols = linker_generated_symbols.clone();

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
                        let value =
                            GlobalSymbolValue::from_defined(*section_name, *virtual_address);
                        global_symbols.insert(name.clone(), GlobalSymbolMapEntry::new(value, true));
                    }
                    _ => {}
                }
            }
        }
    }

    let mut resolved_symbolss: Vec<Vec<ResolvedSymbol>> = vec![];

    for fragment_module in &fragment_modules {
        let mut resolved_symbols = Vec::new();

        for merged_symbol in &fragment_module.symbols {
            match merged_symbol {
                MergedSymbol::Defined {
                    virtual_address, ..
                } => {
                    resolved_symbols.push(ResolvedSymbol::VirtualAddress(*virtual_address));
                }
                MergedSymbol::Absolute { value, .. } => {
                    resolved_symbols.push(ResolvedSymbol::Absolute(*value));
                }
                MergedSymbol::External(name) => {
                    if let Some(global_symbol) = global_symbols.get(name) {
                        match global_symbol.value {
                            GlobalSymbolValue::Defined {
                                virtual_address, ..
                            } => {
                                resolved_symbols
                                    .push(ResolvedSymbol::VirtualAddress(virtual_address));
                            }
                            GlobalSymbolValue::Absolute(value) => {
                                resolved_symbols.push(ResolvedSymbol::Absolute(value));
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
            got_offsets: fragment_module.got_offsets,
            section_ordinals: fragment_module.section_ordinals,
        })
        .collect::<Vec<ResolvedModule>>();

    Ok(ResolvedAsset {
        resolved_modules,
        global_symbols,
    })
}

pub fn find_entry_point(
    global_symbols: &HashMap<String, GlobalSymbolMapEntry>,
) -> Result<u64, LinkerError> {
    if let Some(entry_symbol) = global_symbols.get("_main") {
        match entry_symbol.value {
            GlobalSymbolValue::Defined {
                virtual_address, ..
            } => Ok(virtual_address),
            GlobalSymbolValue::Absolute(value) => Ok(value),
        }
    } else {
        Err(LinkerError::Message(
            "Entry point symbol \"_main\" not found in global symbols".to_string(),
        ))
    }
}
