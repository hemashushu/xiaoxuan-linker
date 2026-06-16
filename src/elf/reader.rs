// Copyright (c) 2026 Hemashushu <hippospark@gmail.com>, All rights reserved.
//
// This Source Code Form is subject to the terms of
// the Mozilla Public License version 2.0 and additional exceptions.
// For more details, see the LICENSE, LICENSE.additional, and CONTRIBUTING files.

use object::{
    Endianness,
    elf::FileHeader64,
    read::elf::{FileHeader, ProgramHeader, Rela, SectionHeader, Sym, SymbolTable},
};

use crate::{
    elf::module::{
        FileType, Machine, OSABI, Relocation, RelocationType, SectionType, SegmentFlag,
        SegmentType, Symbol, SymbolBind, SymbolType,
    },
    error::LinkerError,
};

pub fn read_file(binary: &[u8]) -> Result<&FileHeader64<Endianness>, LinkerError> {
    let Ok(elf) = object::elf::FileHeader64::<object::Endianness>::parse(binary) else {
        return Err(LinkerError::new("Failed to parse ELF64 file"));
    };

    Ok(elf)
}

pub fn read_file_header(
    elf: &FileHeader64<Endianness>,
) -> Result<super::module::FileHeader, LinkerError> {
    let Ok(endian) = elf.endian() else {
        return Err(LinkerError::new("Failed to determine endianness"));
    };

    let os_abi = OSABI::from(elf.e_ident.os_abi);
    let file_type = FileType::from(elf.e_type(endian));
    let machine = Machine::from(elf.e_machine(endian));

    let entry_point = elf.e_entry(endian) as usize;
    let number_of_program_headers = elf.e_phnum(endian) as usize;
    let number_of_section_headers = elf.e_shnum(endian) as usize;

    Ok(super::module::FileHeader {
        os_abi,
        machine,
        file_type,
        entry_point,
        number_of_program_headers,
        number_of_section_headers,
    })
}

pub fn read_section_headers<'a>(
    elf: &'a FileHeader64<Endianness>,
    binary: &'a [u8],
) -> Result<Vec<super::module::SectionHeader<'a>>, LinkerError> {
    let Ok(endian) = elf.endian() else {
        return Err(LinkerError::new("Failed to determine endianness"));
    };

    let Ok(section_table) = elf.sections(endian, binary) else {
        return Err(LinkerError::new("Failed to read section headers"));
    };

    let mut sections = vec![];

    for (_section_index, section_header) in section_table.enumerate() {
        // The section name is stored in the section header string table (shstrtab),
        // and the index of the section name in the shstrtab is given by the `sh_name` field in the section header.
        let section_name =
            str::from_utf8(section_table.section_name(endian, section_header).unwrap()).unwrap();

        let offset = section_header.sh_offset(endian) as usize;
        let size = section_header.sh_size(endian) as usize;
        let align = section_header.sh_addralign(endian) as usize;

        // Common section type (sh_type) includes:
        // - object::elf::SHT_NULL => "NULL"
        // - object::elf::SHT_PROGBITS => "PROGBITS"
        // - object::elf::SHT_SYMTAB => "SYMTAB"
        // - object::elf::SHT_STRTAB => "STRTAB"
        // - object::elf::SHT_RELA => "RELA"
        // - object::elf::SHT_NOBITS => "NOBITS"
        let section_type = SectionType::from(section_header.sh_type(endian));

        // let section_tls = (section_header.sh_flags(endian) as u32) & object::elf::SHF_TLS != 0;
        let binary = section_header.data(endian, binary).unwrap();

        let section = super::module::SectionHeader {
            name: section_name.to_string(),
            offset,
            size,
            align,
            section_type,
            binary,
        };

        sections.push(section);
    }

    Ok(sections)
}

