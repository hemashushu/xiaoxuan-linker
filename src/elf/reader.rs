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
    let program_header_count = elf.e_phnum(endian) as usize;
    let section_header_count = elf.e_shnum(endian) as usize;

    Ok(super::module::FileHeader {
        os_abi,
        machine,
        file_type,
        entry_point,
        program_header_count,
        section_header_count,
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
        // let symbol_index = relocation.r_sym(endian, elf.is_mips64el(endian));
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
        object::elf::R_X86_64_PLT32 => Ok(RelocationType::R_X86_64_PLT32),
        object::elf::R_AARCH64_ADR_PREL_PG_HI21 => Ok(RelocationType::R_AARCH64_ADR_PREL_PG_HI21),
        object::elf::R_AARCH64_LDST64_ABS_LO12_NC => {
            Ok(RelocationType::R_AARCH64_LDST64_ABS_LO12_NC)
        }
        object::elf::R_AARCH64_ADD_ABS_LO12_NC => Ok(RelocationType::R_AARCH64_ADD_ABS_LO12_NC),
        object::elf::R_AARCH64_CALL26 => Ok(RelocationType::R_AARCH64_CALL26),
        object::elf::R_AARCH64_ABS64 => Ok(RelocationType::R_AARCH64_ABS64),
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
    use std::vec;

    use crate::elf::{
        module::{
            FileHeader, FileType, Machine, OSABI, ProgramHeader, Relocation, RelocationType,
            SectionType, SegmentFlag, SegmentType, Symbol, SymbolBind, SymbolType,
        },
        reader::{
            read_file, read_file_header, read_program_headers, read_relocation_sections,
            read_section_headers, read_symbols,
        },
    };

    enum ARCH {
        X86_64,
        AARCH64,
        RISCV64,
        LOONGARCH64,
        POWERSPC64LE,
        S390X,
        UNSUPPORTED,
    }

    fn get_arch() -> ARCH {
        match std::env::consts::ARCH {
            "x86_64" => ARCH::X86_64,
            "aarch64" => ARCH::AARCH64,
            "riscv64" => ARCH::RISCV64,
            "loongarch64" => ARCH::LOONGARCH64,
            "powerpc64" => ARCH::POWERSPC64LE,
            "s390x" => ARCH::S390X,
            _ => ARCH::UNSUPPORTED,
        }
    }

    fn get_arch_dir_name() -> &'static str {
        match get_arch() {
            ARCH::X86_64 => "x86_64-linux",
            ARCH::AARCH64 => "aarch64-linux",
            ARCH::RISCV64 => "riscv64-linux",
            ARCH::LOONGARCH64 => "loongarch64-linux",
            ARCH::POWERSPC64LE => "powerpc64le-linux",
            ARCH::S390X => "s390x-linux",
            ARCH::UNSUPPORTED => panic!("Unsupported architecture"),
        }
    }

    fn get_example_file_binary(file_name: &str) -> Vec<u8> {
        let file_path = std::env::current_dir()
            .unwrap()
            .join("resources/examples")
            .join(get_arch_dir_name())
            .join(file_name);

        std::fs::read(file_path).unwrap()
    }

    #[test]
    fn test_read_file_header_asm_minimal_o() {
        // Manually check with command `readelf -h asm/minimal.o`
        let binary = get_example_file_binary("asm/minimal.o");
        let elf = read_file(&binary).unwrap();
        let file_header = read_file_header(elf).unwrap();

        match get_arch() {
            ARCH::X86_64 => {
                assert_eq!(
                    file_header,
                    FileHeader {
                        os_abi: OSABI::SystemV,
                        machine: Machine::X86_64,
                        file_type: FileType::Relocatable,
                        entry_point: 0,
                        program_header_count: 0,
                        section_header_count: 8,
                    }
                );
            }
            ARCH::AARCH64 => {
                assert_eq!(
                    file_header,
                    FileHeader {
                        os_abi: OSABI::SystemV,
                        machine: Machine::AArch64,
                        file_type: FileType::Relocatable,
                        entry_point: 0,
                        program_header_count: 0,
                        section_header_count: 7,
                    }
                );
            }
            _ => unimplemented!(),
        }
    }

    #[test]
    fn test_read_file_header_asm_minimal_elf() {
        // Manually check with command `readelf -h asm/minimal.elf`
        let binary = get_example_file_binary("asm/minimal.elf");
        let elf = read_file(&binary).unwrap();
        let file_header = read_file_header(elf).unwrap();

        match get_arch() {
            ARCH::X86_64 => {
                assert_eq!(
                    file_header,
                    FileHeader {
                        os_abi: OSABI::SystemV,
                        machine: Machine::X86_64,
                        file_type: FileType::Executable,
                        entry_point: 0x401000,
                        program_header_count: 5,
                        section_header_count: 6,
                    }
                );
            }
            ARCH::AARCH64 => {
                assert_eq!(
                    file_header,
                    FileHeader {
                        os_abi: OSABI::SystemV,
                        machine: Machine::AArch64,
                        file_type: FileType::Executable,
                        entry_point: 0x400078,
                        program_header_count: 1,
                        section_header_count: 5,
                    }
                );
            }
            _ => unimplemented!(),
        }
    }

    #[test]
    fn test_read_section_header_asm_minimal_o() {
        // Fields such as `size`, `binary`, `align`, and `offset` are not
        // intended to be tested here because they are not guaranteed
        // to be the same across different versions of the assembler and platforms.

        // Note: check certain entries only for testing purposes.

        // Manually check with command `readelf -S asm/minimal.o`
        let binary = get_example_file_binary("asm/minimal.o");
        let elf = read_file(&binary).unwrap();
        let sections = read_section_headers(elf, &binary).unwrap();

        match get_arch() {
            ARCH::X86_64 => {
                assert_eq!(sections.len(), 8);
                assert_eq!(
                    sections.iter().take(4).map(|s| &s.name).collect::<Vec<_>>(),
                    vec!["", ".text", ".data", ".bss",]
                );
                assert_eq!(
                    sections
                        .iter()
                        .take(4)
                        .map(|s| s.section_type)
                        .collect::<Vec<_>>(),
                    vec![
                        SectionType::Null,     // null
                        SectionType::Progbits, // .text
                        SectionType::Progbits, // .data
                        SectionType::Nobits,   // .bss
                    ]
                );
            }

            ARCH::AARCH64 => {
                assert_eq!(sections.len(), 7);
                assert_eq!(
                    sections.iter().take(4).map(|s| &s.name).collect::<Vec<_>>(),
                    vec!["", ".text", ".data", ".bss",]
                );
                assert_eq!(
                    sections
                        .iter()
                        .take(4)
                        .map(|s| s.section_type)
                        .collect::<Vec<_>>(),
                    vec![
                        SectionType::Null,     // null
                        SectionType::Progbits, // .text
                        SectionType::Progbits, // .data
                        SectionType::Nobits,   // .bss
                    ]
                );
            }
            _ => unimplemented!(),
        }
    }

    #[test]
    fn test_read_section_header_asm_data_o() {
        // Manually check with command `readelf -S asm/data.o`
        let binary = get_example_file_binary("asm/data.o");
        let elf = read_file(&binary).unwrap();
        let sections = read_section_headers(elf, &binary).unwrap();

        match get_arch() {
            ARCH::X86_64 => {
                assert_eq!(sections.len(), 10);
                assert_eq!(
                    sections.iter().take(4).map(|s| &s.name).collect::<Vec<_>>(),
                    vec!["", ".text", ".rela.text", ".data",]
                );
                assert_eq!(
                    sections
                        .iter()
                        .take(4)
                        .map(|s| s.section_type)
                        .collect::<Vec<_>>(),
                    vec![
                        SectionType::Null,     // null
                        SectionType::Progbits, // .text
                        SectionType::Rela,     // .rela.text
                        SectionType::Progbits, // .data
                    ]
                );
            }

            ARCH::AARCH64 => {
                assert_eq!(sections.len(), 9);
                assert_eq!(
                    sections.iter().take(4).map(|s| &s.name).collect::<Vec<_>>(),
                    vec!["", ".text", ".rela.text", ".data",]
                );
                assert_eq!(
                    sections
                        .iter()
                        .take(4)
                        .map(|s| s.section_type)
                        .collect::<Vec<_>>(),
                    vec![
                        SectionType::Null,     // null
                        SectionType::Progbits, // .text
                        SectionType::Rela,     // .rela.text
                        SectionType::Progbits, // .data
                    ]
                );
            }
            _ => unimplemented!(),
        }
    }

    #[test]
    fn test_read_symbols_asm_minimal_o() {
        // Note: check certain entries only for testing purposes.

        // Manually check with command `readelf -s asm/minimal.o`
        let binary = get_example_file_binary("asm/minimal.o");
        let elf = read_file(&binary).unwrap();
        let symbols = read_symbols(elf, &binary).unwrap();

        match get_arch() {
            ARCH::X86_64 => {
                assert_eq!(symbols.len(), 2);
                assert_eq!(symbols[0], Symbol::Other);
                assert_eq!(
                    symbols[1],
                    Symbol::Defined {
                        name: "_start".to_string(),
                        section_index: 1,
                        bind: SymbolBind::Global,
                        symbol_type: SymbolType::Notype,
                        offset: 0,
                    }
                );
            }

            ARCH::AARCH64 => {
                assert_eq!(symbols.len(), 6);
                assert_eq!(symbols[0], Symbol::Other);

                assert_eq!(
                    symbols[1],
                    Symbol::Defined {
                        name: String::new(),
                        section_index: 1,
                        bind: SymbolBind::Local,
                        symbol_type: SymbolType::Section,
                        offset: 0,
                    }
                );

                assert_eq!(
                    symbols[5],
                    Symbol::Defined {
                        name: "_start".to_string(),
                        section_index: 1,
                        bind: SymbolBind::Global,
                        symbol_type: SymbolType::Notype,
                        offset: 0,
                    }
                );
            }
            _ => unimplemented!(),
        }
    }

    #[test]
    fn test_read_symbols_asm_data_o() {
        // Manually check with command `readelf -s asm/data.o`
        let binary = get_example_file_binary("asm/data.o");
        let elf = read_file(&binary).unwrap();
        let symbols = read_symbols(elf, &binary).unwrap();

        match get_arch() {
            ARCH::X86_64 => {
                assert_eq!(symbols.len(), 11);

                assert_eq!(symbols[0], Symbol::Other);
                assert_eq!(
                    symbols[1],
                    Symbol::Defined {
                        name: String::new(),
                        section_index: 3,
                        bind: SymbolBind::Local,
                        symbol_type: SymbolType::Section,
                        offset: 0,
                    }
                );

                assert_eq!(
                    symbols[10],
                    Symbol::Defined {
                        name: "_start".to_string(),
                        section_index: 1,
                        bind: SymbolBind::Global,
                        symbol_type: SymbolType::Notype,
                        offset: 0x0,
                    }
                );
            }

            ARCH::AARCH64 => {
                assert_eq!(symbols.len(), 16);

                assert_eq!(symbols[0], Symbol::Other);
                assert_eq!(
                    symbols[1],
                    Symbol::Defined {
                        name: String::new(),
                        section_index: 1,
                        bind: SymbolBind::Local,
                        symbol_type: SymbolType::Section,
                        offset: 0,
                    }
                );

                assert_eq!(
                    symbols[15],
                    Symbol::Defined {
                        name: "_start".to_string(),
                        section_index: 1,
                        bind: SymbolBind::Global,
                        symbol_type: SymbolType::Notype,
                        offset: 0,
                    }
                );
            }
            _ => unimplemented!(),
        }
    }

    #[test]
    fn test_read_symbols_asm_symbol_import_o() {
        // Manually check with command `readelf -s asm/symbol-import.o`
        let binary = get_example_file_binary("asm/symbol-import.o");
        let elf = read_file(&binary).unwrap();
        let symbols = read_symbols(elf, &binary).unwrap();

        match get_arch() {
            ARCH::X86_64 => {
                assert_eq!(symbols.len(), 10);

                assert_eq!(symbols[0], Symbol::Other);
                assert_eq!(
                    symbols[1],
                    Symbol::Defined {
                        name: "_start".to_string(),
                        section_index: 1,
                        bind: SymbolBind::Global,
                        symbol_type: SymbolType::Notype,
                        offset: 0,
                    }
                );

                assert_eq!(symbols[9], Symbol::External("y".to_string()));
            }

            ARCH::AARCH64 => {
                assert_eq!(symbols.len(), 14);

                assert_eq!(symbols[0], Symbol::Other);
                assert_eq!(
                    symbols[1], // .text
                    Symbol::Defined {
                        name: String::new(),
                        section_index: 1,
                        bind: SymbolBind::Local,
                        symbol_type: SymbolType::Section,
                        offset: 0,
                    }
                );

                assert_eq!(symbols[13], Symbol::External("y".to_string()));
            }
            _ => unimplemented!(),
        }
    }

    #[test]
    fn test_read_symbols_asm_override_weak_o() {
        // Manually check with command `readelf -s asm/override-weak.o`
        let binary = get_example_file_binary("asm/override-weak.o");
        let elf = read_file(&binary).unwrap();
        let symbols = read_symbols(elf, &binary).unwrap();

        match get_arch() {
            ARCH::X86_64 => {
                assert_eq!(symbols.len(), 3);

                assert_eq!(symbols[0], Symbol::Other);
                assert_eq!(
                    symbols[1],
                    Symbol::Defined {
                        name: "foo".to_string(),
                        bind: SymbolBind::Weak,
                        symbol_type: SymbolType::Notype,
                        section_index: 1,
                        offset: 0
                    }
                );
                assert_eq!(
                    symbols[2],
                    Symbol::Defined {
                        name: "bar".to_string(),
                        bind: SymbolBind::Weak,
                        symbol_type: SymbolType::Notype,
                        section_index: 1,
                        offset: 0x6
                    }
                );
            }

            ARCH::AARCH64 => {
                assert_eq!(symbols.len(), 7);

                assert_eq!(symbols[0], Symbol::Other);
                assert_eq!(
                    symbols[1],
                    Symbol::Defined {
                        name: String::new(),
                        section_index: 1,
                        bind: SymbolBind::Local,
                        symbol_type: SymbolType::Section,
                        offset: 0,
                    }
                );

                assert_eq!(
                    symbols[6],
                    Symbol::Defined {
                        name: "bar".to_string(),
                        bind: SymbolBind::Weak,
                        symbol_type: SymbolType::Notype,
                        section_index: 1,
                        offset: 0x8
                    }
                );
            }
            _ => unimplemented!(),
        }
    }

    #[test]
    fn test_read_relocations_asm_minimal_o() {
        // Note: check certain entries only for testing purposes.

        // Manually check with command `readelf -r asm/minimal.o`
        let binary = get_example_file_binary("asm/minimal.o");
        let elf = read_file(&binary).unwrap();
        let relocation_sections = read_relocation_sections(elf, &binary).unwrap();

        assert_eq!(relocation_sections.len(), 0);
    }

    #[test]
    fn test_read_relocations_asm_data_o() {
        // Manually check with command `readelf -r asm/data.o`
        let binary = get_example_file_binary("asm/data.o");
        let elf = read_file(&binary).unwrap();
        let relocation_sections = read_relocation_sections(elf, &binary).unwrap();

        match get_arch() {
            ARCH::X86_64 => {
                assert_eq!(relocation_sections.len(), 1);

                let relocation_section = &relocation_sections[0];
                assert_eq!(relocation_section.name, ".rela.text");
                assert_eq!(relocation_section.target_section_index, 1);

                let relocations = &relocation_section.relocations;
                assert_eq!(relocations.len(), 10);

                assert_eq!(
                    relocations[0],
                    Relocation {
                        relocation_type: RelocationType::R_X86_64_PC32,
                        placeholder_offset: 0x3,
                        symbol_index: 3,
                        addend: -4
                    }
                );
                assert_eq!(
                    relocations[1],
                    Relocation {
                        relocation_type: RelocationType::R_X86_64_PC32,
                        placeholder_offset: 0xe,
                        symbol_index: 1,
                        addend: -4
                    }
                );
            }

            ARCH::AARCH64 => {
                assert_eq!(relocation_sections.len(), 1);

                let relocation_section = &relocation_sections[0];
                assert_eq!(relocation_section.name, ".rela.text");
                assert_eq!(relocation_section.target_section_index, 1);

                let relocations = &relocation_section.relocations;
                assert_eq!(relocations.len(), 20);

                assert_eq!(
                    relocations[0],
                    Relocation {
                        relocation_type: RelocationType::R_AARCH64_ADR_PREL_PG_HI21,
                        placeholder_offset: 0x0,
                        symbol_index: 4,
                        addend: 0
                    }
                );
                assert_eq!(
                    relocations[1],
                    Relocation {
                        relocation_type: RelocationType::R_AARCH64_LDST64_ABS_LO12_NC,
                        placeholder_offset: 0x4,
                        symbol_index: 4,
                        addend: 0
                    }
                );
            }
            _ => unimplemented!(),
        }
    }

    #[test]
    fn test_read_relocations_asm_relocate_within_data_o() {
        // Manually check with command `readelf -r asm/relocate-within-data.o`
        let binary = get_example_file_binary("asm/relocate-within-data.o");
        let elf = read_file(&binary).unwrap();
        let relocation_sections = read_relocation_sections(elf, &binary).unwrap();

        match get_arch() {
            ARCH::X86_64 => {
                assert_eq!(relocation_sections.len(), 3);

                // `.rela.text`
                {
                    let relocation_section = &relocation_sections[0];
                    assert_eq!(relocation_section.name, ".rela.text");
                    assert_eq!(relocation_section.target_section_index, 1);

                    let relocations = &relocation_section.relocations;
                    assert_eq!(relocations.len(), 6);

                    assert_eq!(
                        relocations[0],
                        Relocation {
                            relocation_type: RelocationType::R_X86_64_PC32,
                            placeholder_offset: 0x13,
                            symbol_index: 4,
                            addend: -4
                        }
                    );
                    assert_eq!(
                        relocations[1],
                        Relocation {
                            relocation_type: RelocationType::R_X86_64_PC32,
                            placeholder_offset: 0x1d,
                            symbol_index: 1,
                            addend: 0xc
                        }
                    );
                }

                // `.rela.data`
                {
                    let relocation_section = &relocation_sections[1];
                    assert_eq!(relocation_section.name, ".rela.data");
                    assert_eq!(relocation_section.target_section_index, 3);

                    let relocations = &relocation_section.relocations;
                    assert_eq!(relocations.len(), 2);

                    assert_eq!(
                        relocations[0],
                        Relocation {
                            relocation_type: RelocationType::R_X86_64_64,
                            placeholder_offset: 0x10,
                            symbol_index: 0x9,
                            addend: 0
                        }
                    );
                    assert_eq!(
                        relocations[1],
                        Relocation {
                            relocation_type: RelocationType::R_X86_64_64,
                            placeholder_offset: 0x18,
                            symbol_index: 0xa,
                            addend: 0
                        }
                    );
                }

                // `.rela.rodata`
                {
                    let relocation_section = &relocation_sections[2];
                    assert_eq!(relocation_section.name, ".rela.rodata");
                    assert_eq!(relocation_section.target_section_index, 6);

                    let relocations = &relocation_section.relocations;
                    assert_eq!(relocations.len(), 2);

                    assert_eq!(
                        relocations[0],
                        Relocation {
                            relocation_type: RelocationType::R_X86_64_64,
                            placeholder_offset: 0,
                            symbol_index: 7,
                            addend: 0
                        }
                    );
                    assert_eq!(
                        relocations[1],
                        Relocation {
                            relocation_type: RelocationType::R_X86_64_64,
                            placeholder_offset: 0x8,
                            symbol_index: 8,
                            addend: 0
                        }
                    );
                }
            }

            ARCH::AARCH64 => {
                assert_eq!(relocation_sections.len(), 3);

                // `.rela.text`
                {
                    let relocation_section = &relocation_sections[0];
                    assert_eq!(relocation_section.name, ".rela.text");
                    assert_eq!(relocation_section.target_section_index, 1);

                    let relocations = &relocation_section.relocations;
                    assert_eq!(relocations.len(), 12);

                    assert_eq!(
                        relocations[0],
                        Relocation {
                            relocation_type: RelocationType::R_AARCH64_ADR_PREL_PG_HI21,
                            placeholder_offset: 0x10,
                            symbol_index: 0xb,
                            addend: 0
                        }
                    );
                    assert_eq!(
                        relocations[1],
                        Relocation {
                            relocation_type: RelocationType::R_AARCH64_LDST64_ABS_LO12_NC,
                            placeholder_offset: 0x14,
                            symbol_index: 0xb,
                            addend: 0
                        }
                    );
                }

                // `.rela.data`
                {
                    let relocation_section = &relocation_sections[1];
                    assert_eq!(relocation_section.name, ".rela.data");
                    assert_eq!(relocation_section.target_section_index, 3);

                    let relocations = &relocation_section.relocations;
                    assert_eq!(relocations.len(), 2);

                    assert_eq!(
                        relocations[0],
                        Relocation {
                            relocation_type: RelocationType::R_AARCH64_ABS64,
                            placeholder_offset: 0x20,
                            symbol_index: 0x13,
                            addend: 0
                        }
                    );
                    assert_eq!(
                        relocations[1],
                        Relocation {
                            relocation_type: RelocationType::R_AARCH64_ABS64,
                            placeholder_offset: 0x28,
                            symbol_index: 0x14,
                            addend: 0
                        }
                    );
                }

                // `.rela.rodata`
                {
                    let relocation_section = &relocation_sections[2];
                    assert_eq!(relocation_section.name, ".rela.rodata");
                    assert_eq!(relocation_section.target_section_index, 6);

                    let relocations = &relocation_section.relocations;
                    assert_eq!(relocations.len(), 2);

                    assert_eq!(
                        relocations[0],
                        Relocation {
                            relocation_type: RelocationType::R_AARCH64_ABS64,
                            placeholder_offset: 0,
                            symbol_index: 2,
                            addend: 0
                        }
                    );
                    assert_eq!(
                        relocations[1],
                        Relocation {
                            relocation_type: RelocationType::R_AARCH64_ABS64,
                            placeholder_offset: 0x8,
                            symbol_index: 2,
                            addend: 8
                        }
                    );
                }
            }
            _ => unimplemented!(),
        }
    }

    #[test]
    fn test_read_program_headers_asm_minimal_o() {
        // Note: check certain entries only for testing purposes.

        // Manually check with command `readelf -l asm/minimal.o`
        let binary = get_example_file_binary("asm/minimal.o");
        let elf = read_file(&binary).unwrap();
        let program_headers = read_program_headers(elf, &binary).unwrap();

        assert_eq!(program_headers.len(), 0);
    }

    #[test]
    fn test_read_program_headers_asm_minimal_elf() {
        // Manually check with command `readelf -l asm/minimal.elf`
        let binary = get_example_file_binary("asm/minimal.elf");
        let elf = read_file(&binary).unwrap();
        let program_headers = read_program_headers(elf, &binary).unwrap();

        match get_arch() {
            ARCH::X86_64 => {
                assert_eq!(program_headers.len(), 5);

                assert_eq!(
                    program_headers[0],
                    ProgramHeader {
                        segment_type: SegmentType::Load,
                        segment_flags: vec![SegmentFlag::Read,],
                        offset: 0,
                        virtual_address: 0x400000,
                        file_size: 0x158,
                        memory_size: 0x158,
                        align: 0x1000
                    }
                );

                assert_eq!(
                    program_headers[1],
                    ProgramHeader {
                        segment_type: SegmentType::Load,
                        segment_flags: vec![SegmentFlag::Execute, SegmentFlag::Read],
                        offset: 0x1000,
                        virtual_address: 0x401000,
                        file_size: 0x10,
                        memory_size: 0x10,
                        align: 0x1000
                    }
                );
            }

            ARCH::AARCH64 => {
                assert_eq!(program_headers.len(), 1);

                assert_eq!(
                    program_headers[0],
                    ProgramHeader {
                        segment_type: SegmentType::Load,
                        segment_flags: vec![SegmentFlag::Execute, SegmentFlag::Read],
                        offset: 0,
                        virtual_address: 0x400000,
                        file_size: 0x84,
                        memory_size: 0x84,
                        align: 0x10000
                    }
                );
            }
            _ => unimplemented!(),
        }
    }

    #[test]
    fn test_read_program_headers_asm_data_elf() {
        // Manually check with command `readelf -l asm/data.elf`
        let binary = get_example_file_binary("asm/data.elf");
        let elf = read_file(&binary).unwrap();
        let program_headers = read_program_headers(elf, &binary).unwrap();

        match get_arch() {
            ARCH::X86_64 => {
                assert_eq!(program_headers.len(), 6);

                assert_eq!(
                    program_headers[0],
                    ProgramHeader {
                        segment_type: SegmentType::Load,
                        segment_flags: vec![SegmentFlag::Read,],
                        offset: 0,
                        virtual_address: 0x400000,
                        file_size: 0x190,
                        memory_size: 0x190,
                        align: 0x1000
                    }
                );

                assert_eq!(
                    program_headers[1],
                    ProgramHeader {
                        segment_type: SegmentType::Load,
                        segment_flags: vec![SegmentFlag::Execute, SegmentFlag::Read],
                        offset: 0x1000,
                        virtual_address: 0x401000,
                        file_size: 0x5a,
                        memory_size: 0x5a,
                        align: 0x1000
                    }
                );
            }

            ARCH::AARCH64 => {
                assert_eq!(program_headers.len(), 2);

                assert_eq!(
                    program_headers[0],
                    ProgramHeader {
                        segment_type: SegmentType::Load,
                        segment_flags: vec![SegmentFlag::Execute, SegmentFlag::Read],
                        offset: 0,
                        virtual_address: 0x400000,
                        file_size: 0x128,
                        memory_size: 0x128,
                        align: 0x10000
                    }
                );

                assert_eq!(
                    program_headers[1],
                    ProgramHeader {
                        segment_type: SegmentType::Load,
                        segment_flags: vec![SegmentFlag::Write, SegmentFlag::Read,],
                        offset: 0x128,
                        virtual_address: 0x410128,
                        file_size: 0x10,
                        memory_size: 0x20,
                        align: 0x10000
                    }
                );
            }
            _ => unimplemented!(),
        }
    }

    #[test]
    fn test_read_file_header_gcc_minimal_o() {
        // Note: check certain entries only for testing purposes.

        // Manually check with command `readelf -h gcc/minimal.o`
        let binary = get_example_file_binary("gcc/minimal.o");
        let elf = read_file(&binary).unwrap();
        let file_header = read_file_header(elf).unwrap();

        match get_arch() {
            ARCH::X86_64 => {
                assert_eq!(
                    file_header,
                    FileHeader {
                        os_abi: OSABI::SystemV,
                        machine: Machine::X86_64,
                        file_type: FileType::Relocatable,
                        entry_point: 0,
                        program_header_count: 0,
                        section_header_count: 12,
                    }
                );
            }
            ARCH::AARCH64 => {
                assert_eq!(
                    file_header,
                    FileHeader {
                        os_abi: OSABI::SystemV,
                        machine: Machine::AArch64,
                        file_type: FileType::Relocatable,
                        entry_point: 0,
                        program_header_count: 0,
                        section_header_count: 12,
                    }
                );
            }
            _ => unimplemented!(),
        }
    }

    #[test]
    fn test_read_file_header_gcc_minimal_elf() {
        // Manually check with command `readelf -h gcc/minimal.elf`
        let binary = get_example_file_binary("gcc/minimal.elf");
        let elf = read_file(&binary).unwrap();
        let file_header = read_file_header(elf).unwrap();

        match get_arch() {
            ARCH::X86_64 => {
                assert_eq!(
                    file_header,
                    FileHeader {
                        os_abi: OSABI::SystemV,
                        machine: Machine::X86_64,
                        file_type: FileType::Executable,
                        entry_point: 0x401042,
                        program_header_count: 7,
                        section_header_count: 9,
                    }
                );
            }
            ARCH::AARCH64 => {
                assert_eq!(
                    file_header,
                    FileHeader {
                        os_abi: OSABI::SystemV,
                        machine: Machine::AArch64,
                        file_type: FileType::Executable,
                        entry_point: 0x40014c,
                        program_header_count: 3,
                        section_header_count: 9,
                    }
                );
            }
            _ => unimplemented!(),
        }
    }

    #[test]
    fn test_read_section_header_gcc_minimal_o() {
        // Fields such as `size`, `binary`, `align`, and `offset` are not
        // intended to be tested here because they are not guaranteed
        // to be the same across different versions of the compiler and platforms.

        // Note: check certain entries only for testing purposes.

        // Manually check with command `readelf -S gcc/minimal.o`
        let binary = get_example_file_binary("gcc/minimal.o");
        let elf = read_file(&binary).unwrap();
        let sections = read_section_headers(elf, &binary).unwrap();

        match get_arch() {
            ARCH::X86_64 => {
                assert_eq!(sections.len(), 12);
                assert_eq!(
                    sections.iter().take(4).map(|s| &s.name).collect::<Vec<_>>(),
                    vec!["", ".text", ".data", ".bss",]
                );
                assert_eq!(
                    sections
                        .iter()
                        .take(4)
                        .map(|s| s.section_type)
                        .collect::<Vec<_>>(),
                    vec![
                        SectionType::Null,     // null
                        SectionType::Progbits, // .text
                        SectionType::Progbits, // .data
                        SectionType::Nobits,   // .bss
                    ]
                );
            }

            ARCH::AARCH64 => {
                assert_eq!(sections.len(), 12);
                assert_eq!(
                    sections.iter().take(4).map(|s| &s.name).collect::<Vec<_>>(),
                    vec!["", ".text", ".data", ".bss",]
                );
                assert_eq!(
                    sections
                        .iter()
                        .take(4)
                        .map(|s| s.section_type)
                        .collect::<Vec<_>>(),
                    vec![
                        SectionType::Null,     // null
                        SectionType::Progbits, // .text
                        SectionType::Progbits, // .data
                        SectionType::Nobits,   // .bss
                    ]
                );
            }
            _ => unimplemented!(),
        }
    }

    #[test]
    fn test_read_section_header_gcc_data_o() {
        // Manually check with command `readelf -S gcc/data.o`
        let binary = get_example_file_binary("gcc/data.o");
        let elf = read_file(&binary).unwrap();
        let sections = read_section_headers(elf, &binary).unwrap();

        match get_arch() {
            ARCH::X86_64 => {
                assert_eq!(sections.len(), 14);
                assert_eq!(
                    sections.iter().take(4).map(|s| &s.name).collect::<Vec<_>>(),
                    vec!["", ".text", ".rela.text", ".data",]
                );
                assert_eq!(
                    sections
                        .iter()
                        .take(4)
                        .map(|s| s.section_type)
                        .collect::<Vec<_>>(),
                    vec![
                        SectionType::Null,     // null
                        SectionType::Progbits, // .text
                        SectionType::Rela,     // .rela.text
                        SectionType::Progbits, // .data
                    ]
                );
            }

            ARCH::AARCH64 => {
                assert_eq!(sections.len(), 14);
                assert_eq!(
                    sections.iter().take(4).map(|s| &s.name).collect::<Vec<_>>(),
                    vec!["", ".text", ".rela.text", ".data",]
                );
                assert_eq!(
                    sections
                        .iter()
                        .take(4)
                        .map(|s| s.section_type)
                        .collect::<Vec<_>>(),
                    vec![
                        SectionType::Null,     // null
                        SectionType::Progbits, // .text
                        SectionType::Rela,     // .rela.text
                        SectionType::Progbits, // .data
                    ]
                );
            }
            _ => unimplemented!(),
        }
    }

    #[test]
    fn test_read_symbols_gcc_minimal_o() {
        // Note: check certain entries only for testing purposes.

        // Manually check with command `readelf -s gcc/minimal.o`
        let binary = get_example_file_binary("gcc/minimal.o");
        let elf = read_file(&binary).unwrap();
        let symbols = read_symbols(elf, &binary).unwrap();

        match get_arch() {
            ARCH::X86_64 => {
                assert_eq!(symbols.len(), 6);
                assert_eq!(symbols[0], Symbol::Other);
                assert_eq!(
                    symbols[5],
                    Symbol::Defined {
                        name: "_start".to_string(),
                        section_index: 1,
                        bind: SymbolBind::Global,
                        symbol_type: SymbolType::Func,
                        offset: 0x42,
                    }
                );
            }

            ARCH::AARCH64 => {
                assert_eq!(symbols.len(), 14);

                assert_eq!(symbols[0], Symbol::Other);

                assert_eq!(
                    symbols[1],
                    Symbol::Defined {
                        name: String::new(),
                        section_index: 1,
                        bind: SymbolBind::Local,
                        symbol_type: SymbolType::Section,
                        offset: 0,
                    }
                );

                assert_eq!(
                    symbols[5],
                    Symbol::Defined {
                        name: "_start".to_string(),
                        section_index: 1,
                        bind: SymbolBind::Global,
                        symbol_type: SymbolType::Notype,
                        offset: 0,
                    }
                );
            }
            _ => unimplemented!(),
        }
    }

    #[test]
    fn test_read_symbols_gcc_data_o() {
        // Manually check with command `readelf -s gcc/data.o`
        let binary = get_example_file_binary("gcc/data.o");
        let elf = read_file(&binary).unwrap();
        let symbols = read_symbols(elf, &binary).unwrap();

        match get_arch() {
            ARCH::X86_64 => {
                assert_eq!(symbols.len(), 12);

                assert_eq!(symbols[0], Symbol::Other);
                assert_eq!(
                    symbols[5],
                    Symbol::Defined {
                        name: "foo".to_string(),
                        section_index: 5,
                        bind: SymbolBind::Global,
                        symbol_type: SymbolType::Object,
                        offset: 0,
                    }
                );

                assert_eq!(
                    symbols[11],
                    Symbol::Defined {
                        name: "_start".to_string(),
                        section_index: 1,
                        bind: SymbolBind::Global,
                        symbol_type: SymbolType::Func,
                        offset: 0x42,
                    }
                );
            }

            ARCH::AARCH64 => {
                assert_eq!(symbols.len(), 16);

                assert_eq!(symbols[0], Symbol::Other);
                assert_eq!(
                    symbols[1],
                    Symbol::Defined {
                        name: String::new(),
                        section_index: 1,
                        bind: SymbolBind::Local,
                        symbol_type: SymbolType::Section,
                        offset: 0,
                    }
                );

                assert_eq!(
                    symbols[15],
                    Symbol::Defined {
                        name: "_start".to_string(),
                        section_index: 1,
                        bind: SymbolBind::Global,
                        symbol_type: SymbolType::Notype,
                        offset: 0,
                    }
                );
            }
            _ => unimplemented!(),
        }
    }

    #[test]
    fn test_read_symbols_gcc_symbol_import_o() {
        // Manually check with command `readelf -s gcc/symbol-import.o`
        let binary = get_example_file_binary("gcc/symbol-import.o");
        let elf = read_file(&binary).unwrap();
        let symbols = read_symbols(elf, &binary).unwrap();

        match get_arch() {
            ARCH::X86_64 => {
                assert_eq!(symbols.len(), 14);

                assert_eq!(symbols[0], Symbol::Other);
                assert_eq!(
                    symbols[5],
                    Symbol::Defined {
                        name: "_start".to_string(),
                        section_index: 1,
                        bind: SymbolBind::Global,
                        symbol_type: SymbolType::Func,
                        offset: 0x42,
                    }
                );

                assert_eq!(symbols[6], Symbol::External("foo".to_string()));
            }

            ARCH::AARCH64 => {
                assert_eq!(symbols.len(), 14);

                assert_eq!(symbols[0], Symbol::Other);
                assert_eq!(
                    symbols[1],
                    Symbol::Defined {
                        name: String::new(),
                        section_index: 1,
                        bind: SymbolBind::Local,
                        symbol_type: SymbolType::Section,
                        offset: 0,
                    }
                );

                assert_eq!(symbols[13], Symbol::External("y".to_string()));
            }
            _ => unimplemented!(),
        }
    }

    #[test]
    fn test_read_symbols_gcc_override_weak_o() {
        // Manually check with command `readelf -s gcc/override-weak.o`
        let binary = get_example_file_binary("gcc/override-weak.o");
        let elf = read_file(&binary).unwrap();
        let symbols = read_symbols(elf, &binary).unwrap();

        match get_arch() {
            ARCH::X86_64 => {
                assert_eq!(symbols.len(), 5);

                assert_eq!(symbols[0], Symbol::Other);
                assert_eq!(
                    symbols[3],
                    Symbol::Defined {
                        name: "foo".to_string(),
                        bind: SymbolBind::Weak,
                        symbol_type: SymbolType::Func,
                        section_index: 1,
                        offset: 0
                    }
                );
                assert_eq!(
                    symbols[4],
                    Symbol::Defined {
                        name: "bar".to_string(),
                        bind: SymbolBind::Weak,
                        symbol_type: SymbolType::Func,
                        section_index: 1,
                        offset: 0xb
                    }
                );
            }

            ARCH::AARCH64 => {
                assert_eq!(symbols.len(), 7);

                assert_eq!(symbols[0], Symbol::Other);
                assert_eq!(
                    symbols[1],
                    Symbol::Defined {
                        name: String::new(),
                        section_index: 1,
                        bind: SymbolBind::Local,
                        symbol_type: SymbolType::Section,
                        offset: 0,
                    }
                );

                assert_eq!(
                    symbols[6],
                    Symbol::Defined {
                        name: "bar".to_string(),
                        bind: SymbolBind::Weak,
                        symbol_type: SymbolType::Notype,
                        section_index: 1,
                        offset: 0x8
                    }
                );
            }
            _ => unimplemented!(),
        }
    }

    #[test]
    fn test_read_relocations_gcc_minimal_o() {
        // Note: check certain entries only for testing purposes.

        // Manually check with command `readelf -r gcc/minimal.o`
        let binary = get_example_file_binary("gcc/minimal.o");
        let elf = read_file(&binary).unwrap();
        let relocation_sections = read_relocation_sections(elf, &binary).unwrap();

        match get_arch() {
            ARCH::X86_64 => {
                assert_eq!(relocation_sections.len(), 1);

                let relocation_section = &relocation_sections[0];
                assert_eq!(relocation_section.name, ".rela.eh_frame");
                assert_eq!(relocation_section.target_section_index, 7); // index of `.eh_frame` section
            }
            ARCH::AARCH64 => {
                unimplemented!()
            }
            _ => unimplemented!(),
        }
    }

    #[test]
    fn test_read_relocations_gcc_data_o() {
        // Manually check with command `readelf -r gcc/data.o`
        let binary = get_example_file_binary("gcc/data.o");
        let elf = read_file(&binary).unwrap();
        let relocation_sections = read_relocation_sections(elf, &binary).unwrap();

        match get_arch() {
            ARCH::X86_64 => {
                assert_eq!(relocation_sections.len(), 2);

                let relocation_section = &relocation_sections[0];
                assert_eq!(relocation_section.name, ".rela.text");
                assert_eq!(relocation_section.target_section_index, 1);

                let relocations = &relocation_section.relocations;
                assert_eq!(relocations.len(), 8);

                assert_eq!(
                    relocations[0],
                    Relocation {
                        relocation_type: RelocationType::R_X86_64_PC32,
                        placeholder_offset: 0x52,
                        symbol_index: 7,
                        addend: -4
                    }
                );
                assert_eq!(
                    relocations[1],
                    Relocation {
                        relocation_type: RelocationType::R_X86_64_PC32,
                        placeholder_offset: 0x62,
                        symbol_index: 8,
                        addend: -4
                    }
                );
            }

            ARCH::AARCH64 => {
                assert_eq!(relocation_sections.len(), 1);

                let relocation_section = &relocation_sections[0];
                assert_eq!(relocation_section.name, ".rela.text");
                assert_eq!(relocation_section.target_section_index, 1);

                let relocations = &relocation_section.relocations;
                assert_eq!(relocations.len(), 20);

                assert_eq!(
                    relocations[0],
                    Relocation {
                        relocation_type: RelocationType::R_AARCH64_ADR_PREL_PG_HI21,
                        placeholder_offset: 0x0,
                        symbol_index: 4,
                        addend: 0
                    }
                );
                assert_eq!(
                    relocations[1],
                    Relocation {
                        relocation_type: RelocationType::R_AARCH64_LDST64_ABS_LO12_NC,
                        placeholder_offset: 0x4,
                        symbol_index: 4,
                        addend: 0
                    }
                );
            }
            _ => unimplemented!(),
        }
    }

    #[test]
    fn test_read_relocations_gcc_relocate_within_data_no_pie_o() {
        // Note:
        // Don't test relocation sections with `relocate-within-data.o` because
        // the GCC compiler generates ".rela.data.rel.local" and ".rela.data.rel.ro.local"
        // because it uses the `-fPIE` flag by default, which is not supported by this crate.

        // Manually check with command `readelf -r gcc/relocate-within-data-no-pie.o`
        let binary = get_example_file_binary("gcc/relocate-within-data-no-pie.o");
        let elf = read_file(&binary).unwrap();
        let relocation_sections = read_relocation_sections(elf, &binary).unwrap();

        match get_arch() {
            ARCH::X86_64 => {
                assert_eq!(relocation_sections.len(), 4);

                // `.rela.text`
                {
                    let relocation_section = &relocation_sections[0];
                    assert_eq!(relocation_section.name, ".rela.text");
                    assert_eq!(relocation_section.target_section_index, 1); // `.text` section index

                    let relocations = &relocation_section.relocations;
                    assert_eq!(relocations.len(), 8);

                    assert_eq!(
                        relocations[0],
                        Relocation {
                            relocation_type: RelocationType::R_X86_64_32,
                            placeholder_offset: 0x6f,
                            symbol_index: 5,
                            addend: 0
                        }
                    );
                    assert_eq!(
                        relocations[1],
                        Relocation {
                            relocation_type: RelocationType::R_X86_64_PC32,
                            placeholder_offset: 0x7d,
                            symbol_index: 9,
                            addend: -4
                        }
                    );
                }

                // `.rela.data`
                {
                    let relocation_section = &relocation_sections[1];
                    assert_eq!(relocation_section.name, ".rela.data");
                    assert_eq!(relocation_section.target_section_index, 3);

                    let relocations = &relocation_section.relocations;
                    assert_eq!(relocations.len(), 2);

                    assert_eq!(
                        relocations[0],
                        Relocation {
                            relocation_type: RelocationType::R_X86_64_64,
                            placeholder_offset: 0x10,
                            symbol_index: 0x7,
                            addend: 0
                        }
                    );
                    assert_eq!(
                        relocations[1],
                        Relocation {
                            relocation_type: RelocationType::R_X86_64_64,
                            placeholder_offset: 0x18,
                            symbol_index: 0x8,
                            addend: 0
                        }
                    );
                }

                // `.rela.rodata`
                {
                    let relocation_section = &relocation_sections[2];
                    assert_eq!(relocation_section.name, ".rela.rodata");
                    assert_eq!(relocation_section.target_section_index, 6);

                    let relocations = &relocation_section.relocations;
                    assert_eq!(relocations.len(), 2);

                    assert_eq!(
                        relocations[0],
                        Relocation {
                            relocation_type: RelocationType::R_X86_64_64,
                            placeholder_offset: 0,
                            symbol_index: 5,
                            addend: 0
                        }
                    );
                    assert_eq!(
                        relocations[1],
                        Relocation {
                            relocation_type: RelocationType::R_X86_64_64,
                            placeholder_offset: 0x8,
                            symbol_index: 6,
                            addend: 0
                        }
                    );
                }
            }

            ARCH::AARCH64 => {
                assert_eq!(relocation_sections.len(), 3);

                // `.rela.text`
                {
                    let relocation_section = &relocation_sections[0];
                    assert_eq!(relocation_section.name, ".rela.text");
                    assert_eq!(relocation_section.target_section_index, 1);
                    let relocations = &relocation_section.relocations;
                    assert_eq!(relocations.len(), 12);

                    assert_eq!(
                        relocations[0],
                        Relocation {
                            relocation_type: RelocationType::R_AARCH64_ADR_PREL_PG_HI21,
                            placeholder_offset: 0x10,
                            symbol_index: 0xb,
                            addend: 0
                        }
                    );
                    assert_eq!(
                        relocations[1],
                        Relocation {
                            relocation_type: RelocationType::R_AARCH64_LDST64_ABS_LO12_NC,
                            placeholder_offset: 0x14,
                            symbol_index: 0xb,
                            addend: 0
                        }
                    );
                }

                // `.rela.data`
                {
                    let relocation_section = &relocation_sections[1];
                    assert_eq!(relocation_section.name, ".rela.data");
                    assert_eq!(relocation_section.target_section_index, 3);

                    let relocations = &relocation_section.relocations;
                    assert_eq!(relocations.len(), 2);

                    assert_eq!(
                        relocations[0],
                        Relocation {
                            relocation_type: RelocationType::R_AARCH64_ABS64,
                            placeholder_offset: 0x20,
                            symbol_index: 0x13,
                            addend: 0
                        }
                    );
                    assert_eq!(
                        relocations[1],
                        Relocation {
                            relocation_type: RelocationType::R_AARCH64_ABS64,
                            placeholder_offset: 0x28,
                            symbol_index: 0x14,
                            addend: 0
                        }
                    );
                }

                // `.rela.rodata`
                {
                    let relocation_section = &relocation_sections[2];
                    assert_eq!(relocation_section.name, ".rela.rodata");
                    assert_eq!(relocation_section.target_section_index, 6);

                    let relocations = &relocation_section.relocations;
                    assert_eq!(relocations.len(), 2);

                    assert_eq!(
                        relocations[0],
                        Relocation {
                            relocation_type: RelocationType::R_AARCH64_ABS64,
                            placeholder_offset: 0,
                            symbol_index: 2,
                            addend: 0
                        }
                    );
                    assert_eq!(
                        relocations[1],
                        Relocation {
                            relocation_type: RelocationType::R_AARCH64_ABS64,
                            placeholder_offset: 0x8,
                            symbol_index: 2,
                            addend: 8
                        }
                    );
                }
            }
            _ => unimplemented!(),
        }
    }

    #[test]
    fn test_read_program_headers_gcc_minimal_o() {
        // Note: check certain entries only for testing purposes.

        // Manually check with command `readelf -l gcc/minimal.o`
        let binary = get_example_file_binary("gcc/minimal.o");
        let elf = read_file(&binary).unwrap();
        let program_headers = read_program_headers(elf, &binary).unwrap();

        assert_eq!(program_headers.len(), 0);
    }

    #[test]
    fn test_read_program_headers_gcc_minimal_elf() {
        // Manually check with command `readelf -l gcc/minimal.elf`
        let binary = get_example_file_binary("gcc/minimal.elf");
        let elf = read_file(&binary).unwrap();
        let program_headers = read_program_headers(elf, &binary).unwrap();

        match get_arch() {
            ARCH::X86_64 => {
                assert_eq!(program_headers.len(), 7);

                assert_eq!(
                    program_headers[0],
                    ProgramHeader {
                        segment_type: SegmentType::Load,
                        segment_flags: vec![SegmentFlag::Read,],
                        offset: 0,
                        virtual_address: 0x400000,
                        file_size: 0x1ec, // includes the section `.note.gnu.build-id`
                        memory_size: 0x1ec,
                        align: 0x1000
                    }
                );

                assert_eq!(
                    program_headers[1],
                    ProgramHeader {
                        segment_type: SegmentType::Load,
                        segment_flags: vec![SegmentFlag::Execute, SegmentFlag::Read],
                        offset: 0x1000,
                        virtual_address: 0x401000,
                        file_size: 0x50,
                        memory_size: 0x50,
                        align: 0x1000
                    }
                );
            }

            ARCH::AARCH64 => {
                assert_eq!(program_headers.len(), 1);

                assert_eq!(
                    program_headers[0],
                    ProgramHeader {
                        segment_type: SegmentType::Load,
                        segment_flags: vec![SegmentFlag::Execute, SegmentFlag::Read],
                        offset: 0,
                        virtual_address: 0x400000,
                        file_size: 0x84,
                        memory_size: 0x84,
                        align: 0x10000
                    }
                );
            }
            _ => unimplemented!(),
        }
    }

    #[test]
    fn test_read_program_headers_gcc_data_elf() {
        // Manually check with command `readelf -l gcc/data.elf`

        let binary = get_example_file_binary("gcc/data.elf");
        let elf = read_file(&binary).unwrap();
        let program_headers = read_program_headers(elf, &binary).unwrap();

        match get_arch() {
            ARCH::X86_64 => {
                assert_eq!(program_headers.len(), 8);

                assert_eq!(
                    program_headers[0],
                    ProgramHeader {
                        segment_type: SegmentType::Load,
                        segment_flags: vec![SegmentFlag::Read,],
                        offset: 0,
                        virtual_address: 0x400000,
                        file_size: 0x224, // includes the section `.note.gnu.build-id`
                        memory_size: 0x224,
                        align: 0x1000
                    }
                );

                assert_eq!(
                    program_headers[1],
                    ProgramHeader {
                        segment_type: SegmentType::Load,
                        segment_flags: vec![SegmentFlag::Execute, SegmentFlag::Read],
                        offset: 0x1000,
                        virtual_address: 0x401000,
                        file_size: 0x9a,
                        memory_size: 0x9a,
                        align: 0x1000
                    }
                );
            }

            ARCH::AARCH64 => {
                assert_eq!(program_headers.len(), 2);

                assert_eq!(
                    program_headers[0],
                    ProgramHeader {
                        segment_type: SegmentType::Load,
                        segment_flags: vec![SegmentFlag::Execute, SegmentFlag::Read],
                        offset: 0,
                        virtual_address: 0x400000,
                        file_size: 0x128,
                        memory_size: 0x128,
                        align: 0x10000
                    }
                );

                assert_eq!(
                    program_headers[1],
                    ProgramHeader {
                        segment_type: SegmentType::Load,
                        segment_flags: vec![SegmentFlag::Write, SegmentFlag::Read,],
                        offset: 0x128,
                        virtual_address: 0x410128,
                        file_size: 0x10,
                        memory_size: 0x20,
                        align: 0x10000
                    }
                );
            }
            _ => unimplemented!(),
        }
    }
}
