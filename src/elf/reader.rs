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
        DataEncoding, FileClass, FileType, Machine, OSABI, Relocation, RelocationType, SectionType,
        SegmentFlag, SegmentType, Symbol, SymbolBind, SymbolType,
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

    let data_encoding = DataEncoding::from(elf.e_ident.data);
    let file_class = FileClass::from(elf.e_ident.class);
    let os_abi = OSABI::from(elf.e_ident.os_abi);
    let file_type = FileType::from(elf.e_type(endian));
    let machine = Machine::from(elf.e_machine(endian));

    let entry_point = elf.e_entry(endian) as usize;
    let program_header_count = elf.e_phnum(endian) as usize;
    let section_header_count = elf.e_shnum(endian) as usize;

    Ok(super::module::FileHeader {
        data_encoding,
        file_class,
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
    let machine = Machine::from(elf.e_machine(endian));

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

            let relocations = parse_relocations(relas, endian, is_mips64el, machine)?;

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
    machine: Machine,
) -> Result<Vec<Relocation>, LinkerError> {
    let mut relocations = Vec::new();

    for rela in relas {
        let relocation_type_raw = rela.r_type(endian, is_mips64el);

        // The assembler emits relaxation markers alongside the actual
        // relocations. They do not patch a field and are not needed by the
        // reader's relocation model.
        if machine == Machine::LoongArch && relocation_type_raw == object::elf::R_LARCH_RELAX {
            continue;
        }

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
        let relocation_type = parse_relocation_type(machine, relocation_type_raw)?;

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

fn parse_relocation_type(
    machine: Machine,
    relocation_type_raw: u32,
) -> Result<RelocationType, LinkerError> {
    match machine {
        Machine::X86_64 => {
            match relocation_type_raw {
                object::elf::R_X86_64_PC32 => Ok(RelocationType::R_X86_64_PC32),
                object::elf::R_X86_64_PLT32 => Ok(RelocationType::R_X86_64_PLT32),
                object::elf::R_X86_64_64 => Ok(RelocationType::R_X86_64_64),
                object::elf::R_X86_64_32 => Ok(RelocationType::R_X86_64_32),
                object::elf::R_X86_64_TPOFF32 => Ok(RelocationType::R_X86_64_TPOFF32),
                /* unsupported */
                _ => Err(LinkerError::new(&format!(
                    "Unsupported relocation type \"{relocation_type_raw}\" for x86_64 architecture"
                ))),
            }
        }
        Machine::AArch64 => {
            match relocation_type_raw {
                object::elf::R_AARCH64_ADR_PREL_PG_HI21 => {
                    Ok(RelocationType::R_AARCH64_ADR_PREL_PG_HI21)
                }
                object::elf::R_AARCH64_ADD_ABS_LO12_NC => {
                    Ok(RelocationType::R_AARCH64_ADD_ABS_LO12_NC)
                }
                object::elf::R_AARCH64_LDST64_ABS_LO12_NC => {
                    Ok(RelocationType::R_AARCH64_LDST64_ABS_LO12_NC)
                }
                object::elf::R_AARCH64_CALL26 => Ok(RelocationType::R_AARCH64_CALL26),
                object::elf::R_AARCH64_ABS64 => Ok(RelocationType::R_AARCH64_ABS64),
                /* unsupported */
                _ => Err(LinkerError::new(&format!(
                    "Unsupported relocation type \"{relocation_type_raw}\" for AArch64 architecture"
                ))),
            }
        }
        Machine::RiscV => {
            match relocation_type_raw {
                object::elf::R_RISCV_PCREL_HI20 => Ok(RelocationType::R_RISCV_PCREL_HI20),
                object::elf::R_RISCV_PCREL_LO12_I => Ok(RelocationType::R_RISCV_PCREL_LO12_I),

                object::elf::R_RISCV_HI20 => Ok(RelocationType::R_RISCV_HI20),
                object::elf::R_RISCV_LO12_I => Ok(RelocationType::R_RISCV_LO12_I),
                object::elf::R_RISCV_LO12_S => Ok(RelocationType::R_RISCV_LO12_S),

                object::elf::R_RISCV_CALL_PLT => Ok(RelocationType::R_RISCV_CALL_PLT),
                object::elf::R_RISCV_64 => Ok(RelocationType::R_RISCV_64),
                /* unsupported */
                _ => Err(LinkerError::new(&format!(
                    "Unsupported relocation type \"{relocation_type_raw}\" for RISC-V architecture"
                ))),
            }
        }
        Machine::LoongArch => {
            match relocation_type_raw {
                object::elf::R_LARCH_PCALA_HI20 => Ok(RelocationType::R_LARCH_PCALA_HI20),
                object::elf::R_LARCH_PCALA_LO12 => Ok(RelocationType::R_LARCH_PCALA_LO12),
                object::elf::R_LARCH_64 => Ok(RelocationType::R_LARCH_64),
                object::elf::R_LARCH_CALL36 => Ok(RelocationType::R_LARCH_CALL36),
                /* unsupported */
                _ => Err(LinkerError::new(&format!(
                    "Unsupported relocation type \"{relocation_type_raw}\" for LoongArch architecture"
                ))),
            }
        }
        Machine::PowerPC64 => {
            // todo
            todo!()
        }
        Machine::S390 => {
            // todo
            todo!()
        }
        Machine::Other(_) => unimplemented!(),
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
    use std::{fmt::Display, vec};

    use crate::elf::{
        module::{
            DataEncoding, FileClass, FileType, Machine, OSABI, RelocationType, SectionType,
            SegmentFlag, SegmentType, Symbol, SymbolBind, SymbolType,
        },
        reader::{
            read_file, read_file_header, read_program_headers, read_relocation_sections,
            read_section_headers, read_symbols,
        },
    };

    #[derive(Debug, PartialEq, Clone, Copy)]
    enum SourceType {
        Assembly,

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

    const IMPLEMENTED_ARCHS: &[Machine] = &[
        Machine::X86_64,
        Machine::AArch64,
        Machine::RiscV,
        Machine::LoongArch,
        Machine::PowerPC64,
        Machine::S390,
    ];

    fn get_arch_dir_name(arch: &Machine) -> &'static str {
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

    fn get_example_file_binary(
        source_type: SourceType,
        arch: &Machine,
        file_name: &str,
    ) -> Vec<u8> {
        let file_path = std::env::current_dir()
            .unwrap()
            .join("resources/examples/elf")
            .join(source_type.to_string())
            .join(get_arch_dir_name(arch))
            .join(file_name);

        std::fs::read(file_path).unwrap()
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

    // ===============================================
    // Assembly programs compiled with `as` for testing purposes
    // ===============================================

    #[test]
    fn test_read_file_header_asm_minimal_o() {
        // Manually check with command `readelf -h asm/ARCH/minimal.o`

        for arch in IMPLEMENTED_ARCHS {
            let binary = get_example_file_binary(SourceType::Assembly, arch, "minimal.o");
            let elf = read_file(&binary).unwrap();
            let file_header = read_file_header(elf).unwrap();

            assert_eq!(file_header.file_type, FileType::Relocatable);
            assert_eq!(file_header.os_abi, OSABI::SystemV);
            assert_eq!(file_header.file_class, FileClass::Elf64);

            match arch {
                Machine::X86_64 => {
                    assert_eq!(file_header.machine, Machine::X86_64);
                    assert_eq!(file_header.data_encoding, DataEncoding::LittleEndian);
                }
                Machine::AArch64 => {
                    assert_eq!(file_header.machine, Machine::AArch64);
                    assert_eq!(file_header.data_encoding, DataEncoding::LittleEndian);
                }
                Machine::RiscV => {
                    assert_eq!(file_header.machine, Machine::RiscV);
                    assert_eq!(file_header.data_encoding, DataEncoding::LittleEndian);
                }
                Machine::LoongArch => {
                    assert_eq!(file_header.machine, Machine::LoongArch);
                    assert_eq!(file_header.data_encoding, DataEncoding::LittleEndian);
                }
                Machine::PowerPC64 => {
                    assert_eq!(file_header.machine, Machine::PowerPC64);
                    assert_eq!(file_header.data_encoding, DataEncoding::LittleEndian);
                }
                Machine::S390 => {
                    assert_eq!(file_header.machine, Machine::S390);
                    assert_eq!(file_header.data_encoding, DataEncoding::BigEndian);
                }
                Machine::Other(_) => unimplemented!(),
            }
        }
    }

    #[test]
    fn test_read_file_header_asm_minimal_elf() {
        // Manually check with command `readelf -h asm/ARCH/minimal.elf`

        for arch in IMPLEMENTED_ARCHS {
            let binary = get_example_file_binary(SourceType::Assembly, arch, "minimal.elf");
            let elf = read_file(&binary).unwrap();
            let file_header = read_file_header(elf).unwrap();

            assert_eq!(file_header.file_type, FileType::Executable);
        }
    }

    #[test]
    fn test_read_section_header_asm_minimal_o() {
        // Fields such as `size`, `binary`, `align`, and `offset` are not
        // intended to be tested here because they are not guaranteed
        // to be the same across different versions of the assembler and platforms.

        for arch in IMPLEMENTED_ARCHS {
            // Manually check with command `readelf -S asm/ARCH/minimal.o`
            let binary = get_example_file_binary(SourceType::Assembly, arch, "minimal.o");
            let elf = read_file(&binary).unwrap();
            let sections = read_section_headers(elf, &binary).unwrap();

            // Check section names
            assert_contains_all(
                &sections.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
                &[".text", ".data", ".bss", ".symtab", ".strtab", ".shstrtab"],
            );

            // Check section types

            // The first section header is always a NULL section header, which is reserved and has no name.
            assert!(matches!(sections.first(), Some(s) if s.section_type == SectionType::Null));

            assert!(matches!(
                sections
                    .iter()
                    .find(|s| s.name == ".text"),
                Some(s) if s.section_type == SectionType::Progbits
            ));

            assert!(matches!(
                sections
                    .iter()
                    .find(|s| s.name == ".data"),
                Some(s) if s.section_type == SectionType::Progbits
            ));

            assert!(matches!(
                sections
                    .iter()
                    .find(|s| s.name == ".bss"),
                Some(s) if s.section_type == SectionType::Nobits
            ));

            assert!(matches!(
                sections
                    .iter()
                    .find(|s| s.name == ".symtab"),
                Some(s) if s.section_type == SectionType::Symtab
            ));

            assert!(matches!(
                sections
                    .iter()
                    .find(|s| s.name == ".strtab"),
                Some(s) if s.section_type == SectionType::Strtab
            ));

            assert!(matches!(
                sections
                    .iter()
                    .find(|s| s.name == ".shstrtab"),
                Some(s) if s.section_type == SectionType::Strtab
            ));
        }
    }

    #[test]
    fn test_read_section_header_asm_function_o() {
        // Manually check with command `readelf -S asm/ARCH/function.o`
        for arch in IMPLEMENTED_ARCHS {
            let binary = get_example_file_binary(SourceType::Assembly, arch, "function.o");
            let elf = read_file(&binary).unwrap();
            let sections = read_section_headers(elf, &binary).unwrap();

            // Check section names
            assert_contains_all(
                &sections.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
                &[
                    ".text",
                    ".rela.text",
                    ".data",
                    ".bss",
                    ".rodata",
                    ".symtab",
                    ".strtab",
                    ".shstrtab",
                ],
            );

            // Check section types
            assert!(matches!(
                sections
                    .iter()
                    .find(|s| s.name == ".text"),
                Some(s) if s.section_type == SectionType::Progbits
            ));

            assert!(matches!(
                sections
                    .iter()
                    .find(|s| s.name == ".bss"),
                Some(s) if s.section_type == SectionType::Nobits
            ));

            assert!(matches!(
                sections
                    .iter()
                    .find(|s| s.name == ".rela.text"),
                Some(s) if s.section_type == SectionType::Rela
            ));
        }
    }

    #[test]
    fn test_read_symbols_asm_minimal_o() {
        // Manually check with command `readelf -s asm/ARCH/minimal.o`
        for arch in IMPLEMENTED_ARCHS {
            let binary = get_example_file_binary(SourceType::Assembly, arch, "minimal.o");
            let elf = read_file(&binary).unwrap();
            let symbols = read_symbols(elf, &binary).unwrap();

            // The first symbol table entry (index 0) is reserved and must be undefined.
            assert_eq!(symbols[0], Symbol::Other);

            // Assembler generates `Notype` for function symbols.
            assert!(matches!(
                symbols
                    .iter()
                    .find(|s| matches!(s, Symbol::Defined {name,..} if name == "_start")),
                Some(Symbol::Defined {
                    bind: SymbolBind::Global,
                    symbol_type: SymbolType::Notype,
                    ..
                })
            ));
        }
    }

    #[test]
    fn test_read_symbols_asm_function_o() {
        for arch in IMPLEMENTED_ARCHS {
            // Manually check with command `readelf -s asm/ARCH/function.o`
            let binary = get_example_file_binary(SourceType::Assembly, arch, "function.o");
            let elf = read_file(&binary).unwrap();
            let symbols = read_symbols(elf, &binary).unwrap();

            // Check symbol names
            assert_contains_all(
                &symbols
                    .iter()
                    .filter(|s| matches!(s, Symbol::Defined { .. }))
                    .map(|s| {
                        if let Symbol::Defined { name, .. } = s {
                            name.as_str()
                        } else {
                            unreachable!()
                        }
                    })
                    .collect::<Vec<_>>(),
                &["print_hello", "print_world"],
            );

            // Check symbols names, types and binds, but not check the section index and offset,
            // because they may vary across different platforms and versions of the assembler.

            // Check certain entries for testing purposes.

            // Assembler generates `Notype` for function symbols.
            assert!(matches!(
                symbols
                    .iter()
                    .find(|s| matches!(s, Symbol::Defined { name, .. } if name == "print_hello")),
                Some(Symbol::Defined {
                    bind: SymbolBind::Local,
                    symbol_type: SymbolType::Notype,
                    ..
                })
            ));

            assert!(matches!(
                symbols
                    .iter()
                    .find(|s| matches!(s, Symbol::Defined { name, .. } if name == "print_world")),
                Some(Symbol::Defined {
                    bind: SymbolBind::Local,
                    symbol_type: SymbolType::Notype,
                    ..
                })
            ));
        }
    }

    #[test]
    fn test_read_symbols_asm_data_o() {
        for arch in IMPLEMENTED_ARCHS {
            // Manually check with command `readelf -s asm/ARCH/data.o`
            let binary = get_example_file_binary(SourceType::Assembly, arch, "data.o");
            let elf = read_file(&binary).unwrap();
            let symbols = read_symbols(elf, &binary).unwrap();

            // Check symbol names
            assert_contains_all(
                &symbols
                    .iter()
                    .filter(|s| matches!(s, Symbol::Defined { .. }))
                    .map(|s| {
                        if let Symbol::Defined { name, .. } = s {
                            name.as_str()
                        } else {
                            unreachable!()
                        }
                    })
                    .collect::<Vec<_>>(),
                &["foo", "bar", "a", "b", "x", "y", "_start"],
            );

            // Check symbols names, types and binds, but not check the section index and offset,
            // because they may vary across different platforms and versions of the assembler.

            // Check certain entries for testing purposes.

            // Assembler generates `Notype` for data symbols.
            assert!(matches!(
                symbols
                    .iter()
                    .find(|s| matches!(s, Symbol::Defined { name, .. } if name == "foo")),
                Some(Symbol::Defined {
                    bind: SymbolBind::Local,
                    symbol_type: SymbolType::Notype,
                    ..
                })
            ));

            assert!(matches!(
                symbols
                    .iter()
                    .find(|s| matches!(s, Symbol::Defined { name, .. } if name == "bar")),
                Some(Symbol::Defined {
                    bind: SymbolBind::Local,
                    symbol_type: SymbolType::Notype,
                    ..
                })
            ));

            match arch {
                Machine::X86_64 => {
                    // No architecture-specific symbol is emitted for x86_64.
                }
                Machine::AArch64 => {
                    // No architecture-specific symbol is emitted for aarch64.
                }
                Machine::RiscV => {
                    // The assembler generates a special symbol `__global_pointer$` for the global pointer register (gp).
                    assert!(
                        symbols
                            .iter()
                            .find(|s| matches!(s, Symbol::External(name) if name == "__global_pointer$"))
                            .is_some()
                    );
                }
                Machine::LoongArch => {
                    // No architecture-specific symbol is emitted for LoongArch.
                }
                Machine::PowerPC64 => {
                    // todo
                }
                Machine::S390 => {
                    // todo
                }
                Machine::Other(_) => unimplemented!(),
            }
        }
    }

    #[test]
    fn test_read_symbols_asm_symbol_export_o() {
        for arch in IMPLEMENTED_ARCHS {
            // Manually check with command `readelf -s asm/ARCH/symbol-export.o`
            let binary = get_example_file_binary(SourceType::Assembly, arch, "symbol-export.o");
            let elf = read_file(&binary).unwrap();
            let symbols = read_symbols(elf, &binary).unwrap();

            // Check symbol names
            assert_contains_all(
                &symbols
                    .iter()
                    .filter(|s| matches!(s, Symbol::Defined { .. }))
                    .map(|s| {
                        if let Symbol::Defined { name, .. } = s {
                            name.as_str()
                        } else {
                            unreachable!()
                        }
                    })
                    .collect::<Vec<_>>(),
                &["foo", "bar", "a", "b", "x", "y", "dec", "inc"],
            );

            // Check symbols names, types and binds, but not check the section index and offset,
            // because they may vary across different platforms and versions of the assembler.

            // Check certain entries for testing purposes.

            // Assembler generates `Notype` for data and function symbols.
            assert!(matches!(
                symbols
                    .iter()
                    .find(|s| matches!(s, Symbol::Defined { name, .. } if name == "foo")),
                Some(Symbol::Defined {
                    bind: SymbolBind::Global,
                    symbol_type: SymbolType::Notype,
                    ..
                })
            ));

            assert!(matches!(
                symbols
                    .iter()
                    .find(|s| matches!(s, Symbol::Defined { name, .. } if name == "bar")),
                Some(Symbol::Defined {
                    bind: SymbolBind::Global,
                    symbol_type: SymbolType::Notype,
                    ..
                })
            ));

            assert!(matches!(
                symbols
                    .iter()
                    .find(|s| matches!(s, Symbol::Defined { name, .. } if name == "dec")),
                Some(Symbol::Defined {
                    bind: SymbolBind::Global,
                    symbol_type: SymbolType::Notype,
                    ..
                })
            ));

            assert!(matches!(
                symbols
                    .iter()
                    .find(|s| matches!(s, Symbol::Defined { name, .. } if name == "inc")),
                Some(Symbol::Defined {
                    bind: SymbolBind::Global,
                    symbol_type: SymbolType::Notype,
                    ..
                })
            ));
        }
    }

    #[test]
    fn test_read_symbols_asm_symbol_import_o() {
        // Manually check with command `readelf -s asm/ARCH/symbol-import.o`

        for arch in IMPLEMENTED_ARCHS {
            let binary = get_example_file_binary(SourceType::Assembly, arch, "symbol-import.o");
            let elf = read_file(&binary).unwrap();
            let symbols = read_symbols(elf, &binary).unwrap();

            // Check imported symbol names
            assert_contains_all(
                &symbols
                    .iter()
                    .filter(|s| matches!(s, Symbol::External(_)))
                    .map(|s| {
                        if let Symbol::External(name) = s {
                            name.as_str()
                        } else {
                            unreachable!()
                        }
                    })
                    .collect::<Vec<_>>(),
                &["foo", "bar", "a", "b", "x", "y", "inc", "dec"],
            );
        }
    }

    #[test]
    fn test_read_symbols_asm_override_weak_o() {
        // Manually check with command `readelf -s asm/ARCH/override-weak.o`

        for arch in IMPLEMENTED_ARCHS {
            let binary = get_example_file_binary(SourceType::Assembly, arch, "override-weak.o");
            let elf = read_file(&binary).unwrap();
            let symbols = read_symbols(elf, &binary).unwrap();

            // Check weak symbol names
            assert_contains_all(
                &symbols
                    .iter()
                    .filter(
                        |s| matches!(s, Symbol::Defined { bind, .. } if *bind == SymbolBind::Weak),
                    )
                    .map(|s| {
                        if let Symbol::Defined { name, .. } = s {
                            name.as_str()
                        } else {
                            unreachable!()
                        }
                    })
                    .collect::<Vec<_>>(),
                &["foo", "bar"],
            );
        }
    }

    #[test]
    fn test_read_symbols_asm_override_strong_o() {
        // Manually check with command `readelf -s asm/ARCH/override-strong.o`

        for arch in IMPLEMENTED_ARCHS {
            let binary = get_example_file_binary(SourceType::Assembly, arch, "override-strong.o");
            let elf = read_file(&binary).unwrap();
            let symbols = read_symbols(elf, &binary).unwrap();

            // Check import symbol
            assert!(
                symbols
                    .iter()
                    .find(|s| matches!(s, Symbol::External(name) if name == "foo"))
                    .is_some()
            );

            // Check strong symbol

            // Assembler generates `Notype` for function symbols.
            assert!(matches!(
                symbols
                    .iter()
                    .find(|s| matches!(s, Symbol::Defined { name, .. } if name == "bar")),
                Some(Symbol::Defined {
                    bind: SymbolBind::Local,
                    symbol_type: SymbolType::Notype,
                    ..
                })
            ));
        }
    }

    #[test]
    fn test_read_relocations_asm_function_o() {
        // Manually check with command `readelf -r asm/ARCH/function.o`

        for arch in IMPLEMENTED_ARCHS {
            let binary = get_example_file_binary(SourceType::Assembly, arch, "function.o");
            let elf = read_file(&binary).unwrap();
            let sections = read_section_headers(elf, &binary).unwrap();
            let symbols = read_symbols(elf, &binary).unwrap();
            let mut relocation_sections = read_relocation_sections(elf, &binary).unwrap();

            // Check relocation section `.rela.text`
            let relocation_section_opt = relocation_sections
                .iter_mut()
                .find(|s| s.name == ".rela.text");

            assert!(relocation_section_opt.is_some());

            let relocation_section = relocation_section_opt.unwrap();
            assert_eq!(
                sections[relocation_section.target_section_index].name,
                ".text"
            );

            // Check relocation entries in `.rela.text` section
            let relocations = &mut relocation_section.relocations;

            // Sort the relocations by placeholder_offset to ensure consistent order for testing.
            relocations.sort_by_key(|a| a.placeholder_offset);

            // Check certain entries for testing purposes.
            match arch {
                Machine::X86_64 => {
                    // The assembler generates relocations with offset that refers to the section symbols, not the actual symbols.
                    let relocation0 = &relocations[0];
                    let symbol0 = &symbols[relocation0.symbol_index];
                    assert_eq!(relocation0.relocation_type, RelocationType::R_X86_64_PC32);
                    assert!(
                        matches!(symbol0, Symbol::Defined { section_index, ..} if sections[*section_index].name == ".rodata")
                    );

                    let relocation1 = &relocations[1];
                    let symbol1 = &symbols[relocation1.symbol_index];
                    assert_eq!(relocation1.relocation_type, RelocationType::R_X86_64_PC32);
                    assert!(
                        matches!(symbol1, Symbol::Defined { section_index, ..} if sections[*section_index].name == ".rodata")
                    );
                }
                Machine::AArch64 => {
                    // The assembler generates relocations with offset that refers to the section symbols, not the actual symbols.

                    // Combination: R_AARCH64_ADR_PREL_PG_HI21 + R_AARCH64_ADD_ABS_LO12_NC

                    let relocation0 = &relocations[0];
                    let symbol0 = &symbols[relocation0.symbol_index];
                    assert_eq!(
                        relocation0.relocation_type,
                        RelocationType::R_AARCH64_ADR_PREL_PG_HI21
                    );
                    assert!(
                        matches!(symbol0, Symbol::Defined { section_index, ..} if sections[*section_index].name == ".rodata")
                    );

                    let relocation1 = &relocations[1];
                    let symbol1 = &symbols[relocation1.symbol_index];
                    assert_eq!(
                        relocation1.relocation_type,
                        RelocationType::R_AARCH64_ADD_ABS_LO12_NC
                    );
                    assert!(
                        matches!(symbol1, Symbol::Defined { section_index, ..} if sections[*section_index].name == ".rodata")
                    );
                }
                Machine::RiscV => {
                    // Relocation for symbol `hello`: R_RISCV_PCREL_HI20 + R_RISCV_PCREL_LO12_I

                    let relocation0 = &relocations[0];
                    let symbol0 = &symbols[relocation0.symbol_index];
                    assert_eq!(
                        relocation0.relocation_type,
                        RelocationType::R_RISCV_PCREL_HI20
                    );
                    assert!(matches!(symbol0, Symbol::Defined { name, ..} if name == "hello"));

                    let relocation1 = &relocations[1];
                    let symbol1 = &symbols[relocation1.symbol_index];
                    assert_eq!(
                        relocation1.relocation_type,
                        RelocationType::R_RISCV_PCREL_LO12_I
                    );
                    assert!(matches!(symbol1, Symbol::Defined { name, ..} if name == ".L0 ")); // Note that this symbol name has a trailing space
                }
                Machine::LoongArch => {
                    // Relocation for symbol `hello`: R_LARCH_PCALA_HI20 + R_LARCH_PCALA_LO12

                    let relocation0 = &relocations[0];
                    let symbol0 = &symbols[relocation0.symbol_index];
                    assert_eq!(
                        relocation0.relocation_type,
                        RelocationType::R_LARCH_PCALA_HI20
                    );
                    assert!(matches!(symbol0, Symbol::Defined { name, ..} if name == "hello"));

                    let relocation1 = &relocations[1];
                    let symbol1 = &symbols[relocation1.symbol_index];
                    assert_eq!(
                        relocation1.relocation_type,
                        RelocationType::R_LARCH_PCALA_LO12
                    );
                    assert!(matches!(symbol1, Symbol::Defined { name, ..} if name == "hello"));
                }
                Machine::PowerPC64 => {
                    // todo
                }
                Machine::S390 => {
                    // todo
                }
                Machine::Other(_) => unimplemented!(),
            }
        }
    }

    #[test]
    fn test_read_relocations_asm_data_o() {
        // Manually check with command `readelf -r asm/ARCH/data.o`

        for arch in IMPLEMENTED_ARCHS {
            let binary = get_example_file_binary(SourceType::Assembly, arch, "data.o");
            let elf = read_file(&binary).unwrap();
            let sections = read_section_headers(elf, &binary).unwrap();
            let symbols = read_symbols(elf, &binary).unwrap();
            let mut relocation_sections = read_relocation_sections(elf, &binary).unwrap();

            // Check relocation section `.rela.text`
            let relocation_section_opt = relocation_sections
                .iter_mut()
                .find(|s| s.name == ".rela.text");

            assert!(relocation_section_opt.is_some());

            let relocation_section = relocation_section_opt.unwrap();
            assert_eq!(
                sections[relocation_section.target_section_index].name,
                ".text"
            );

            // Check relocation entries in `.rela.text` section
            let relocations = &mut relocation_section.relocations;

            // Sort the relocations by placeholder_offset to ensure consistent order for testing.
            relocations.sort_by_key(|a| a.placeholder_offset);

            // Check certain entries for testing purposes.
            match arch {
                Machine::X86_64 => {
                    // The assembler generates relocations with offset that refers to the section symbols, not the actual symbols.
                    let relocation0 = &relocations[0];
                    let symbol0 = &symbols[relocation0.symbol_index];
                    assert_eq!(relocation0.relocation_type, RelocationType::R_X86_64_PC32);
                    assert!(
                        matches!(symbol0, Symbol::Defined { section_index, ..} if sections[*section_index].name == ".rodata")
                    );

                    let relocation1 = &relocations[1];
                    let symbol1 = &symbols[relocation1.symbol_index];
                    assert_eq!(relocation1.relocation_type, RelocationType::R_X86_64_PC32);
                    assert!(
                        matches!(symbol1, Symbol::Defined { section_index, ..} if sections[*section_index].name == ".data")
                    );
                }
                Machine::AArch64 => {
                    // The assembler generates relocations with offset that refers to the section symbols, not the actual symbols.

                    // Case 1: R_AARCH64_ADR_PREL_PG_HI21 + R_AARCH64_ADD_ABS_LO12_NC (GCC favorite)

                    let relocation0 = &relocations[0];
                    let symbol0 = &symbols[relocation0.symbol_index];
                    assert_eq!(
                        relocation0.relocation_type,
                        RelocationType::R_AARCH64_ADR_PREL_PG_HI21
                    );
                    assert!(
                        matches!(symbol0, Symbol::Defined { section_index, ..} if sections[*section_index].name == ".rodata")
                    );

                    let relocation1 = &relocations[1];
                    let symbol1 = &symbols[relocation1.symbol_index];
                    assert_eq!(
                        relocation1.relocation_type,
                        RelocationType::R_AARCH64_ADD_ABS_LO12_NC
                    );
                    assert!(
                        matches!(symbol1, Symbol::Defined { section_index, ..} if sections[*section_index].name == ".rodata")
                    );

                    // Case 2: R_AARCH64_ADR_PREL_PG_HI21 + R_AARCH64_LDST64_ABS_LO12_NC

                    let relocation4 = &relocations[4];
                    let symbol4 = &symbols[relocation4.symbol_index];
                    assert_eq!(
                        relocation4.relocation_type,
                        RelocationType::R_AARCH64_ADR_PREL_PG_HI21
                    );
                    assert!(
                        matches!(symbol4, Symbol::Defined { section_index, ..} if sections[*section_index].name == ".rodata")
                    );

                    let relocation5 = &relocations[5];
                    let symbol5 = &symbols[relocation5.symbol_index];
                    assert_eq!(
                        relocation5.relocation_type,
                        RelocationType::R_AARCH64_LDST64_ABS_LO12_NC
                    );
                    assert!(
                        matches!(symbol5, Symbol::Defined { section_index, ..} if sections[*section_index].name == ".rodata")
                    );
                }
                Machine::RiscV => {
                    // Relocation for symbol `__global_pointer$`: R_RISCV_PCREL_HI20 + R_RISCV_PCREL_LO12_I

                    let relocation0 = &relocations[0];
                    let symbol0 = &symbols[relocation0.symbol_index];
                    assert_eq!(
                        relocation0.relocation_type,
                        RelocationType::R_RISCV_PCREL_HI20
                    );
                    assert!(
                        matches!(symbol0, Symbol::External(name) if name == "__global_pointer$")
                    );

                    let relocation1 = &relocations[1];
                    let symbol1 = &symbols[relocation1.symbol_index];
                    assert_eq!(
                        relocation1.relocation_type,
                        RelocationType::R_RISCV_PCREL_LO12_I
                    );
                    assert!(matches!(symbol1, Symbol::Defined { name, ..} if name == ".L0 ")); // Note that this symbol name has a trailing space

                    // Relocation for symbol `foo`: R_RISCV_PCREL_HI20 + R_RISCV_PCREL_LO12_I

                    let relocation2 = &relocations[2];
                    let symbol2 = &symbols[relocation2.symbol_index];
                    assert_eq!(
                        relocation2.relocation_type,
                        RelocationType::R_RISCV_PCREL_HI20
                    );
                    assert!(matches!(symbol2, Symbol::Defined { name, ..} if name == "foo"));

                    let relocation3 = &relocations[3];
                    let symbol3 = &symbols[relocation3.symbol_index];
                    assert_eq!(
                        relocation3.relocation_type,
                        RelocationType::R_RISCV_PCREL_LO12_I
                    );
                    assert!(matches!(symbol3, Symbol::Defined { name, ..} if name == ".L0 ")); // Note that this symbol name has a trailing space
                }
                Machine::LoongArch => {
                    // Relocation for symbol `foo`: R_LARCH_PCALA_HI20 + R_LARCH_PCALA_LO12

                    let relocation0 = &relocations[0];
                    let symbol0 = &symbols[relocation0.symbol_index];
                    assert_eq!(
                        relocation0.relocation_type,
                        RelocationType::R_LARCH_PCALA_HI20
                    );
                    assert!(matches!(symbol0, Symbol::Defined { name, ..} if name == "foo"));

                    let relocation1 = &relocations[1];
                    let symbol1 = &symbols[relocation1.symbol_index];
                    assert_eq!(
                        relocation1.relocation_type,
                        RelocationType::R_LARCH_PCALA_LO12
                    );
                    assert!(matches!(symbol1, Symbol::Defined { name, ..} if name == "foo"));
                }
                Machine::PowerPC64 => {
                    // todo
                }
                Machine::S390 => {
                    // todo
                }
                Machine::Other(_) => unimplemented!(),
            }
        }
    }

    #[test]
    fn test_read_relocations_asm_relocate_within_data_o() {
        // Manually check with command `readelf -r asm/ARCH/relocate-within-data.o`

        for arch in IMPLEMENTED_ARCHS {
            let binary =
                get_example_file_binary(SourceType::Assembly, arch, "relocate-within-data.o");
            let elf = read_file(&binary).unwrap();
            let sections = read_section_headers(elf, &binary).unwrap();
            let symbols = read_symbols(elf, &binary).unwrap();
            let mut relocation_sections = read_relocation_sections(elf, &binary).unwrap();

            // Check relocation section `.rela.text`
            {
                let relocation_section_opt = relocation_sections
                    .iter_mut()
                    .find(|s| s.name == ".rela.text");

                assert!(relocation_section_opt.is_some());

                let relocation_section = relocation_section_opt.unwrap();
                assert_eq!(
                    sections[relocation_section.target_section_index].name,
                    ".text"
                );

                // Check relocation entries in `.rela.text` section
                let relocations = &mut relocation_section.relocations;

                // Sort the relocations by placeholder_offset to ensure consistent order for testing.
                relocations.sort_by_key(|a| a.placeholder_offset);

                // Check certain entries for testing purposes.
                match arch {
                    Machine::X86_64 => {
                        // The assembler generates relocations with offset that refers to the section symbols, not the actual symbols.
                        let relocation0 = &relocations[0];
                        let symbol0 = &symbols[relocation0.symbol_index];
                        assert_eq!(relocation0.relocation_type, RelocationType::R_X86_64_PC32);
                        assert!(
                            matches!(symbol0, Symbol::Defined { section_index, ..} if sections[*section_index].name == ".rodata")
                        );

                        let relocation1 = &relocations[1];
                        let symbol1 = &symbols[relocation1.symbol_index];
                        assert_eq!(relocation1.relocation_type, RelocationType::R_X86_64_PC32);
                        assert!(
                            matches!(symbol1, Symbol::Defined { section_index, ..} if sections[*section_index].name == ".data")
                        );
                    }
                    Machine::AArch64 => {
                        // The assembler generates relocations with offset that refers to the section symbols, not the actual symbols.
                        let relocation0 = &relocations[0];
                        let symbol0 = &symbols[relocation0.symbol_index];
                        assert_eq!(
                            relocation0.relocation_type,
                            RelocationType::R_AARCH64_ADR_PREL_PG_HI21
                        );
                        assert!(
                            matches!(symbol0, Symbol::Defined { section_index, ..} if sections[*section_index].name == ".rodata")
                        );

                        let relocation1 = &relocations[1];
                        let symbol1 = &symbols[relocation1.symbol_index];
                        assert_eq!(
                            relocation1.relocation_type,
                            RelocationType::R_AARCH64_LDST64_ABS_LO12_NC
                        );
                        assert!(
                            matches!(symbol1, Symbol::Defined { section_index, ..} if sections[*section_index].name == ".rodata")
                        );
                    }
                    Machine::RiscV => {
                        // Relocation for symbol `__global_pointer$`: R_RISCV_PCREL_HI20 + R_RISCV_PCREL_LO12_I

                        let relocation0 = &relocations[0];
                        let symbol0 = &symbols[relocation0.symbol_index];
                        assert_eq!(
                            relocation0.relocation_type,
                            RelocationType::R_RISCV_PCREL_HI20
                        );
                        assert!(
                            matches!(symbol0, Symbol::External(name) if name == "__global_pointer$")
                        );

                        let relocation1 = &relocations[1];
                        let symbol1 = &symbols[relocation1.symbol_index];
                        assert_eq!(
                            relocation1.relocation_type,
                            RelocationType::R_RISCV_PCREL_LO12_I
                        );
                        assert!(matches!(symbol1, Symbol::Defined { name, ..} if name == ".L0 ")); // Note that this symbol name has a trailing space

                        // Relocation for symbol `pfoo`: R_RISCV_PCREL_HI20 + R_RISCV_PCREL_LO12_I

                        let relocation2 = &relocations[2];
                        let symbol2 = &symbols[relocation2.symbol_index];
                        assert_eq!(
                            relocation2.relocation_type,
                            RelocationType::R_RISCV_PCREL_HI20
                        );
                        assert!(matches!(symbol2, Symbol::Defined { name, ..} if name == "pfoo"));

                        let relocation3 = &relocations[3];
                        let symbol3 = &symbols[relocation3.symbol_index];
                        assert_eq!(
                            relocation3.relocation_type,
                            RelocationType::R_RISCV_PCREL_LO12_I
                        );
                        assert!(matches!(symbol3, Symbol::Defined { name, ..} if name == ".L0 ")); // Note that this symbol name has a trailing space
                    }
                    Machine::LoongArch => {
                        // Relocation for symbol `pfoo`: R_LARCH_PCALA_HI20 + R_LARCH_PCALA_LO12

                        let relocation0 = &relocations[0];
                        let symbol0 = &symbols[relocation0.symbol_index];
                        assert_eq!(
                            relocation0.relocation_type,
                            RelocationType::R_LARCH_PCALA_HI20
                        );
                        assert!(matches!(symbol0, Symbol::Defined { name, ..} if name == "pfoo"));

                        let relocation1 = &relocations[1];
                        let symbol1 = &symbols[relocation1.symbol_index];
                        assert_eq!(
                            relocation1.relocation_type,
                            RelocationType::R_LARCH_PCALA_LO12
                        );
                        assert!(matches!(symbol1, Symbol::Defined { name, ..} if name == "pfoo"));
                    }
                    Machine::PowerPC64 => {
                        // todo
                    }
                    Machine::S390 => {
                        // todo
                    }
                    Machine::Other(_) => unimplemented!(),
                }
            }

            // Check relocation section `.rela.data`
            {
                let relocation_section_opt = relocation_sections
                    .iter_mut()
                    .find(|s| s.name == ".rela.data");

                assert!(relocation_section_opt.is_some());

                let relocation_section = relocation_section_opt.unwrap();
                assert_eq!(
                    sections[relocation_section.target_section_index].name,
                    ".data"
                );

                // Check relocation entries in `.rela.data` section
                let relocations = &mut relocation_section.relocations;

                // Sort the relocations by placeholder_offset to ensure consistent order for testing.
                relocations.sort_by_key(|a| a.placeholder_offset);

                // Check certain entries for testing purposes.
                match arch {
                    Machine::X86_64 => {
                        let relocation0 = &relocations[0];
                        let symbol0 = &symbols[relocation0.symbol_index];
                        assert_eq!(relocation0.relocation_type, RelocationType::R_X86_64_64);
                        assert!(matches!(symbol0, Symbol::Defined { name, ..} if name == "dec"));

                        let relocation1 = &relocations[1];
                        let symbol1 = &symbols[relocation1.symbol_index];
                        assert_eq!(relocation1.relocation_type, RelocationType::R_X86_64_64);
                        assert!(matches!(symbol1, Symbol::Defined { name, ..} if name == "inc"));
                    }
                    Machine::AArch64 => {
                        let relocation0 = &relocations[0];
                        let symbol0 = &symbols[relocation0.symbol_index];
                        assert_eq!(relocation0.relocation_type, RelocationType::R_AARCH64_ABS64);
                        assert!(matches!(symbol0, Symbol::Defined { name, ..} if name == "dec"));

                        let relocation1 = &relocations[1];
                        let symbol1 = &symbols[relocation1.symbol_index];
                        assert_eq!(relocation1.relocation_type, RelocationType::R_AARCH64_ABS64);
                        assert!(matches!(symbol1, Symbol::Defined { name, ..} if name == "inc"));
                    }
                    Machine::RiscV => {
                        let relocation0 = &relocations[0];
                        let symbol0 = &symbols[relocation0.symbol_index];
                        assert_eq!(relocation0.relocation_type, RelocationType::R_RISCV_64);
                        assert!(matches!(symbol0, Symbol::Defined{name, ..} if name == "dec"));

                        let relocation1 = &relocations[1];
                        let symbol1 = &symbols[relocation1.symbol_index];
                        assert_eq!(relocation1.relocation_type, RelocationType::R_RISCV_64);
                        assert!(matches!(symbol1, Symbol::Defined{name, ..} if name == "inc"));
                    }
                    Machine::LoongArch => {
                        let relocation0 = &relocations[0];
                        let symbol0 = &symbols[relocation0.symbol_index];
                        assert_eq!(relocation0.relocation_type, RelocationType::R_LARCH_64);
                        assert!(matches!(symbol0, Symbol::Defined{name, ..} if name == "dec"));

                        let relocation1 = &relocations[1];
                        let symbol1 = &symbols[relocation1.symbol_index];
                        assert_eq!(relocation1.relocation_type, RelocationType::R_LARCH_64);
                        assert!(matches!(symbol1, Symbol::Defined{name, ..} if name == "inc"));
                    }
                    Machine::PowerPC64 => {
                        // todo
                    }
                    Machine::S390 => {
                        // todo
                    }
                    Machine::Other(_) => unimplemented!(),
                }
            }

            // Check relocation section `.rela.rodata`
            {
                let relocation_section_opt = relocation_sections
                    .iter_mut()
                    .find(|s| s.name == ".rela.rodata");

                assert!(relocation_section_opt.is_some());

                let relocation_section = relocation_section_opt.unwrap();
                assert_eq!(
                    sections[relocation_section.target_section_index].name,
                    ".rodata"
                );

                // Check relocation entries in `.rela.rodata` section
                let relocations = &mut relocation_section.relocations;

                // Sort the relocations by placeholder_offset to ensure consistent order for testing.
                relocations.sort_by_key(|a| a.placeholder_offset);

                // Check certain entries for testing purposes.
                match arch {
                    Machine::X86_64 => {
                        let relocation0 = &relocations[0];
                        let symbol0 = &symbols[relocation0.symbol_index];
                        assert_eq!(relocation0.relocation_type, RelocationType::R_X86_64_64);
                        assert!(matches!(symbol0, Symbol::Defined { name, ..} if name == "foo"));

                        let relocation1 = &relocations[1];
                        let symbol1 = &symbols[relocation1.symbol_index];
                        assert_eq!(relocation1.relocation_type, RelocationType::R_X86_64_64);
                        assert!(matches!(symbol1, Symbol::Defined { name, ..} if name == "bar"));
                    }

                    Machine::AArch64 => {
                        // The assembler generates relocations with offset that refers to the section symbols, not the actual symbols.
                        let relocation0 = &relocations[0];
                        let symbol0 = &symbols[relocation0.symbol_index];
                        assert_eq!(relocation0.relocation_type, RelocationType::R_AARCH64_ABS64);
                        assert!(
                            matches!(symbol0, Symbol::Defined { section_index, ..} if sections[*section_index].name == ".data")
                        );

                        let relocation1 = &relocations[1];
                        let symbol1 = &symbols[relocation1.symbol_index];
                        assert_eq!(relocation1.relocation_type, RelocationType::R_AARCH64_ABS64);
                        assert!(
                            matches!(symbol1, Symbol::Defined { section_index, ..} if sections[*section_index].name == ".data")
                        );
                    }
                    Machine::RiscV => {
                        let relocation0 = &relocations[0];
                        let symbol0 = &symbols[relocation0.symbol_index];
                        assert_eq!(relocation0.relocation_type, RelocationType::R_RISCV_64);
                        assert!(matches!(symbol0, Symbol::Defined{name, ..} if name == "foo"));

                        let relocation1 = &relocations[1];
                        let symbol1 = &symbols[relocation1.symbol_index];
                        assert_eq!(relocation1.relocation_type, RelocationType::R_RISCV_64);
                        assert!(matches!(symbol1, Symbol::Defined{name, ..} if name == "bar"));
                    }
                    Machine::LoongArch => {
                        let relocation0 = &relocations[0];
                        let symbol0 = &symbols[relocation0.symbol_index];
                        assert_eq!(relocation0.relocation_type, RelocationType::R_LARCH_64);
                        assert!(matches!(symbol0, Symbol::Defined{name, ..} if name == "foo"));

                        let relocation1 = &relocations[1];
                        let symbol1 = &symbols[relocation1.symbol_index];
                        assert_eq!(relocation1.relocation_type, RelocationType::R_LARCH_64);
                        assert!(matches!(symbol1, Symbol::Defined{name, ..} if name == "bar"));
                    }
                    Machine::PowerPC64 => {
                        // todo
                    }
                    Machine::S390 => {
                        // todo
                    }
                    Machine::Other(_) => unimplemented!(),
                }
            }
        }
    }

    #[test]
    fn test_read_program_headers_asm_minimal_o() {
        // Manually check with command `readelf -l asm/ARCH/minimal.o`

        for arch in IMPLEMENTED_ARCHS {
            let binary = get_example_file_binary(SourceType::Assembly, arch, "minimal.o");
            let elf = read_file(&binary).unwrap();
            let program_headers = read_program_headers(elf, &binary).unwrap();

            assert!(program_headers.is_empty());
        }
    }

    #[test]
    fn test_read_program_headers_asm_minimal_elf() {
        // Manually check with command `readelf -l asm/ARCH/minimal.elf`

        for arch in IMPLEMENTED_ARCHS {
            let binary = get_example_file_binary(SourceType::Assembly, arch, "minimal.elf");
            let elf = read_file(&binary).unwrap();
            let program_headers = read_program_headers(elf, &binary).unwrap();

            match arch {
                Machine::X86_64 => {
                    // Segment offset, virtual address, file size, memory size and alignment
                    // may vary across different versions of the assembler and platforms.
                    // Check segment types and flags, but not check the other fields.

                    // segment that covers file header and program headers
                    assert_eq!(program_headers[0].segment_type, SegmentType::Load);
                    assert_eq!(program_headers[0].segment_flags, vec![SegmentFlag::Read]);

                    // segment that covers .text
                    assert_eq!(program_headers[1].segment_type, SegmentType::Load);
                    assert_eq!(
                        program_headers[1].segment_flags,
                        vec![SegmentFlag::Execute, SegmentFlag::Read]
                    );
                }
                Machine::AArch64 => {
                    // segment that covers .text
                    assert_eq!(program_headers[0].segment_type, SegmentType::Load);
                    assert_eq!(
                        program_headers[0].segment_flags,
                        vec![SegmentFlag::Execute, SegmentFlag::Read]
                    );
                }
                Machine::RiscV => {
                    // segment 0 is RISCV_ATTRIBUTE

                    // segment that covers file header, program headers and .text
                    assert_eq!(program_headers[1].segment_type, SegmentType::Load);
                    assert_eq!(
                        program_headers[1].segment_flags,
                        vec![SegmentFlag::Execute, SegmentFlag::Read]
                    );
                }
                Machine::LoongArch => {
                    assert_eq!(program_headers[0].segment_type, SegmentType::Load);
                    assert_eq!(
                        program_headers[0].segment_flags,
                        vec![SegmentFlag::Execute, SegmentFlag::Read]
                    );
                }
                Machine::PowerPC64 => {
                    // todo
                }
                Machine::S390 => {
                    // todo
                }
                Machine::Other(_) => unimplemented!(),
            }
        }
    }

    #[test]
    fn test_read_program_headers_asm_data_elf() {
        // Manually check with command `readelf -l asm/ARCH/data.elf`

        for arch in IMPLEMENTED_ARCHS {
            let binary = get_example_file_binary(SourceType::Assembly, arch, "data.elf");
            let elf = read_file(&binary).unwrap();
            let program_headers = read_program_headers(elf, &binary).unwrap();

            match arch {
                Machine::X86_64 => {
                    // Segment offset, virtual address, file size, memory size and alignment
                    // may vary across different versions of the assembler and platforms.
                    // Check segment types and flags, but not check the other fields.

                    // segment that covers file header and program headers
                    assert_eq!(program_headers[0].segment_type, SegmentType::Load);
                    assert_eq!(program_headers[0].segment_flags, vec![SegmentFlag::Read]);

                    // segment that covers .text
                    assert_eq!(program_headers[1].segment_type, SegmentType::Load);
                    assert_eq!(
                        program_headers[1].segment_flags,
                        vec![SegmentFlag::Execute, SegmentFlag::Read]
                    );

                    // segment that covers .rodata
                    assert_eq!(program_headers[2].segment_type, SegmentType::Load);
                    assert_eq!(program_headers[2].segment_flags, vec![SegmentFlag::Read]);

                    // segment that covers .data and .bss
                    assert_eq!(program_headers[3].segment_type, SegmentType::Load);
                    assert_eq!(
                        program_headers[3].segment_flags,
                        vec![SegmentFlag::Write, SegmentFlag::Read]
                    );
                }
                Machine::AArch64 => {
                    // segment that covers .text and .rodata
                    assert_eq!(program_headers[0].segment_type, SegmentType::Load);
                    assert_eq!(
                        program_headers[0].segment_flags,
                        vec![SegmentFlag::Execute, SegmentFlag::Read]
                    );

                    // segment that covers .data and .bss
                    assert_eq!(program_headers[1].segment_type, SegmentType::Load);
                    assert_eq!(
                        program_headers[1].segment_flags,
                        vec![SegmentFlag::Write, SegmentFlag::Read]
                    );
                }
                Machine::RiscV => {
                    // Segment 0 is RISCV_ATTRIBUTE

                    // segment that covers .text and .rodata
                    assert_eq!(program_headers[1].segment_type, SegmentType::Load);
                    assert_eq!(
                        program_headers[1].segment_flags,
                        vec![SegmentFlag::Execute, SegmentFlag::Read]
                    );

                    // segment that covers .data and .bss
                    assert_eq!(program_headers[2].segment_type, SegmentType::Load);
                    assert_eq!(
                        program_headers[2].segment_flags,
                        vec![SegmentFlag::Write, SegmentFlag::Read]
                    );
                }
                Machine::LoongArch => {
                    assert_eq!(program_headers[0].segment_type, SegmentType::Load);
                    assert_eq!(
                        program_headers[0].segment_flags,
                        vec![SegmentFlag::Execute, SegmentFlag::Read]
                    );
                    assert_eq!(program_headers[1].segment_type, SegmentType::Load);
                    assert_eq!(
                        program_headers[1].segment_flags,
                        vec![SegmentFlag::Write, SegmentFlag::Read]
                    );
                }
                Machine::PowerPC64 => {
                    // todo
                }
                Machine::S390 => {
                    // todo
                }
                Machine::Other(_) => unimplemented!(),
            }
        }
    }

    // ===============================================
    // C programs compiled with `gcc` for testing purposes
    // ===============================================

    #[test]
    fn test_read_file_header_gcc_minimal_o() {
        // Manually check with command `readelf -h gcc/ARCH/minimal.o`

        for arch in IMPLEMENTED_ARCHS {
            let binary = get_example_file_binary(SourceType::GCC, arch, "minimal.o");
            let elf = read_file(&binary).unwrap();
            let file_header = read_file_header(elf).unwrap();

            assert_eq!(file_header.file_type, FileType::Relocatable);
            assert_eq!(file_header.os_abi, OSABI::SystemV);
            assert_eq!(file_header.file_class, FileClass::Elf64);

            match arch {
                Machine::X86_64 => {
                    assert_eq!(file_header.machine, Machine::X86_64);
                    assert_eq!(file_header.data_encoding, DataEncoding::LittleEndian);
                }
                Machine::AArch64 => {
                    assert_eq!(file_header.machine, Machine::AArch64);
                    assert_eq!(file_header.data_encoding, DataEncoding::LittleEndian);
                }
                Machine::RiscV => {
                    assert_eq!(file_header.machine, Machine::RiscV);
                    assert_eq!(file_header.data_encoding, DataEncoding::LittleEndian);
                }
                Machine::LoongArch => {
                    assert_eq!(file_header.machine, Machine::LoongArch);
                    assert_eq!(file_header.data_encoding, DataEncoding::LittleEndian);
                }
                Machine::PowerPC64 => {
                    assert_eq!(file_header.machine, Machine::PowerPC64);
                    assert_eq!(file_header.data_encoding, DataEncoding::LittleEndian);
                }
                Machine::S390 => {
                    assert_eq!(file_header.machine, Machine::S390);
                    assert_eq!(file_header.data_encoding, DataEncoding::BigEndian);
                }
                Machine::Other(_) => unimplemented!(),
            }
        }
    }

    #[test]
    fn test_read_file_header_gcc_minimal_elf() {
        // Manually check with command `readelf -h gcc/ARCH/minimal.elf`

        for arch in IMPLEMENTED_ARCHS {
            let binary = get_example_file_binary(SourceType::GCC, arch, "minimal.elf");
            let elf = read_file(&binary).unwrap();
            let file_header = read_file_header(elf).unwrap();

            assert_eq!(file_header.file_type, FileType::Executable);
        }
    }

    #[test]
    fn test_read_section_header_gcc_minimal_o() {
        // Fields such as `size`, `binary`, `align`, and `offset` are not
        // intended to be tested here because they are not guaranteed
        // to be the same across different versions of the assembler and platforms.

        for arch in IMPLEMENTED_ARCHS {
            // Manually check with command `readelf -S gcc/ARCH/minimal.o`
            let binary = get_example_file_binary(SourceType::GCC, arch, "minimal.o");
            let elf = read_file(&binary).unwrap();
            let sections = read_section_headers(elf, &binary).unwrap();

            // Check section names
            assert_contains_all(
                &sections.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
                &[".text", ".data", ".bss", ".symtab", ".strtab", ".shstrtab"],
            );

            // Check section types

            // The first section header is always a NULL section header, which is reserved and has no name.
            assert!(matches!(sections.first(), Some(s) if s.section_type == SectionType::Null));

            assert!(matches!(
                sections
                    .iter()
                    .find(|s| s.name == ".text"),
                Some(s) if s.section_type == SectionType::Progbits
            ));

            assert!(matches!(
                sections
                    .iter()
                    .find(|s| s.name == ".data"),
                Some(s) if s.section_type == SectionType::Progbits
            ));

            assert!(matches!(
                sections
                    .iter()
                    .find(|s| s.name == ".bss"),
                Some(s) if s.section_type == SectionType::Nobits
            ));

            assert!(matches!(
                sections
                    .iter()
                    .find(|s| s.name == ".symtab"),
                Some(s) if s.section_type == SectionType::Symtab
            ));

            assert!(matches!(
                sections
                    .iter()
                    .find(|s| s.name == ".strtab"),
                Some(s) if s.section_type == SectionType::Strtab
            ));

            assert!(matches!(
                sections
                    .iter()
                    .find(|s| s.name == ".shstrtab"),
                Some(s) if s.section_type == SectionType::Strtab
            ));
        }
    }

    #[test]
    fn test_read_section_header_gcc_function_o() {
        // Manually check with command `readelf -S gcc/ARCH/function.o`
        for arch in IMPLEMENTED_ARCHS {
            let binary = get_example_file_binary(SourceType::GCC, arch, "function.o");
            let elf = read_file(&binary).unwrap();
            let sections = read_section_headers(elf, &binary).unwrap();

            // Check section names
            assert_contains_all(
                &sections.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
                &[
                    ".text",
                    ".rela.text",
                    ".data",
                    ".bss",
                    ".rodata",
                    ".symtab",
                    ".strtab",
                    ".shstrtab",
                ],
            );

            // Check section types
            assert!(matches!(
                sections
                    .iter()
                    .find(|s| s.name == ".text"),
                Some(s) if s.section_type == SectionType::Progbits
            ));

            assert!(matches!(
                sections
                    .iter()
                    .find(|s| s.name == ".bss"),
                Some(s) if s.section_type == SectionType::Nobits
            ));

            assert!(matches!(
                sections
                    .iter()
                    .find(|s| s.name == ".rela.text"),
                Some(s) if s.section_type == SectionType::Rela
            ));
        }
    }

    #[test]
    fn test_read_symbols_gcc_minimal_o() {
        // Manually check with command `readelf -s gcc/ARCH/minimal.o`
        for arch in IMPLEMENTED_ARCHS {
            let binary = get_example_file_binary(SourceType::GCC, arch, "minimal.o");
            let elf = read_file(&binary).unwrap();
            let symbols = read_symbols(elf, &binary).unwrap();

            // The first symbol table entry (index 0) is reserved and must be undefined.
            assert_eq!(symbols[0], Symbol::Other);

            // GCC generates correct symbol type for function symbols,
            // while the assembler generates `Notype` for function symbols.
            assert!(matches!(
                symbols
                    .iter()
                    .find(|s| matches!(s, Symbol::Defined {name,..} if name == "_start")),
                Some(Symbol::Defined {
                    bind: SymbolBind::Global,
                    symbol_type: SymbolType::Func,
                    ..
                })
            ));
        }
    }

    #[test]
    fn test_read_symbols_gcc_function_o() {
        for arch in IMPLEMENTED_ARCHS {
            // Manually check with command `readelf -s gcc/ARCH/function.o`
            let binary = get_example_file_binary(SourceType::GCC, arch, "function.o");
            let elf = read_file(&binary).unwrap();
            let symbols = read_symbols(elf, &binary).unwrap();

            // Check symbol names
            assert_contains_all(
                &symbols
                    .iter()
                    .filter(|s| matches!(s, Symbol::Defined { .. }))
                    .map(|s| {
                        if let Symbol::Defined { name, .. } = s {
                            name.as_str()
                        } else {
                            unreachable!()
                        }
                    })
                    .collect::<Vec<_>>(),
                &["print_hello", "print_world"],
            );

            // Check symbols names, types and binds, but not check the section index and offset,
            // because they may vary across different platforms and versions of the assembler.

            // Check certain entries for testing purposes.

            // GCC generates correct symbol type for data and function symbols,
            // while the assembler generates `Notype` for data and function symbols.
            assert!(matches!(
                symbols
                    .iter()
                    .find(|s| matches!(s, Symbol::Defined { name, .. } if name == "print_hello")),
                Some(Symbol::Defined {
                    bind: SymbolBind::Global, // C function symbols are global by default
                    symbol_type: SymbolType::Func,
                    ..
                })
            ));

            assert!(matches!(
                symbols
                    .iter()
                    .find(|s| matches!(s, Symbol::Defined { name, .. } if name == "print_world")),
                Some(Symbol::Defined {
                    bind: SymbolBind::Global, // C function symbols are global by default
                    symbol_type: SymbolType::Func,
                    ..
                })
            ));
        }
    }

    #[test]
    fn test_read_symbols_gcc_data_o() {
        for arch in IMPLEMENTED_ARCHS {
            // Manually check with command `readelf -s gcc/ARCH/data.o`
            let binary = get_example_file_binary(SourceType::GCC, arch, "data.o");
            let elf = read_file(&binary).unwrap();
            let symbols = read_symbols(elf, &binary).unwrap();

            // Check symbol names
            assert_contains_all(
                &symbols
                    .iter()
                    .filter(|s| matches!(s, Symbol::Defined { .. }))
                    .map(|s| {
                        if let Symbol::Defined { name, .. } = s {
                            name.as_str()
                        } else {
                            unreachable!()
                        }
                    })
                    .collect::<Vec<_>>(),
                &["foo", "bar", "a", "b", "x", "y", "_start"],
            );

            // Check symbols names, types and binds, but not check the section index and offset,
            // because they may vary across different platforms and versions of the assembler.

            // Check certain entries for testing purposes.

            // GCC generates correct symbol type for data and function symbols,
            // while the assembler generates `Notype` for data and function symbols.
            assert!(matches!(
                symbols
                    .iter()
                    .find(|s| matches!(s, Symbol::Defined { name, .. } if name == "foo")),
                Some(Symbol::Defined {
                    bind: SymbolBind::Local,
                    symbol_type: SymbolType::Object,
                    ..
                })
            ));

            assert!(matches!(
                symbols
                    .iter()
                    .find(|s| matches!(s, Symbol::Defined { name, .. } if name == "bar")),
                Some(Symbol::Defined {
                    bind: SymbolBind::Local,
                    symbol_type: SymbolType::Object,
                    ..
                })
            ));
        }
    }

    #[test]
    fn test_read_symbols_gcc_symbol_export_o() {
        for arch in IMPLEMENTED_ARCHS {
            // Manually check with command `readelf -s gcc/ARCH/symbol-export.o`
            let binary = get_example_file_binary(SourceType::GCC, arch, "symbol-export.o");
            let elf = read_file(&binary).unwrap();
            let symbols = read_symbols(elf, &binary).unwrap();

            // Check symbol names
            assert_contains_all(
                &symbols
                    .iter()
                    .filter(|s| matches!(s, Symbol::Defined { .. }))
                    .map(|s| {
                        if let Symbol::Defined { name, .. } = s {
                            name.as_str()
                        } else {
                            unreachable!()
                        }
                    })
                    .collect::<Vec<_>>(),
                &["foo", "bar", "a", "b", "x", "y", "dec", "inc"],
            );

            // Check symbols names, types and binds, but not check the section index and offset,
            // because they may vary across different platforms and versions of the assembler.

            // Check certain entries for testing purposes.

            // GCC generates correct symbol type for data and function symbols,
            // while the assembler generates `Notype` for data and function symbols.
            assert!(matches!(
                symbols
                    .iter()
                    .find(|s| matches!(s, Symbol::Defined { name, .. } if name == "foo")),
                Some(Symbol::Defined {
                    bind: SymbolBind::Global,
                    symbol_type: SymbolType::Object,
                    ..
                })
            ));

            assert!(matches!(
                symbols
                    .iter()
                    .find(|s| matches!(s, Symbol::Defined { name, .. } if name == "bar")),
                Some(Symbol::Defined {
                    bind: SymbolBind::Global,
                    symbol_type: SymbolType::Object,
                    ..
                })
            ));

            assert!(matches!(
                symbols
                    .iter()
                    .find(|s| matches!(s, Symbol::Defined { name, .. } if name == "dec")),
                Some(Symbol::Defined {
                    bind: SymbolBind::Global,
                    symbol_type: SymbolType::Func,
                    ..
                })
            ));

            assert!(matches!(
                symbols
                    .iter()
                    .find(|s| matches!(s, Symbol::Defined { name, .. } if name == "inc")),
                Some(Symbol::Defined {
                    bind: SymbolBind::Global,
                    symbol_type: SymbolType::Func,
                    ..
                })
            ));
        }
    }

    #[test]
    fn test_read_symbols_gcc_symbol_import_o() {
        // Manually check with command `readelf -s gcc/ARCH/symbol-import.o`

        for arch in IMPLEMENTED_ARCHS {
            let binary = get_example_file_binary(SourceType::Assembly, arch, "symbol-import.o");
            let elf = read_file(&binary).unwrap();
            let symbols = read_symbols(elf, &binary).unwrap();

            // Check imported symbol names
            assert_contains_all(
                &symbols
                    .iter()
                    .filter(|s| matches!(s, Symbol::External(_)))
                    .map(|s| {
                        if let Symbol::External(name) = s {
                            name.as_str()
                        } else {
                            unreachable!()
                        }
                    })
                    .collect::<Vec<_>>(),
                &["foo", "bar", "a", "b", "x", "y", "inc", "dec"],
            );
        }
    }

    #[test]
    fn test_read_symbols_gcc_override_weak_o() {
        // Manually check with command `readelf -s gcc/ARCH/override-weak.o`

        for arch in IMPLEMENTED_ARCHS {
            let binary = get_example_file_binary(SourceType::GCC, arch, "override-weak.o");
            let elf = read_file(&binary).unwrap();
            let symbols = read_symbols(elf, &binary).unwrap();

            // Check weak symbol names
            assert_contains_all(
                &symbols
                    .iter()
                    .filter(
                        |s| matches!(s, Symbol::Defined { bind, .. } if *bind == SymbolBind::Weak),
                    )
                    .map(|s| {
                        if let Symbol::Defined { name, .. } = s {
                            name.as_str()
                        } else {
                            unreachable!()
                        }
                    })
                    .collect::<Vec<_>>(),
                &["foo", "bar"],
            );
        }
    }

    #[test]
    fn test_read_symbols_gcc_override_strong_o() {
        // Manually check with command `readelf -s gcc/ARCH/override-strong.o`

        for arch in IMPLEMENTED_ARCHS {
            let binary = get_example_file_binary(SourceType::GCC, arch, "override-strong.o");
            let elf = read_file(&binary).unwrap();
            let symbols = read_symbols(elf, &binary).unwrap();

            // Check import symbol
            assert!(
                symbols
                    .iter()
                    .find(|s| matches!(s, Symbol::External(name) if name == "foo"))
                    .is_some()
            );

            // Check strong symbol

            // GCC generates correct symbol type for data and function symbols,
            // while the assembler generates `Notype` for data and function symbols.
            assert!(matches!(
                symbols
                    .iter()
                    .find(|s| matches!(s, Symbol::Defined { name, .. } if name == "bar")),
                Some(Symbol::Defined {
                    bind: SymbolBind::Global, // C function symbols are global by default
                    symbol_type: SymbolType::Func,
                    ..
                })
            ));
        }
    }

    #[test]
    fn test_read_relocations_gcc_function_o() {
        // Manually check with command `readelf -r gcc/ARCH/function.o`

        for arch in IMPLEMENTED_ARCHS {
            let binary = get_example_file_binary(SourceType::GCC, arch, "function.o");
            let elf = read_file(&binary).unwrap();
            let sections = read_section_headers(elf, &binary).unwrap();
            let symbols = read_symbols(elf, &binary).unwrap();
            let mut relocation_sections = read_relocation_sections(elf, &binary).unwrap();

            // Check relocation section `.rela.text`
            let relocation_section_opt = relocation_sections
                .iter_mut()
                .find(|s| s.name == ".rela.text");

            assert!(relocation_section_opt.is_some());

            let relocation_section = relocation_section_opt.unwrap();
            assert_eq!(
                sections[relocation_section.target_section_index].name,
                ".text"
            );

            // Check relocation entries in `.rela.text` section
            let relocations = &mut relocation_section.relocations;

            // Sort the relocations by placeholder_offset to ensure consistent order for testing.
            relocations.sort_by_key(|a| a.placeholder_offset);

            // Check certain entries for testing purposes.
            match arch {
                Machine::X86_64 => {
                    let relocation0 = &relocations[0];
                    let symbol0 = &symbols[relocation0.symbol_index];
                    assert_eq!(relocation0.relocation_type, RelocationType::R_X86_64_32);
                    assert!(matches!(symbol0, Symbol::Defined { name, ..} if name == "hello"));

                    let relocation1 = &relocations[1];
                    let symbol1 = &symbols[relocation1.symbol_index];
                    assert_eq!(relocation1.relocation_type, RelocationType::R_X86_64_32);
                    assert!(matches!(symbol1, Symbol::Defined { name, ..} if name == "world"));

                    let relocation2 = &relocations[2];
                    let symbol2 = &symbols[relocation2.symbol_index];
                    assert_eq!(relocation2.relocation_type, RelocationType::R_X86_64_PLT32);
                    assert!(
                        matches!(symbol2, Symbol::Defined { name, ..} if name == "print_hello")
                    );

                    let relocation3 = &relocations[3];
                    let symbol3 = &symbols[relocation3.symbol_index];
                    assert_eq!(relocation3.relocation_type, RelocationType::R_X86_64_PLT32);
                    assert!(
                        matches!(symbol3, Symbol::Defined { name, ..} if name == "print_world")
                    );
                }
                Machine::AArch64 => {
                    // Combination: R_AARCH64_ADR_PREL_PG_HI21 + R_AARCH64_ADD_ABS_LO12_NC

                    let relocation0 = &relocations[0];
                    let symbol0 = &symbols[relocation0.symbol_index];
                    assert_eq!(
                        relocation0.relocation_type,
                        RelocationType::R_AARCH64_ADR_PREL_PG_HI21
                    );
                    assert!(matches!(symbol0, Symbol::Defined { name, ..} if name == "hello"));

                    let relocation1 = &relocations[1];
                    let symbol1 = &symbols[relocation1.symbol_index];
                    assert_eq!(
                        relocation1.relocation_type,
                        RelocationType::R_AARCH64_ADD_ABS_LO12_NC
                    );
                    assert!(matches!(symbol1, Symbol::Defined { name, ..} if name == "hello"));

                    // Relocation for functions `print_hello` and `print_world`: R_AARCH64_CALL26
                    let relocation4 = &relocations[4];
                    let symbol4 = &symbols[relocation4.symbol_index];
                    assert_eq!(
                        relocation4.relocation_type,
                        RelocationType::R_AARCH64_CALL26
                    );
                    assert!(
                        matches!(symbol4, Symbol::Defined { name, ..} if name == "print_hello")
                    );

                    let relocation5 = &relocations[5];
                    let symbol5 = &symbols[relocation5.symbol_index];
                    assert_eq!(
                        relocation5.relocation_type,
                        RelocationType::R_AARCH64_CALL26
                    );
                    assert!(
                        matches!(symbol5, Symbol::Defined { name, ..} if name == "print_world")
                    );
                }
                Machine::RiscV => {
                    // Relocation for symbol `hello`: R_RISCV_PCREL_HI20 + R_RISCV_PCREL_LO12_I

                    let relocation2 = &relocations[2];
                    let symbol2 = &symbols[relocation2.symbol_index];
                    assert_eq!(relocation2.relocation_type, RelocationType::R_RISCV_HI20);
                    assert!(matches!(symbol2, Symbol::Defined { name, ..} if name == "hello"));

                    let relocation3 = &relocations[3];
                    let symbol3 = &symbols[relocation3.symbol_index];
                    assert_eq!(relocation3.relocation_type, RelocationType::R_RISCV_LO12_I);
                    assert!(matches!(symbol3, Symbol::Defined { name, ..} if name == "hello"));

                    // Relocation for functions `print_hello`: R_RISCV_CALL_PLT
                    let relocation8 = &relocations[8];
                    let symbol8 = &symbols[relocation8.symbol_index];
                    assert_eq!(
                        relocation8.relocation_type,
                        RelocationType::R_RISCV_CALL_PLT
                    );
                    assert!(
                        matches!(symbol8, Symbol::Defined { name, ..} if name == "print_hello")
                    );
                }
                Machine::LoongArch => {
                    // Relocation for symbol `hello`: R_LARCH_PCALA_HI2 + R_R_LARCH_PCALA_LO12

                    let relocation2 = &relocations[2];
                    let symbol2 = &symbols[relocation2.symbol_index];
                    assert_eq!(
                        relocation2.relocation_type,
                        RelocationType::R_LARCH_PCALA_HI20
                    );
                    assert!(matches!(symbol2, Symbol::Defined { name, ..} if name == "hello"));

                    let relocation3 = &relocations[3];
                    let symbol3 = &symbols[relocation3.symbol_index];
                    assert_eq!(
                        relocation3.relocation_type,
                        RelocationType::R_LARCH_PCALA_LO12
                    );
                    assert!(matches!(symbol3, Symbol::Defined { name, ..} if name == "hello"));

                    // Relocation for functions `print_hello`: R_LARCH_CALL36
                    let relocation8 = &relocations[8];
                    let symbol8 = &symbols[relocation8.symbol_index];
                    assert_eq!(relocation8.relocation_type, RelocationType::R_LARCH_CALL36);
                    assert!(
                        matches!(symbol8, Symbol::Defined { name, ..} if name == "print_hello")
                    );
                }
                Machine::PowerPC64 => {
                    // todo
                }
                Machine::S390 => {
                    // todo
                }
                Machine::Other(_) => unimplemented!(),
            }
        }
    }

    #[test]
    fn test_read_relocations_gcc_data_o() {
        // Manually check with command `readelf -r gcc/ARCH/data.o`

        for arch in IMPLEMENTED_ARCHS {
            let binary = get_example_file_binary(SourceType::GCC, arch, "data.o");
            let elf = read_file(&binary).unwrap();
            let sections = read_section_headers(elf, &binary).unwrap();
            let symbols = read_symbols(elf, &binary).unwrap();
            let mut relocation_sections = read_relocation_sections(elf, &binary).unwrap();

            // Check relocation section `.rela.text`
            let relocation_section_opt = relocation_sections
                .iter_mut()
                .find(|s| s.name == ".rela.text");

            assert!(relocation_section_opt.is_some());

            let relocation_section = relocation_section_opt.unwrap();
            assert_eq!(
                sections[relocation_section.target_section_index].name,
                ".text"
            );

            // Check relocation entries in `.rela.text` section
            let relocations = &mut relocation_section.relocations;

            // Sort the relocations by placeholder_offset to ensure consistent order for testing.
            relocations.sort_by_key(|a| a.placeholder_offset);

            // Check certain entries for testing purposes.
            match arch {
                Machine::X86_64 => {
                    // The assembler generates relocations with section symbols.
                    let relocation0 = &relocations[0];
                    let symbol0 = &symbols[relocation0.symbol_index];
                    assert_eq!(relocation0.relocation_type, RelocationType::R_X86_64_PC32);
                    assert!(
                        matches!(symbol0, Symbol::Defined { section_index, ..} if sections[*section_index].name == ".data")
                    );

                    let relocation1 = &relocations[1];
                    let symbol1 = &symbols[relocation1.symbol_index];
                    assert_eq!(relocation1.relocation_type, RelocationType::R_X86_64_PC32);
                    assert!(
                        matches!(symbol1, Symbol::Defined { section_index, ..} if sections[*section_index].name == ".data")
                    );
                }
                Machine::AArch64 => {
                    // The assembler generates relocations with section symbols.

                    // Relocation for symbol `foo`: R_AARCH64_ADR_PREL_PG_HI21 + R_AARCH64_ADD_ABS_LO12_NC
                    let relocation0 = &relocations[0];
                    let symbol0 = &symbols[relocation0.symbol_index];
                    assert_eq!(
                        relocation0.relocation_type,
                        RelocationType::R_AARCH64_ADR_PREL_PG_HI21
                    );
                    assert!(
                        matches!(symbol0, Symbol::Defined { section_index, ..} if sections[*section_index].name == ".data")
                    );

                    let relocation1 = &relocations[1];
                    let symbol1 = &symbols[relocation1.symbol_index];
                    assert_eq!(
                        relocation1.relocation_type,
                        RelocationType::R_AARCH64_ADD_ABS_LO12_NC
                    );
                    assert!(
                        matches!(symbol1, Symbol::Defined { section_index, ..} if sections[*section_index].name == ".data")
                    );
                }
                Machine::RiscV => {
                    // Relocation for symbol `a`: R_RISCV_HI20 + R_RISCV_LO12_S

                    let relocation1 = &relocations[1];
                    let symbol1 = &symbols[relocation1.symbol_index];
                    assert_eq!(relocation1.relocation_type, RelocationType::R_RISCV_HI20);
                    assert!(matches!(symbol1, Symbol::Defined { name, ..} if name == "a"));

                    let relocation2 = &relocations[2];
                    let symbol2 = &symbols[relocation2.symbol_index];
                    assert_eq!(relocation2.relocation_type, RelocationType::R_RISCV_LO12_S);
                    assert!(matches!(symbol2, Symbol::Defined { name, ..} if name == "a"));
                }
                Machine::LoongArch => {
                    // Relocation for symbol `a`: R_LARCH_PCALA_HI20 + R_LARCH_PCALA_LO12

                    let relocation1 = &relocations[1];
                    let symbol1 = &symbols[relocation1.symbol_index];
                    assert_eq!(
                        relocation1.relocation_type,
                        RelocationType::R_LARCH_PCALA_HI20
                    );
                    assert!(matches!(symbol1, Symbol::Defined { name, ..} if name == "a"));

                    let relocation2 = &relocations[2];
                    let symbol2 = &symbols[relocation2.symbol_index];
                    assert_eq!(
                        relocation2.relocation_type,
                        RelocationType::R_LARCH_PCALA_LO12
                    );
                    assert!(matches!(symbol2, Symbol::Defined { name, ..} if name == "a"));
                }
                Machine::PowerPC64 => {
                    // todo
                }
                Machine::S390 => {
                    // todo
                }
                Machine::Other(_) => unimplemented!(),
            }
        }
    }

    #[test]
    fn test_read_relocations_gcc_relocate_within_data_o() {
        // Manually check with command `readelf -r gcc/ARCH/relocate-within-data.o`

        for arch in IMPLEMENTED_ARCHS {
            let binary = get_example_file_binary(SourceType::GCC, arch, "relocate-within-data.o");
            let elf = read_file(&binary).unwrap();
            let sections = read_section_headers(elf, &binary).unwrap();
            let symbols = read_symbols(elf, &binary).unwrap();
            let mut relocation_sections = read_relocation_sections(elf, &binary).unwrap();

            // Check relocation section `.rela.text`
            {
                let relocation_section_opt = relocation_sections
                    .iter_mut()
                    .find(|s| s.name == ".rela.text");

                assert!(relocation_section_opt.is_some());

                let relocation_section = relocation_section_opt.unwrap();
                assert_eq!(
                    sections[relocation_section.target_section_index].name,
                    ".text"
                );

                // Check relocation entries in `.rela.text` section
                let relocations = &mut relocation_section.relocations;

                // Sort the relocations by placeholder_offset to ensure consistent order for testing.
                relocations.sort_by_key(|a| a.placeholder_offset);

                // Check certain entries for testing purposes.
                match arch {
                    Machine::X86_64 => {
                        let relocation0 = &relocations[0];
                        let symbol0 = &symbols[relocation0.symbol_index];
                        assert_eq!(relocation0.relocation_type, RelocationType::R_X86_64_32);
                        assert!(matches!(symbol0, Symbol::Defined { name, ..} if name == "foo"));

                        let relocation1 = &relocations[1];
                        let symbol1 = &symbols[relocation1.symbol_index];
                        assert_eq!(relocation1.relocation_type, RelocationType::R_X86_64_PC32);
                        assert!(matches!(symbol1, Symbol::Defined { name, ..} if name == "pdec"));
                    }
                    Machine::AArch64 => {
                        let relocation0 = &relocations[0];
                        let symbol0 = &symbols[relocation0.symbol_index];
                        assert_eq!(
                            relocation0.relocation_type,
                            RelocationType::R_AARCH64_ADR_PREL_PG_HI21
                        );
                        assert!(matches!(symbol0, Symbol::Defined { name, ..} if name == "foo"));

                        let relocation1 = &relocations[1];
                        let symbol1 = &symbols[relocation1.symbol_index];
                        assert_eq!(
                            relocation1.relocation_type,
                            RelocationType::R_AARCH64_ADD_ABS_LO12_NC
                        );
                        assert!(matches!(symbol1, Symbol::Defined { name, ..} if name == "foo"));
                    }
                    Machine::RiscV => {
                        let relocation1 = &relocations[1];
                        let symbol1 = &symbols[relocation1.symbol_index];
                        assert_eq!(relocation1.relocation_type, RelocationType::R_RISCV_HI20);
                        assert!(matches!(symbol1, Symbol::Defined { name, ..} if name == "foo"));

                        let relocation2 = &relocations[2];
                        let symbol2 = &symbols[relocation2.symbol_index];
                        assert_eq!(relocation2.relocation_type, RelocationType::R_RISCV_LO12_I);
                        assert!(matches!(symbol2, Symbol::Defined { name, ..} if name == "foo"));
                    }
                    Machine::LoongArch => {
                        let relocation1 = &relocations[1];
                        let symbol1 = &symbols[relocation1.symbol_index];
                        assert_eq!(
                            relocation1.relocation_type,
                            RelocationType::R_LARCH_PCALA_HI20
                        );
                        assert!(matches!(symbol1, Symbol::Defined { name, ..} if name == "foo"));

                        let relocation2 = &relocations[2];
                        let symbol2 = &symbols[relocation2.symbol_index];
                        assert_eq!(
                            relocation2.relocation_type,
                            RelocationType::R_LARCH_PCALA_LO12
                        );
                        assert!(matches!(symbol2, Symbol::Defined { name, ..} if name == "foo"));
                    }
                    Machine::PowerPC64 => {
                        // todo
                    }
                    Machine::S390 => {
                        // todo
                    }
                    Machine::Other(_) => unimplemented!(),
                }
            }

            // Check relocation section `.rela.data`

            // RISC-V GCC generates section `.sdata` and `rela.sdata` for small data by default,
            // to generate `.data` and `.rela.data` sections, you need to compile with `-msmall-data-limit=0` option.
            {
                let relocation_section_opt = relocation_sections
                    .iter_mut()
                    .find(|s| s.name == ".rela.data");

                assert!(relocation_section_opt.is_some());

                let relocation_section = relocation_section_opt.unwrap();
                assert_eq!(
                    sections[relocation_section.target_section_index].name,
                    ".data"
                );

                // Check relocation entries in `.rela.data` section
                let relocations = &mut relocation_section.relocations;

                // Sort the relocations by placeholder_offset to ensure consistent order for testing.
                relocations.sort_by_key(|a| a.placeholder_offset);

                // Check certain entries for testing purposes.
                match arch {
                    Machine::X86_64 => {
                        let relocation0 = &relocations[0];
                        let symbol0 = &symbols[relocation0.symbol_index];
                        assert_eq!(relocation0.relocation_type, RelocationType::R_X86_64_64);
                        assert!(matches!(symbol0, Symbol::Defined { name, ..} if name == "dec"));

                        let relocation1 = &relocations[1];
                        let symbol1 = &symbols[relocation1.symbol_index];
                        assert_eq!(relocation1.relocation_type, RelocationType::R_X86_64_64);
                        assert!(matches!(symbol1, Symbol::Defined { name, ..} if name == "inc"));
                    }
                    Machine::AArch64 => {
                        let relocation0 = &relocations[0];
                        let symbol0 = &symbols[relocation0.symbol_index];
                        assert_eq!(relocation0.relocation_type, RelocationType::R_AARCH64_ABS64);
                        assert!(matches!(symbol0, Symbol::Defined { name, ..} if name == "dec"));

                        let relocation1 = &relocations[1];
                        let symbol1 = &symbols[relocation1.symbol_index];
                        assert_eq!(relocation1.relocation_type, RelocationType::R_AARCH64_ABS64);
                        assert!(matches!(symbol1, Symbol::Defined { name, ..} if name == "inc"));
                    }
                    Machine::RiscV => {
                        let relocation0 = &relocations[0];
                        let symbol0 = &symbols[relocation0.symbol_index];
                        assert_eq!(relocation0.relocation_type, RelocationType::R_RISCV_64);
                        assert!(matches!(symbol0, Symbol::Defined { name, ..} if name == "dec"));

                        let relocation1 = &relocations[1];
                        let symbol1 = &symbols[relocation1.symbol_index];
                        assert_eq!(relocation1.relocation_type, RelocationType::R_RISCV_64);
                        assert!(matches!(symbol1, Symbol::Defined { name, ..} if name == "inc"));
                    }
                    Machine::LoongArch => {
                        let relocation0 = &relocations[0];
                        let symbol0 = &symbols[relocation0.symbol_index];
                        assert_eq!(relocation0.relocation_type, RelocationType::R_LARCH_64);
                        assert!(matches!(symbol0, Symbol::Defined { name, ..} if name == "dec"));

                        let relocation1 = &relocations[1];
                        let symbol1 = &symbols[relocation1.symbol_index];
                        assert_eq!(relocation1.relocation_type, RelocationType::R_LARCH_64);
                        assert!(matches!(symbol1, Symbol::Defined { name, ..} if name == "inc"));
                    }
                    Machine::PowerPC64 => {
                        // todo
                    }
                    Machine::S390 => {
                        // todo
                    }
                    Machine::Other(_) => unimplemented!(),
                }
            }

            // Check relocation section `.rela.rodata`
            // RISC-V GCC generates section `.srodata` and `rela.srodata` for small read-only data,
            // to generate `.data` and `.rela.data` sections, you need to compile with `-msmall-data-limit=0` option.
            {
                let relocation_section_opt = relocation_sections
                    .iter_mut()
                    .find(|s| s.name == ".rela.rodata");

                assert!(relocation_section_opt.is_some());

                let relocation_section = relocation_section_opt.unwrap();
                assert_eq!(
                    sections[relocation_section.target_section_index].name,
                    ".rodata"
                );

                // Check relocation entries in `.rela.rodata` section
                let relocations = &mut relocation_section.relocations;

                // Sort the relocations by placeholder_offset to ensure consistent order for testing.
                relocations.sort_by_key(|a| a.placeholder_offset);

                // Check certain entries for testing purposes.
                match arch {
                    Machine::X86_64 => {
                        let relocation0 = &relocations[0];
                        let symbol0 = &symbols[relocation0.symbol_index];
                        assert_eq!(relocation0.relocation_type, RelocationType::R_X86_64_64);
                        assert!(matches!(symbol0, Symbol::Defined { name, ..} if name == "foo"));

                        let relocation1 = &relocations[1];
                        let symbol1 = &symbols[relocation1.symbol_index];
                        assert_eq!(relocation1.relocation_type, RelocationType::R_X86_64_64);
                        assert!(matches!(symbol1, Symbol::Defined { name, ..} if name == "bar"));
                    }
                    Machine::AArch64 => {
                        let relocation0 = &relocations[0];
                        let symbol0 = &symbols[relocation0.symbol_index];
                        assert_eq!(relocation0.relocation_type, RelocationType::R_AARCH64_ABS64);
                        assert!(matches!(symbol0, Symbol::Defined { name, ..} if name == "foo"));

                        let relocation1 = &relocations[1];
                        let symbol1 = &symbols[relocation1.symbol_index];
                        assert_eq!(relocation1.relocation_type, RelocationType::R_AARCH64_ABS64);
                        assert!(matches!(symbol1, Symbol::Defined { name, ..} if name == "bar"));
                    }
                    Machine::RiscV => {
                        let relocation0 = &relocations[0];
                        let symbol0 = &symbols[relocation0.symbol_index];
                        assert_eq!(relocation0.relocation_type, RelocationType::R_RISCV_64);
                        assert!(matches!(symbol0, Symbol::Defined{name, ..} if name == "foo"));

                        let relocation1 = &relocations[1];
                        let symbol1 = &symbols[relocation1.symbol_index];
                        assert_eq!(relocation1.relocation_type, RelocationType::R_RISCV_64);
                        assert!(matches!(symbol1, Symbol::Defined{name, ..} if name == "bar"));
                    }
                    Machine::LoongArch => {
                        let relocation0 = &relocations[0];
                        let symbol0 = &symbols[relocation0.symbol_index];
                        assert_eq!(relocation0.relocation_type, RelocationType::R_LARCH_64);
                        assert!(matches!(symbol0, Symbol::Defined{name, ..} if name == "foo"));

                        let relocation1 = &relocations[1];
                        let symbol1 = &symbols[relocation1.symbol_index];
                        assert_eq!(relocation1.relocation_type, RelocationType::R_LARCH_64);
                        assert!(matches!(symbol1, Symbol::Defined{name, ..} if name == "bar"));
                    }
                    Machine::PowerPC64 => {
                        // todo
                    }
                    Machine::S390 => {
                        // todo
                    }
                    Machine::Other(_) => unimplemented!(),
                }
            }
        }
    }

    #[test]
    fn test_read_program_headers_gcc_minimal_o() {
        // Manually check with command `readelf -l gcc/ARCH/minimal.o`

        for arch in IMPLEMENTED_ARCHS {
            let binary = get_example_file_binary(SourceType::GCC, arch, "minimal.o");
            let elf = read_file(&binary).unwrap();
            let program_headers = read_program_headers(elf, &binary).unwrap();

            assert!(program_headers.is_empty());
        }
    }

    #[test]
    fn test_read_program_headers_gcc_minimal_elf() {
        // Manually check with command `readelf -l gcc/ARCH/minimal.elf`

        for arch in IMPLEMENTED_ARCHS {
            let binary = get_example_file_binary(SourceType::GCC, arch, "minimal.elf");
            let elf = read_file(&binary).unwrap();
            let program_headers = read_program_headers(elf, &binary).unwrap();

            match arch {
                Machine::X86_64 => {
                    // Segment offset, virtual address, file size, memory size and alignment
                    // may vary across different versions of the assembler and platforms.
                    // Check segment types and flags, but not check the other fields.

                    // segment that covers file header and program headers
                    assert_eq!(program_headers[0].segment_type, SegmentType::Load);
                    assert_eq!(program_headers[0].segment_flags, vec![SegmentFlag::Read]);

                    // segment that covers .text
                    assert_eq!(program_headers[1].segment_type, SegmentType::Load);
                    assert_eq!(
                        program_headers[1].segment_flags,
                        vec![SegmentFlag::Execute, SegmentFlag::Read]
                    );
                }
                Machine::AArch64 => {
                    // segment that covers .text
                    assert_eq!(program_headers[0].segment_type, SegmentType::Load);
                    assert_eq!(
                        program_headers[0].segment_flags,
                        vec![SegmentFlag::Execute, SegmentFlag::Read]
                    );
                }
                Machine::RiscV => {
                    // segment 0 is RISCV_ATTRIBUTE

                    // segment that covers file header, program headers and .text
                    assert_eq!(program_headers[1].segment_type, SegmentType::Load);
                    assert_eq!(
                        program_headers[1].segment_flags,
                        vec![SegmentFlag::Execute, SegmentFlag::Read]
                    );
                }
                Machine::LoongArch => {
                    assert_eq!(program_headers[0].segment_type, SegmentType::Load);
                    assert_eq!(
                        program_headers[0].segment_flags,
                        vec![SegmentFlag::Execute, SegmentFlag::Read]
                    );
                }
                Machine::PowerPC64 => {
                    // todo
                }
                Machine::S390 => {
                    // todo
                }
                Machine::Other(_) => unimplemented!(),
            }
        }
    }

    #[test]
    fn test_read_program_headers_gcc_data_elf() {
        // Manually check with command `readelf -l gcc/ARCH/data.elf`

        for arch in IMPLEMENTED_ARCHS {
            let binary = get_example_file_binary(SourceType::GCC, arch, "data.elf");
            let elf = read_file(&binary).unwrap();
            let program_headers = read_program_headers(elf, &binary).unwrap();

            match arch {
                Machine::X86_64 => {
                    // Segment offset, virtual address, file size, memory size and alignment
                    // may vary across different versions of the assembler and platforms.
                    // Check segment types and flags, but not check the other fields.

                    // segment that covers file header and program headers
                    assert_eq!(program_headers[0].segment_type, SegmentType::Load);
                    assert_eq!(program_headers[0].segment_flags, vec![SegmentFlag::Read]);

                    // segment that covers .text
                    assert_eq!(program_headers[1].segment_type, SegmentType::Load);
                    assert_eq!(
                        program_headers[1].segment_flags,
                        vec![SegmentFlag::Execute, SegmentFlag::Read]
                    );

                    // segment that covers .rodata
                    assert_eq!(program_headers[2].segment_type, SegmentType::Load);
                    assert_eq!(program_headers[2].segment_flags, vec![SegmentFlag::Read]);

                    // segment that covers .data and .bss
                    assert_eq!(program_headers[3].segment_type, SegmentType::Load);
                    assert_eq!(
                        program_headers[3].segment_flags,
                        vec![SegmentFlag::Write, SegmentFlag::Read]
                    );
                }
                Machine::AArch64 => {
                    // segment that covers .text, .rodata
                    assert_eq!(program_headers[0].segment_type, SegmentType::Load);
                    assert_eq!(
                        program_headers[0].segment_flags,
                        vec![SegmentFlag::Execute, SegmentFlag::Read]
                    );

                    // segment that covers .data and .bss
                    assert_eq!(program_headers[1].segment_type, SegmentType::Load);
                    assert_eq!(
                        program_headers[1].segment_flags,
                        vec![SegmentFlag::Write, SegmentFlag::Read]
                    );
                }
                Machine::RiscV => {
                    // Segment 0 is RISCV_ATTRIBUTE

                    // segment that covers .text and .rodata
                    assert_eq!(program_headers[1].segment_type, SegmentType::Load);
                    assert_eq!(
                        program_headers[1].segment_flags,
                        vec![SegmentFlag::Execute, SegmentFlag::Read]
                    );

                    // segment that covers .data and .bss
                    assert_eq!(program_headers[2].segment_type, SegmentType::Load);
                    assert_eq!(
                        program_headers[2].segment_flags,
                        vec![SegmentFlag::Write, SegmentFlag::Read]
                    );
                }
                Machine::LoongArch => {
                    assert_eq!(program_headers[0].segment_type, SegmentType::Load);
                    assert_eq!(
                        program_headers[0].segment_flags,
                        vec![SegmentFlag::Execute, SegmentFlag::Read]
                    );
                    assert_eq!(program_headers[1].segment_type, SegmentType::Load);
                    assert_eq!(
                        program_headers[1].segment_flags,
                        vec![SegmentFlag::Write, SegmentFlag::Read]
                    );
                }
                Machine::PowerPC64 => {
                    // todo
                }
                Machine::S390 => {
                    // todo
                }
                Machine::Other(_) => unimplemented!(),
            }
        }
    }
}
