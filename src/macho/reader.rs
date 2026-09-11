// Copyright (c) 2026 Hemashushu <hippospark@gmail.com>, All rights reserved.
//
// This Source Code Form is subject to the terms of
// the Mozilla Public License version 2.0 and additional exceptions.
// For more details, see the LICENSE, LICENSE.additional, and CONTRIBUTING files.

use std::str;

use object::macho::MachHeader64;
use object::read::macho::{MachHeader, Nlist, Section, Segment, SymbolTable};

use object::Endianness;

use crate::error::LinkerError;
use crate::macho::module::{
    CpuType, FileHeader, FileType, RelocatableModule, Relocation as ModuleRelocation,
    RelocationSection, RelocationType, SectionHeader, SectionType, Symbol, SymbolBind, SymbolType,
};

/// Parse raw binary data into a Mach-O 64-bit header.
pub fn read_file<'a>(
    module_name: &str,
    binary: &'a [u8],
) -> Result<&'a MachHeader64<Endianness>, LinkerError> {
    let Ok(header) = MachHeader64::<Endianness>::parse(binary, 0) else {
        return Err(LinkerError::new(&format!(
            "Failed to parse Mach-O 64-bit module: {}",
            module_name
        )));
    };

    Ok(header)
}

/// Read the file header information from a Mach-O 64-bit header.
pub fn read_file_header(
    module_name: &str,
    header: &MachHeader64<Endianness>,
) -> Result<FileHeader, LinkerError> {
    let Ok(endian) = header.endian() else {
        return Err(LinkerError::new(&format!(
            "Failed to determine endianness for module: {}",
            module_name
        )));
    };

    let cpu_type = CpuType::from(header.cputype(endian));
    let cpu_subtype = header.cpusubtype(endian).0;
    let file_type = FileType::from(header.filetype(endian));
    let load_commands_count = header.ncmds(endian) as usize;
    let load_commands_size = header.sizeofcmds(endian);
    let flags = header.flags(endian).0;
    let reserved = header.reserved.get(endian);

    Ok(FileHeader {
        cpu_type,
        cpu_subtype,
        file_type,
        load_commands_count,
        load_commands_size,
        flags,
        reserved,
    })
}

/// Read section headers from a Mach-O 64-bit file.
pub fn read_section_headers<'a>(
    module_name: &str,
    header: &'a MachHeader64<Endianness>,
    binary: &'a [u8],
) -> Result<Vec<SectionHeader<'a>>, LinkerError> {
    let Ok(endian) = header.endian() else {
        return Err(LinkerError::new(&format!(
            "Failed to determine endianness for module: {}",
            module_name
        )));
    };

    let mut sections = Vec::new();

    let Ok(mut load_commands) = header.load_commands(endian, binary, 0) else {
        return Err(LinkerError::new(&format!(
            "Failed to read load commands for module: {}",
            module_name
        )));
    };

    while let Ok(Some(command)) = load_commands.next() {
        if let Ok(Some((segment_cmd, section_data))) = command.segment_64() {
            let seg_name_bytes = segment_cmd.name();
            let seg_name = str::from_utf8(seg_name_bytes)
                .unwrap_or("")
                .trim_matches('\0')
                .to_string();

            let Ok(section_table) = segment_cmd.sections(endian, section_data) else {
                continue;
            };

            for sect in section_table.iter() {
                let sect_name_bytes = sect.name();
                let sect_name = str::from_utf8(sect_name_bytes)
                    .unwrap_or("")
                    .trim_matches('\0')
                    .to_string();

                let seg_name_for_sect_bytes = sect.segment_name();
                let seg_name_for_sect_str = str::from_utf8(seg_name_for_sect_bytes)
                    .unwrap_or("")
                    .trim_matches('\0');
                let seg_name_for_sect = if seg_name_for_sect_str.is_empty() {
                    seg_name.clone()
                } else {
                    seg_name_for_sect_str.to_string()
                };

                let virtual_address = sect.addr(endian);
                let size = sect.size(endian);
                let offset = sect.offset(endian);
                let align_pow = sect.align(endian);
                let align = 1u64 << align_pow;
                let relocation_offset = sect.reloff(endian);
                let relocation_count = sect.nreloc(endian);
                let flags = sect.flags(endian).0;
                let section_type = SectionType::from((flags & 0xff) as u8);

                let offset_u64 = offset as u64;
                let data = binary
                    .get(offset_u64 as usize..(offset_u64 + size) as usize)
                    .unwrap_or(&[]);

                sections.push(SectionHeader {
                    segment_name: seg_name_for_sect,
                    section_name: sect_name,
                    virtual_address,
                    size,
                    offset: offset,
                    align,
                    relocation_offset,
                    relocation_count,
                    flags,
                    section_type,
                    binary: data,
                });
            }
        }
    }

    Ok(sections)
}