pub fn read_symbols(
    elf: &FileHeader64<Endianness>,
    binary: &[u8],
) -> Result<Vec<Symbol>, LinkerError> {
    let Ok(endian) = elf.endian() else {
        return Err(LinkerError::new("Failed to determine endianness"));
    };

    let Ok(section_table) = elf.sections(endian, binary) else {
        return Err(LinkerError::new("Failed to read section headers"));
    };

    for (section_index, section_header) in section_table.enumerate() {
        let section_type = section_header.sh_type(endian);

        if section_type == object::elf::SHT_SYMTAB {
            // There are two similar symbol table sections:
            // - SHT_SYMTAB: for linkers (development tools)
            // - SHT_DYNSYM: for dynamic linking (loader)
            //
            // In general, one relocatable object file (ET_REL) only has one symbol table section (SHT_SYMTAB),
            // which name is usually `.symtab`.

            let Ok(Some(symbol_table)) =
                section_header.symbols(endian, binary, &section_table, section_index)
            else {
                return Err(LinkerError::new("Failed to read symbol table"));
            };

            // There are two useful fields in the symbol table section header:
            // - `sh_link`: it gives the index of the string table section (`.strtab`) linked by
            //   the symbol table section, and the symbol names are stored in the strtab.
            // - `sh_info`: it indicates the number of local symbols in the symbol table (or,
            //   the index of the first global symbol in the symbol table)
            //
            // But we don't need these two fields because the library we use (`object` crate)
            // has already provided the `symbol_table.strings()` method to obtain the string table,
            // and the number of local symbols can be counted by iterating over the `Vec<Symbol>`.
            let symbols = parse_symbol_table(&symbol_table, endian)?;
            return Ok(symbols);
        }
    }

    Err(LinkerError::new("Failed to find symbol table"))
}

fn parse_symbol_table(
    symbol_table: &SymbolTable<object::elf::FileHeader64<Endianness>>,
    endian: Endianness,
) -> Result<Vec<Symbol>, LinkerError> {
    // Symbols Example
    //
    //  Local symbols (not visible outside the file):
    //
    // | Index | Address          | Type   | Bind   | Section Index | Name        |
    // |-------|------------------|--------|--------|---------------|-------------|
    // | 0     | 0000000000000000 | NOTYPE | LOCAL  | UND           |             |
    // | 1     | 0000000000000000 | FILE   | LOCAL  | ABS           | hello.asm   |
    // | 2     | 0000000000402000 | NOTYPE | LOCAL  | 2             | msg         |
    // | 3     | 0000000000402007 | NOTYPE | LOCAL  | 2             | len         |
    //
    // Global symbols (visible outside the file):
    //
    // | Index | Address          | Type   | Bind   | Section Index | Name        |
    // |-------|------------------|--------|--------|---------------|-------------|
    // | 4     | 0000000000401000 | NOTYPE | GLOBAL | 1             | _start      |
    // | 5     | 000000000040300f | NOTYPE | GLOBAL | 2             | __bss_start |
    // | 6     | 000000000040300f | NOTYPE | GLOBAL | 2             | _edata      |
    // | 7     | 0000000000403010 | NOTYPE | GLOBAL | 2             | _end        |

    let string_table = symbol_table.strings();
    let mut symbols = Vec::new();

    for (symbol_index, sym) in symbol_table.enumerate() {
        // The symbol name is stored in the string table (strtab) linked by the symbol table section,
        // and the index of the symbol name in the strtab is given by the `st_name` field in the symbol table entry.
        //
        // Most common section has a corresponding symbol in the symbol table,
        // and the symbol name in the symbol table is empty.
        let symbol_name = str::from_utf8(sym.name(endian, string_table).unwrap()).unwrap();

        // The `st_shndx` field indicates the section index of the symbol definition:
        // - If `st_shndx` is a valid section index, it indicates the section where the symbol is defined, and the symbol value is the offset within that section.
        // - If `st_shndx` is `SHN_UNDEF`, it indicates an undefined symbol, which is referenced but not defined in the module, and the symbol value is 0.
        let section_index = sym.st_shndx(endian);
        let symbol = match section_index {
            object::elf::SHN_UNDEF if symbol_index.0 == 0 => {
                // The first symbol table entry (index 0) is reserved and must be undefined.
                Symbol::Other
            }
            object::elf::SHN_UNDEF => {
                // External symbol
                Symbol::External(symbol_name.to_string())
            }
            _ if section_index >= object::elf::SHN_LORESERVE => {
                // Other section index, such as `SHN_ABS` (absolute symbol) and
                // `SHN_COMMON` (common symbol), or an invalid section index.
                Symbol::Other
            }
            _ => {
                // The `st_info` field encodes both the symbol bind and type:
                // - high 4 bits is the bind (e.g. STB_GLOBAL, STB_LOCAL, and STB_WEAK).
                // - low 4 bits is the type (e.g. STT_FUNC, STT_OBJECT, STT_SECTION, STT_FILE, and STT_COMMON).
                //
                // Obtains symbol bind and type from the `st_info` field:
                //
                // ```rust
                // let info = symbol.st_info();
                // let symbol_bind = info >> 4;
                // let symbol_type = info & 0x0f;
                // ```
                //
                // Or using `symbol` trait methods:
                //
                // ```rust
                // symbol.st_bind(),
                // symbol.st_type()
                // ```
                let bind = SymbolBind::from(sym.st_bind());

                // Common symbol type (st_type) includes:
                // - object::elf::STT_NOTYPE => "NOTYPE"
                // - object::elf::STT_OBJECT => "OBJECT"
                // - object::elf::STT_FUNC => "FUNC"
                // - object::elf::STT_SECTION => "SECTION"
                // - object::elf::STT_FILE => "FILE"
                // - object::elf::STT_COMMON => "COMMON"
                //
                // Since the symbol type does not affect the linking process in this linker,
                // we don't need to check the symbol type, but we can print it for debugging purposes.
                let symbol_type = SymbolType::from(sym.st_type());

                // The low 2 bits of the `st_other` field encode the symbol visibility:
                // - STV_DEFAULT: the symbol is visible to all modules.
                // - STV_INTERNAL: the symbol is visible only within the module.
                // - STV_HIDDEN: the symbol is hidden from other modules.
                // - STV_PROTECTED: the symbol is visible to other modules but cannot be overridden.
                //
                // This linker only supports STV_DEFAULT, which is the default visibility for symbols.

                let offset = sym.st_value(endian) as usize;

                Symbol::Defined {
                    name: symbol_name.to_string(),
                    section_index: section_index as usize,
                    bind,
                    symbol_type,
                    offset,
                }
            }
        };

        symbols.push(symbol);
    }

    Ok(symbols)
}