/// Read symbol table entries from a Mach-O 64-bit file.
pub fn read_symbols(
    module_name: &str,
    header: &MachHeader64<Endianness>,
    binary: &[u8],
) -> Result<Vec<Symbol>, LinkerError> {
    let Ok(endian) = header.endian() else {
        return Err(LinkerError::new(&format!(
            "Failed to determine endianness for module: {}",
            module_name
        )));
    };

    let Ok(mut load_commands) = header.load_commands(endian, binary, 0) else {
        return Err(LinkerError::new(&format!(
            "Failed to read load commands for module: {}",
            module_name
        )));
    };

    while let Ok(Some(command)) = load_commands.next() {
        if let Ok(Some(symtab_command)) = command.symtab() {
            let Ok(symtab) = symtab_command.symbols::<MachHeader64<Endianness>, _>(endian, binary)
            else {
                return Err(LinkerError::new(&format!(
                    "Failed to parse symbol table for module: {}",
                    module_name
                )));
            };

            return parse_symbol_table(&symtab, endian);
        }
    }

    Ok(Vec::new())
}

fn parse_symbol_table(
    symtab: &SymbolTable<MachHeader64<Endianness>>,
    endian: Endianness,
) -> Result<Vec<Symbol>, LinkerError> {
    let string_table = symtab.strings();
    let mut symbols = Vec::new();

    for sym in symtab.iter() {
        let name_bytes = sym.name(endian, string_table).unwrap_or(b"");
        let name = str::from_utf8(name_bytes).unwrap_or("").to_string();

        let n_type = sym.n_type().0;
        let n_sect = sym.n_sect();
        let n_desc = sym.n_desc(endian).0;
        let value = sym.n_value(endian);

        let type_type = n_type & object::macho::N_TYPE;
        let is_ext = (n_type & object::macho::N_EXT.0) != 0;
        let is_stab = (n_type & 0xe0) != 0;

        if is_stab {
            symbols.push(Symbol::Other);
            continue;
        }

        let bind = if (n_desc & object::macho::N_WEAK_DEF.0) != 0
            || (n_desc & object::macho::N_WEAK_REF.0) != 0
        {
            SymbolBind::Weak
        } else if is_ext {
            SymbolBind::Global
        } else {
            SymbolBind::Local
        };

        let symbol = match type_type {
            0 => {
                if is_ext {
                    Symbol::External(name)
                } else if value == 0 {
                    Symbol::Null
                } else {
                    Symbol::Other
                }
            }
            0x02 => Symbol::Absolute { name, bind, value },
            0x0e => {
                let symbol_type = if (n_desc & object::macho::N_ARM_THUMB_DEF.0) != 0 {
                    SymbolType::Func
                } else {
                    SymbolType::Notype
                };

                Symbol::Defined {
                    name,
                    section_index: n_sect as usize,
                    bind,
                    symbol_type,
                    value,
                }
            }
            0x0c => Symbol::Other,
            0x0a => Symbol::Other,
            _ => Symbol::Other,
        };

        symbols.push(symbol);
    }

    Ok(symbols)
}

/// Read relocation sections from a Mach-O 64-bit file.
pub fn read_relocation_sections(
    module_name: &str,
    header: &MachHeader64<Endianness>,
    binary: &[u8],
) -> Result<Vec<RelocationSection>, LinkerError> {
    let Ok(endian) = header.endian() else {
        return Err(LinkerError::new(&format!(
            "Failed to determine endianness for module: {}",
            module_name
        )));
    };

    let mut relocation_sections = Vec::new();
    let sections = read_section_headers(module_name, header, binary)?;

    for (sect_idx, section) in sections.iter().enumerate() {
        if section.relocation_count == 0 {
            continue;
        }

        let reloff = section.relocation_offset as usize;

        if reloff == 0 || reloff >= binary.len() {
            continue;
        }

        let Ok(mut load_commands) = header.load_commands(endian, binary, 0) else {
            continue;
        };

        while let Ok(Some(command)) = load_commands.next() {
            if let Ok(Some((segment_cmd, section_data))) = command.segment_64() {
                let Ok(section_table) = segment_cmd.sections(endian, section_data) else {
                    continue;
                };

                for sect in section_table.iter() {
                    let sect_reloff = sect.reloff(endian);
                    if sect_reloff == section.relocation_offset {
                        if let Ok(relocs) = sect.relocations(endian, binary) {
                            let mut parsed_relocs = Vec::new();
                            for raw_reloc in relocs {
                                let info = raw_reloc.info(endian);
                                let is_pcrel = info.r_pcrel;
                                let length = 1 << info.r_length;
                                let is_extern = info.r_extern;
                                let symbol_index = info.r_symbolnum as usize;
                                let r_type = info.r_type.0;
                                let offset = raw_reloc.r_word0.get(endian) as u64;

                                let relocation_type = RelocationType::from(r_type);

                                parsed_relocs.push(ModuleRelocation {
                                    relocation_type,
                                    offset,
                                    is_pcrel,
                                    length,
                                    is_extern,
                                    symbol_index,
                                    addend: 0,
                                });
                            }

                            relocation_sections.push(RelocationSection {
                                name: format!("{},{}", section.segment_name, section.section_name),
                                target_section_index: sect_idx + 1, // 1-based indexing for Mach-O sections
                                relocations: parsed_relocs,
                            });
                        }
                        break;
                    }
                }
            }
        }
    }

    Ok(relocation_sections)
}

/// Read a Mach-O relocatable module (`MH_OBJECT`).
pub fn read_relocatable_module<'a>(
    name: &str,
    binary: &'a [u8],
) -> Result<RelocatableModule<'a>, LinkerError> {
    let header = read_file(name, binary)?;
    let file_header = read_file_header(name, header)?;

    if file_header.file_type != FileType::Object {
        return Err(LinkerError::new(&format!(
            "Unsupported Mach-O type for module: {}, expected relocatable (MH_OBJECT) file",
            name
        )));
    }

    if file_header.cpu_type != CpuType::ARM64 {
        return Err(LinkerError::new(&format!(
            "Unsupported CPU architecture: {} for module: {}, expected ARM64",
            file_header.cpu_type, name
        )));
    }

    let sections = read_section_headers(name, header, binary)?;
    let symbols = read_symbols(name, header, binary)?;
    let relocation_sections = read_relocation_sections(name, header, binary)?;

    Ok(RelocatableModule {
        name: name.to_string(),
        header: file_header,
        sections,
        symbols,
        relocation_sections,
    })
}

#[cfg(all(target_os = "macos", test))]
mod tests {
    use super::*;

    fn get_clang_example_binary(file_name: &str) -> Vec<u8> {
        let file_path = std::env::current_dir()
            .unwrap()
            .join("resources/examples/mach-o/clang/aarch64")
            .join(file_name);

        std::fs::read(file_path).unwrap()
    }

    #[test]
    fn test_read_file_header_minimal_o() {
        let binary = get_clang_example_binary("minimal.o");
        let header = read_file("minimal.o", &binary).unwrap();
        let file_header = read_file_header("minimal.o", header).unwrap();

        assert_eq!(file_header.file_type, FileType::Object);
        assert_eq!(file_header.cpu_type, CpuType::ARM64);
    }