pub fn read_relocation_sections(
    elf: &FileHeader64<Endianness>,
    binary: &[u8],
) -> Result<Vec<super::module::RelocationSection>, LinkerError> {
    let Ok(endian) = elf.endian() else {
        return Err(LinkerError::new("Failed to determine endianness"));
    };

    let Ok(section_table) = elf.sections(endian, binary) else {
        return Err(LinkerError::new("Failed to read section headers"));
    };

    let is_mips64el = elf.is_mips64el(endian);

    let mut relocation_sections = vec![];

    for (_section_index, section_header) in section_table.enumerate() {
        let section_type = section_header.sh_type(endian);

        if section_type == object::elf::SHT_RELA {
            // There are two types of relocation sections:
            // SHT_REL: relocation entries without addends, the addend is stored in the "placeholder".
            // SHT_RELA: relocation entries with addends, the addend is stored in the relocation entry.
            //
            // In general, one relocatable object file (ET_REL) may have multiple relocation sections (SHT_REL or SHT_RELA),
            // each of them corresponds to a section that contains placeholders (e.g. `.text`),
            // and the name of the relocation section is usually `.rel.text` or `.rela.text`.

            let Ok(Some((relas, _linked_symbol_table_section_index))) =
                section_header.rela(endian, binary)
            else {
                return Err(LinkerError::new("Failed to read relocation entries"));
            };

            let relocations = parse_relocations(relas, endian, is_mips64el)?;

            // There are two fields provide more information about the relocation section:
            // - `sh_link`: it gives the index of the symbol table section linked by the
            //   relocation section, and the relocation entries refer to the symbols in that symbol table.
            // - `sh_info`: it gives the index of the section to which the relocation entries apply
            //   (e.g. the `.text` section).
            // But we don't need these two fields because we assume that there is
            // only one symbol table section and one code section.

            let target_section_index = section_header.sh_info(endian) as usize;

            let section_name =
                str::from_utf8(section_table.section_name(endian, section_header).unwrap())
                    .unwrap();

            let relocation_section = super::module::RelocationSection {
                name: section_name.to_string(),
                target_section_index,
                relocations,
            };

            relocation_sections.push(relocation_section);
        }
    }

    Ok(relocation_sections)
}