    #[test]
    fn test_read_file_header_minimal_macho() {
        let binary = get_clang_example_binary("minimal.macho");
        let header = read_file("minimal.macho", &binary).unwrap();
        let file_header = read_file_header("minimal.macho", header).unwrap();

        assert_eq!(file_header.file_type, FileType::Executable);
        assert_eq!(file_header.cpu_type, CpuType::ARM64);
    }

    #[test]
    fn test_read_sections_minimal_o() {
        let binary = get_clang_example_binary("minimal.o");
        let header = read_file("minimal.o", &binary).unwrap();
        let sections = read_section_headers("minimal.o", header, &binary).unwrap();

        assert!(!sections.is_empty());
        let text_section = sections
            .iter()
            .find(|s| s.section_name == "__text")
            .expect("Should contain __text section");
        assert_eq!(text_section.segment_name, "__TEXT");
        assert_eq!(text_section.section_type, SectionType::Regular);
    }

    #[test]
    fn test_read_sections_data_o() {
        let binary = get_clang_example_binary("data.o");
        let header = read_file("data.o", &binary).unwrap();
        let sections = read_section_headers("data.o", header, &binary).unwrap();

        let section_names: Vec<&str> = sections.iter().map(|s| s.section_name.as_str()).collect();
        assert!(section_names.contains(&"__text"));
        assert!(section_names.contains(&"__data") || section_names.contains(&"__const"));
    }

    #[test]
    fn test_read_symbols_data_o() {
        let binary = get_clang_example_binary("data.o");
        let header = read_file("data.o", &binary).unwrap();
        let symbols = read_symbols("data.o", header, &binary).unwrap();

        let defined_names: Vec<&str> = symbols
            .iter()
            .filter_map(|s| match s {
                Symbol::Defined { name, .. } => Some(name.as_str()),
                _ => None,
            })
            .collect();

        assert!(defined_names.contains(&"_foo"));
        assert!(defined_names.contains(&"_bar"));
        assert!(defined_names.contains(&"_a"));
        assert!(defined_names.contains(&"_b"));
    }

    #[test]
    fn test_read_symbols_symbol_import_o() {
        let binary = get_clang_example_binary("symbol-import.o");
        let header = read_file("symbol-import.o", &binary).unwrap();
        let symbols = read_symbols("symbol-import.o", header, &binary).unwrap();

        let external_names: Vec<&str> = symbols
            .iter()
            .filter_map(|s| match s {
                Symbol::External(name) => Some(name.as_str()),
                _ => None,
            })
            .collect();

        assert!(external_names.contains(&"_foo"));
        assert!(external_names.contains(&"_bar"));
        assert!(external_names.contains(&"_dec"));
        assert!(external_names.contains(&"_inc"));
    }

    #[test]
    fn test_read_relocations_relocate_within_data_o() {
        let binary = get_clang_example_binary("relocate-within-data.o");
        let header = read_file("relocate-within-data.o", &binary).unwrap();
        let relocation_sections =
            read_relocation_sections("relocate-within-data.o", header, &binary).unwrap();

        assert!(!relocation_sections.is_empty());
        let total_relocs: usize = relocation_sections
            .iter()
            .map(|rs| rs.relocations.len())
            .sum();
        assert!(total_relocs > 0);
    }

    #[test]
    fn test_read_relocatable_module_minimal_o() {
        let binary = get_clang_example_binary("minimal.o");
        let module = read_relocatable_module("minimal.o", &binary);
        assert!(module.is_ok());

        let module = module.unwrap();
        assert_eq!(module.name, "minimal.o");
        assert_eq!(module.header.file_type, FileType::Object);
    }

    #[test]
    fn test_read_relocatable_module_data_o() {
        let binary = get_clang_example_binary("data.o");
        let module = read_relocatable_module("data.o", &binary);
        assert!(module.is_ok());

        let module = module.unwrap();
        assert_eq!(module.name, "data.o");
        assert_eq!(module.header.file_type, FileType::Object);
    }
}