fn parse_relocations(
    relas: &[object::elf::Rela64<Endianness>],
    endian: Endianness,
    is_mips64el: bool,
) -> Result<Vec<Relocation>, LinkerError> {
    let mut relocations = Vec::new();

    for rela in relas {
        let placeholder_offset = rela.r_offset(endian) as usize;
        let addend = rela.r_addend(endian) as isize;

        // The `r_info` field encodes both the symbol index and the relocation type.
        // - high 32 bits is the symbol index.
        // - low 32 bits is the relocation type (such as R_X86_64_PC32, R_X86_64_PLT32, etc.)
        //
        // Obtains symbol index and relocation type from the `r_info` field:
        //
        // ```rust
        // let info = relocation.r_info(endian, elf.is_mips64el(endian));
        // let symbol_index = info >> 32;
        // let relocation_type = info & 0xffffffff;
        // ```
        //
        // Or using `relocation` trait methods:
        //
        // ```rust
        // let symbol_index =relocation.r_sym(endian, elf.is_mips64el(endian));
        // let relocation_type = relocation.r_type(endian, elf.is_mips64el(endian));
        // ```

        let symbol_index = rela.r_sym(endian, is_mips64el);
        let relocation_type_raw = rela.r_type(endian, is_mips64el);
        let relocation_type = parse_relocation_type(relocation_type_raw)?;

        // Common relocation type (r_type) includes:
        // - object::elf::R_X86_64_64 => "R_X86_64_64"
        // - object::elf::R_X86_64_PC32 => "R_X86_64_PC32"
        // - object::elf::R_X86_64_GOT32 => "R_X86_64_GOT32"
        // - object::elf::R_X86_64_PLT32 => "R_X86_64_PLT32"
        // - object::elf::R_X86_64_RELATIVE => "R_X86_64_RELATIVE"
        // - object::elf::R_X86_64_32 => "R_X86_64_32"

        let relocation = Relocation {
            relocation_type,
            placeholder_offset,
            symbol_index: symbol_index as usize,
            addend,
        };
        relocations.push(relocation);
    }

    Ok(relocations)
}

fn parse_relocation_type(relocation_type_raw: u32) -> Result<RelocationType, LinkerError> {
    match relocation_type_raw {
        object::elf::R_X86_64_PC32 => Ok(RelocationType::R_X86_64_PC32),
        object::elf::R_X86_64_64 => Ok(RelocationType::R_X86_64_64),
        object::elf::R_X86_64_32 => Ok(RelocationType::R_X86_64_32),
        object::elf::R_X86_64_TPOFF32 => Ok(RelocationType::R_X86_64_TPOFF32),
        _ => Err(LinkerError::new(&format!(
            "Unsupported relocation type: {relocation_type_raw}"
        ))),
    }
}

pub fn read_program_headers(
    elf: &FileHeader64<Endianness>,
    binary: &[u8],
) -> Result<Vec<super::module::ProgramHeader>, LinkerError> {
    let Ok(endian) = elf.endian() else {
        return Err(LinkerError::new("Failed to determine endianness"));
    };

    let Ok(segments) = elf.program_headers(endian, binary) else {
        return Err(LinkerError::new("Failed to read program headers"));
    };

    let mut program_headers = vec![];

    for segment in segments {
        let segment_type = SegmentType::from(segment.p_type(endian));
        let mut segment_flags = vec![];

        let flags = segment.p_flags(endian);
        if flags & object::elf::PF_X != 0 {
            segment_flags.push(SegmentFlag::Execute);
        }
        if flags & object::elf::PF_W != 0 {
            segment_flags.push(SegmentFlag::Write);
        }
        if flags & object::elf::PF_R != 0 {
            segment_flags.push(SegmentFlag::Read);
        }

        let offset = segment.p_offset(endian) as usize;
        let virtual_address = segment.p_vaddr(endian) as usize;
        let file_size = segment.p_filesz(endian) as usize;
        let memory_size = segment.p_memsz(endian) as usize;
        let align = segment.p_align(endian) as usize;

        let program_header = super::module::ProgramHeader {
            segment_type,
            segment_flags,
            offset,
            virtual_address,
            file_size,
            memory_size,
            align,
        };

        program_headers.push(program_header);
    }

    Ok(program_headers)
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use std::{fs, vec};

    use crate::elf::{
        module::{
            FileHeader, FileType, Machine, OSABI, Relocation, RelocationType, SectionType, Symbol,
            SymbolBind, SymbolType,
        },
        reader::{
            read_file, read_file_header, read_program_headers, read_relocation_sections, read_section_headers, read_symbols
        },
    };

    fn get_example_file_binary(file_name: &str) -> Vec<u8> {
        let file_path = std::env::current_dir()
            .unwrap()
            .join("resources/examples/x86_64-linux")
            .join(file_name);

        fs::read(file_path).unwrap()
    }

    #[test]
    fn test_read_file_header() {
        // Read file header of `minimal.o`
        // Manually check with command `readelf -h minimal.o`
        {
            let binary = get_example_file_binary("minimal.o");
            let elf = read_file(&binary).unwrap();
            let file_header = read_file_header(elf).unwrap();

            assert_eq!(
                file_header,
                FileHeader {
                    os_abi: OSABI::SystemV,
                    machine: Machine::X86_64,
                    file_type: FileType::Relocatable,
                    entry_point: 0,
                    number_of_program_headers: 0,
                    number_of_section_headers: 5,
                }
            );
        }

        // Read file header of `minimal.elf`
        // Manually check with command `readelf -h minimal.elf`
        {
            let binary = get_example_file_binary("minimal.elf");
            let elf = read_file(&binary).unwrap();
            let file_header = read_file_header(elf).unwrap();

            assert_eq!(
                file_header,
                FileHeader {
                    os_abi: OSABI::SystemV,
                    machine: Machine::X86_64,
                    file_type: FileType::Executable,
                    entry_point: 0x401000,
                    number_of_program_headers: 2,
                    number_of_section_headers: 5,
                }
            );
        }
    }

    #[test]
    fn test_read_section_header() {
        // Read section headers of `minimal.o`
        // Manually check with command `readelf -S minimal.o`
        {
            let binary = get_example_file_binary("minimal.o");
            let elf = read_file(&binary).unwrap();
            let sections = read_section_headers(elf, &binary).unwrap();

            assert_eq!(sections.len(), 5);
            assert_eq!(
                sections.iter().map(|s| &s.name).collect::<Vec<_>>(),
                vec!["", ".text", ".shstrtab", ".symtab", ".strtab"]
            );
            assert_eq!(
                sections.iter().map(|s| s.section_type).collect::<Vec<_>>(),
                vec![
                    SectionType::Null,
                    SectionType::Progbits,
                    SectionType::Strtab,
                    SectionType::Symtab,
                    SectionType::Strtab
                ]
            );
            assert_eq!(
                sections.iter().map(|s| s.size).collect::<Vec<_>>(),
                vec![0, 0xc, 0x21, 0x60, 0x14]
            );
            assert_eq!(
                sections.iter().map(|s| s.binary.len()).collect::<Vec<_>>(),
                vec![0, 0xc, 0x21, 0x60, 0x14]
            );
            assert_eq!(
                sections.iter().map(|s| s.align).collect::<Vec<_>>(),
                vec![0, 16, 1, 8, 1]
            );
            assert_eq!(
                sections.iter().map(|s| s.offset).collect::<Vec<_>>(),
                vec![0, 0x180, 0x190, 0x1c0, 0x220]
            );
        }

        // Read section headers of `data.o`
        // Manually check with command `readelf -S data.o`
        {
            let binary = get_example_file_binary("data.o");
            let elf = read_file(&binary).unwrap();
            let sections = read_section_headers(elf, &binary).unwrap();

            assert_eq!(sections.len(), 9);
            assert_eq!(
                sections.iter().map(|s| &s.name).collect::<Vec<_>>(),
                vec![
                    "",
                    ".rodata",
                    ".data",
                    ".bss",
                    ".text",
                    ".shstrtab",
                    ".symtab",
                    ".strtab",
                    ".rela.text"
                ]
            );
            assert_eq!(
                sections.iter().map(|s| s.section_type).collect::<Vec<_>>(),
                vec![
                    SectionType::Null,
                    SectionType::Progbits,
                    SectionType::Progbits,
                    SectionType::Nobits,
                    SectionType::Progbits,
                    SectionType::Strtab,
                    SectionType::Symtab,
                    SectionType::Strtab,
                    SectionType::Rela
                ]
            );
            assert_eq!(
                sections.iter().map(|s| s.size).collect::<Vec<_>>(),
                vec![0, 0x10, 0x10, 0x10, 0x58, 0x3f, 0x138, 0x21, 0xf0]
            );
            assert_eq!(
                sections.iter().map(|s| s.binary.len()).collect::<Vec<_>>(),
                vec![0, 0x10, 0x10, 0x0, 0x58, 0x3f, 0x138, 0x21, 0xf0]
            );
            assert_eq!(
                sections.iter().map(|s| s.align).collect::<Vec<_>>(),
                vec![0, 4, 4, 4, 16, 1, 8, 1, 8]
            );
            assert_eq!(
                sections.iter().map(|s| s.offset).collect::<Vec<_>>(),
                vec![0, 0x280, 0x290, 0x2a0, 0x2a0, 0x300, 0x340, 0x480, 0x4b0]
            );
        }
    }

    #[test]
    fn test_read_symbols() {
        // Read symbols of `minimal.o`
        // Manually check with command `readelf -s minimal.o`
        {
            let binary = get_example_file_binary("minimal.o");
            let elf = read_file(&binary).unwrap();
            let symbols = read_symbols(elf, &binary).unwrap();

            assert_eq!(symbols.len(), 4);

            assert_eq!(symbols[0], Symbol::Other);
            assert_eq!(symbols[1], Symbol::Other);
            assert_eq!(
                symbols[2],
                Symbol::Defined {
                    name: String::new(),
                    section_index: 1,
                    bind: SymbolBind::Local,
                    symbol_type: SymbolType::Section,
                    offset: 0,
                }
            );
            assert_eq!(
                symbols[3],
                Symbol::Defined {
                    name: "_start".to_string(),
                    section_index: 1,
                    bind: SymbolBind::Global,
                    symbol_type: SymbolType::Notype,
                    offset: 0,
                }
            );
        }

        // Read symbols of `data.o`
        // Manually check with command `readelf -s data.o`
        {
            let binary = get_example_file_binary("data.o");
            let elf = read_file(&binary).unwrap();
            let symbols = read_symbols(elf, &binary).unwrap();

            assert_eq!(symbols.len(), 13);

            assert_eq!(symbols[0], Symbol::Other);
            assert_eq!(symbols[1], Symbol::Other);
            assert_eq!(
                symbols[2],
                Symbol::Defined {
                    name: String::new(),
                    section_index: 1,
                    bind: SymbolBind::Local,
                    symbol_type: SymbolType::Section,
                    offset: 0,
                }
            );
            assert_eq!(
                symbols[3],
                Symbol::Defined {
                    name: String::new(),
                    section_index: 2,
                    bind: SymbolBind::Local,
                    symbol_type: SymbolType::Section,
                    offset: 0,
                }
            );
            assert_eq!(
                symbols[4],
                Symbol::Defined {
                    name: String::new(),
                    section_index: 3,
                    bind: SymbolBind::Local,
                    symbol_type: SymbolType::Section,
                    offset: 0,
                }
            );
            assert_eq!(
                symbols[5],
                Symbol::Defined {
                    name: String::new(),
                    section_index: 4,
                    bind: SymbolBind::Local,
                    symbol_type: SymbolType::Section,
                    offset: 0,
                }
            );

            assert_eq!(
                symbols[6],
                Symbol::Defined {
                    name: "foo".to_string(),
                    section_index: 1,
                    bind: SymbolBind::Local,
                    symbol_type: SymbolType::Notype,
                    offset: 0,
                }
            );
            assert_eq!(
                symbols[7],
                Symbol::Defined {
                    name: "bar".to_string(),
                    section_index: 1,
                    bind: SymbolBind::Local,
                    symbol_type: SymbolType::Notype,
                    offset: 0x8,
                }
            );

            assert_eq!(
                symbols[8],
                Symbol::Defined {
                    name: "a".to_string(),
                    section_index: 2,
                    bind: SymbolBind::Local,
                    symbol_type: SymbolType::Notype,
                    offset: 0,
                }
            );
            assert_eq!(
                symbols[9],
                Symbol::Defined {
                    name: "b".to_string(),
                    section_index: 2,
                    bind: SymbolBind::Local,
                    symbol_type: SymbolType::Notype,
                    offset: 0x8,
                }
            );

            assert_eq!(
                symbols[10],
                Symbol::Defined {
                    name: "x".to_string(),
                    section_index: 3,
                    bind: SymbolBind::Local,
                    symbol_type: SymbolType::Notype,
                    offset: 0,
                }
            );
            assert_eq!(
                symbols[11],
                Symbol::Defined {
                    name: "y".to_string(),
                    section_index: 3,
                    bind: SymbolBind::Local,
                    symbol_type: SymbolType::Notype,
                    offset: 0x8,
                }
            );

            assert_eq!(
                symbols[12],
                Symbol::Defined {
                    name: "_start".to_string(),
                    section_index: 4,
                    bind: SymbolBind::Global,
                    symbol_type: SymbolType::Notype,
                    offset: 0,
                }
            );
        }

        // Read symbols of `symbol-import.o`
        // Manually check with command `readelf -s symbol-import.o`
        {
            let binary = get_example_file_binary("symbol-import.o");
            let elf = read_file(&binary).unwrap();
            let symbols = read_symbols(elf, &binary).unwrap();

            assert_eq!(symbols.len(), 12);

            assert_eq!(symbols[0], Symbol::Other);
            assert_eq!(symbols[1], Symbol::Other);
            assert_eq!(
                symbols[2],
                Symbol::Defined {
                    name: String::new(),
                    section_index: 1,
                    bind: SymbolBind::Local,
                    symbol_type: SymbolType::Section,
                    offset: 0,
                }
            );

            assert_eq!(symbols[3], Symbol::External("foo".to_string()));
            assert_eq!(symbols[4], Symbol::External("bar".to_string()));

            assert_eq!(symbols[5], Symbol::External("a".to_string()));
            assert_eq!(symbols[6], Symbol::External("b".to_string()));

            assert_eq!(symbols[7], Symbol::External("x".to_string()));
            assert_eq!(symbols[8], Symbol::External("y".to_string()));

            assert_eq!(symbols[9], Symbol::External("dec".to_string()));
            assert_eq!(symbols[10], Symbol::External("inc".to_string()));

            assert_eq!(
                symbols[11],
                Symbol::Defined {
                    name: "_start".to_string(),
                    section_index: 1,
                    bind: SymbolBind::Global,
                    symbol_type: SymbolType::Notype,
                    offset: 0,
                }
            );
        }

        // Read symbols of `override-weak.o`
        // Manually check with command `readelf -s override-weak.o`
        {
            let binary = get_example_file_binary("override-weak.o");
            let elf = read_file(&binary).unwrap();
            let symbols = read_symbols(elf, &binary).unwrap();

            assert_eq!(symbols.len(), 5);

            assert_eq!(symbols[0], Symbol::Other);
            assert_eq!(symbols[1], Symbol::Other);
            assert_eq!(
                symbols[2],
                Symbol::Defined {
                    name: String::new(),
                    section_index: 1,
                    bind: SymbolBind::Local,
                    symbol_type: SymbolType::Section,
                    offset: 0,
                }
            );

            assert_eq!(
                symbols[3],
                Symbol::Defined {
                    name: "foo".to_string(),
                    bind: SymbolBind::Weak,
                    symbol_type: SymbolType::Notype,
                    section_index: 1,
                    offset: 0
                }
            );
            assert_eq!(
                symbols[4],
                Symbol::Defined {
                    name: "bar".to_string(),
                    bind: SymbolBind::Weak,
                    symbol_type: SymbolType::Notype,
                    section_index: 1,
                    offset: 0x6
                }
            );
        }
    }

    #[test]
    fn test_read_relocation_sections() {
        // Read relocation sections of `minimal.o`
        // Manually check with command `readelf -r minimal.o`
        {
            let binary = get_example_file_binary("minimal.o");
            let elf = read_file(&binary).unwrap();
            let relocation_sections = read_relocation_sections(elf, &binary).unwrap();

            assert_eq!(relocation_sections.len(), 0);
        }

        // Read relocation sections of `data.o`
        // Manually check with command `readelf -r data.o`
        {
            let binary = get_example_file_binary("data.o");
            let elf = read_file(&binary).unwrap();
            let relocation_sections = read_relocation_sections(elf, &binary).unwrap();

            assert_eq!(relocation_sections.len(), 1);

            let relocation_section = &relocation_sections[0];
            assert_eq!(relocation_section.name, ".rela.text");
            assert_eq!(relocation_section.target_section_index, 4); // `.text` section index

            let relocations = &relocation_section.relocations;
            assert_eq!(relocations.len(), 10);

            assert_eq!(
                relocations[0],
                Relocation {
                    relocation_type: RelocationType::R_X86_64_PC32,
                    placeholder_offset: 0x3,
                    symbol_index: 2, // section symbol of `.rodata` section
                    addend: -4
                }
            );
            assert_eq!(
                relocations[1],
                Relocation {
                    relocation_type: RelocationType::R_X86_64_PC32,
                    placeholder_offset: 0xe,
                    symbol_index: 3, // section symbol of `.data` section
                    addend: -4
                }
            );

            // The rest of the relocation entries are similar,
            // so we can just check the first two entries for testing purposes.
        }

        // Read relocation sections of `relocate-within-data.o`
        // Manually check with command `readelf -r relocate-within-data.o`
        {
            let binary = get_example_file_binary("relocate-within-data.o");
            let elf = read_file(&binary).unwrap();
            let relocation_sections = read_relocation_sections(elf, &binary).unwrap();

            assert_eq!(relocation_sections.len(), 3);

            // `.rela.text`
            {
                let relocation_section = &relocation_sections[0];
                assert_eq!(relocation_section.name, ".rela.data");
                assert_eq!(relocation_section.target_section_index, 1); // `.data` section index

                let relocations = &relocation_section.relocations;
                assert_eq!(relocations.len(), 2);

                assert_eq!(
                    relocations[0],
                    Relocation {
                        relocation_type: RelocationType::R_X86_64_64,
                        placeholder_offset: 0x10,
                        symbol_index: 4, // section symbol of `.text` section
                        addend: 0
                    }
                );
                assert_eq!(
                    relocations[1],
                    Relocation {
                        relocation_type: RelocationType::R_X86_64_64,
                        placeholder_offset: 0x18,
                        symbol_index: 4, // section symbol of `.text` section
                        addend: 8
                    }
                );
            }

            // `.rela.rodata`
            {
                let relocation_section = &relocation_sections[1];
                assert_eq!(relocation_section.name, ".rela.rodata");
                assert_eq!(relocation_section.target_section_index, 2); // `.rodata` section index

                let relocations = &relocation_section.relocations;
                assert_eq!(relocations.len(), 2);

                assert_eq!(
                    relocations[0],
                    Relocation {
                        relocation_type: RelocationType::R_X86_64_64,
                        placeholder_offset: 0,
                        symbol_index: 2, // section symbol of `.data` section
                        addend: 0
                    }
                );
                assert_eq!(
                    relocations[1],
                    Relocation {
                        relocation_type: RelocationType::R_X86_64_64,
                        placeholder_offset: 0x8,
                        symbol_index: 2, // section symbol of `.data` section
                        addend: 8
                    }
                );
            }

            // `.rela.text`
            {
                let relocation_section = &relocation_sections[2];
                assert_eq!(relocation_section.name, ".rela.text");
                assert_eq!(relocation_section.target_section_index, 3); // `.text` section index

                let relocations = &relocation_section.relocations;
                assert_eq!(relocations.len(), 6);

                assert_eq!(
                    relocations[0],
                    Relocation {
                        relocation_type: RelocationType::R_X86_64_PC32,
                        placeholder_offset: 0x13,
                        symbol_index: 3, // section symbol of `.rodata` section
                        addend: -4
                    }
                );
                assert_eq!(
                    relocations[1],
                    Relocation {
                        relocation_type: RelocationType::R_X86_64_PC32,
                        placeholder_offset: 0x1d,
                        symbol_index: 2, // section symbol of `.data` section
                        addend: 0xc
                    }
                );

                // The rest of the relocation entries are similar,
                // so we can just check the first two entries for testing purposes.
            }
        }
    }

    #[test]
    fn test_read_program_headers() {
        // Read program headers of `minimal.o`
        // Manually check with command `readelf -l minimal.o`
        {
            let binary = get_example_file_binary("minimal.o");
            let elf = read_file(&binary).unwrap();
            let program_headers = read_program_headers(elf, &binary).unwrap();

            assert_eq!(program_headers.len(), 0);
        }
    }
}
