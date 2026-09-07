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
        DataEncoding, FileClass, FileType, Machine, OSABI, RelocatableModule, Relocation,
        RelocationType, SectionType, SegmentFlag, SegmentType, Symbol, SymbolBind, SymbolType,
    },
    error::LinkerError,
};

pub fn read_file<'a>(
    module_name: &str,
    binary: &'a [u8],
) -> Result<&'a FileHeader64<Endianness>, LinkerError> {
    let Ok(elf) = object::elf::FileHeader64::<object::Endianness>::parse(binary) else {
        return Err(LinkerError::new(&format!(
            "Failed to parse ELF64 module: {}",
            module_name
        )));
    };

    Ok(elf)
}

pub fn read_file_header(
    module_name: &str,
    elf: &FileHeader64<Endianness>,
) -> Result<super::module::FileHeader, LinkerError> {
    let Ok(endian) = elf.endian() else {
        return Err(LinkerError::new(&format!(
            "Failed to determine endianness for module: {}",
            module_name
        )));
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
    module_name: &str,
    elf: &'a FileHeader64<Endianness>,
    binary: &'a [u8],
) -> Result<Vec<super::module::SectionHeader<'a>>, LinkerError> {
    let Ok(endian) = elf.endian() else {
        return Err(LinkerError::new(&format!(
            "Failed to determine endianness for module: {}",
            module_name
        )));
    };

    let Ok(section_table) = elf.sections(endian, binary) else {
        return Err(LinkerError::new(&format!(
            "Failed to read section headers for module: {}",
            module_name
        )));
    };

    let mut sections = vec![];

    for (_section_index, section_header) in section_table.enumerate() {
        // The section name is stored in the section header string table (shstrtab),
        // and the index of the section name in the shstrtab is given by the `sh_name` field in the section header.
        let section_name =
            str::from_utf8(section_table.section_name(endian, section_header).unwrap()).unwrap();

        let virtual_address = section_header.sh_addr(endian) as usize;
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
            virtual_address,
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
    module_name: &str,
    elf: &FileHeader64<Endianness>,
    binary: &[u8],
) -> Result<Vec<Symbol>, LinkerError> {
    let Ok(endian) = elf.endian() else {
        return Err(LinkerError::new(&format!(
            "Failed to determine endianness for module: {}",
            module_name
        )));
    };

    let Ok(section_table) = elf.sections(endian, binary) else {
        return Err(LinkerError::new(&format!(
            "Failed to read section headers for module: {}",
            module_name
        )));
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
                return Err(LinkerError::new(&format!(
                    "Failed to read symbol table for module: {}",
                    module_name
                )));
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

    Err(LinkerError::new(&format!(
        "Failed to find symbol table for module: {}",
        module_name
    )))
}

fn parse_symbol_table(
    symbol_table: &SymbolTable<object::elf::FileHeader64<Endianness>>,
    endian: Endianness,
) -> Result<Vec<Symbol>, LinkerError> {
    // Symbols Example
    //
    //  Local symbols (not visible outside the file):
    //
    // | Index | Value            | Type   | Bind   | Section Index | Name        |
    // |-------|------------------|--------|--------|---------------|-------------|
    // | 0     | 0000000000000000 | NOTYPE | LOCAL  | UND           |             |
    // | 1     | 0000000000000000 | FILE   | LOCAL  | ABS           | hello.asm   |
    // | 2     | 0000000000402000 | NOTYPE | LOCAL  | 2             | msg         |
    // | 3     | 0000000000402007 | NOTYPE | LOCAL  | 2             | len         |
    //
    // Global symbols (visible outside the file):
    //
    // | Index | Value            | Type   | Bind   | Section Index | Name        |
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
                Symbol::Null
            }
            object::elf::SHN_UNDEF => {
                // External symbol
                Symbol::External(symbol_name.to_string())
            }
            object::elf::SHN_ABS if sym.st_type() == object::elf::STT_FILE => {
                // File symbol
                Symbol::File(symbol_name.to_string())
            }
            object::elf::SHN_ABS => {
                // Absolute symbol
                let bind = SymbolBind::from(sym.st_bind());
                let value = sym.st_value(endian);

                Symbol::Absolute {
                    name: symbol_name.to_string(),
                    bind,
                    value,
                }
            }
            _ if section_index >= object::elf::SHN_LORESERVE
                && section_index <= object::elf::SHN_HIRESERVE =>
            {
                // Other section index, such as `SHN_COMMON` (common symbol)
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

                let value = sym.st_value(endian);

                Symbol::Defined {
                    name: symbol_name.to_string(),
                    section_index: section_index as usize,
                    bind,
                    symbol_type,
                    value,
                }
            }
        };

        symbols.push(symbol);
    }

    Ok(symbols)
}

pub fn read_relocation_sections(
    module_name: &str,
    elf: &FileHeader64<Endianness>,
    binary: &[u8],
) -> Result<Vec<super::module::RelocationSection>, LinkerError> {
    let Ok(endian) = elf.endian() else {
        return Err(LinkerError::new(&format!(
            "Failed to determine endianness for module: {}",
            module_name
        )));
    };

    let Ok(section_table) = elf.sections(endian, binary) else {
        return Err(LinkerError::new(&format!(
            "Failed to read section headers for module: {}",
            module_name
        )));
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
                return Err(LinkerError::new(&format!(
                    "Failed to read relocation entries for module: {}",
                    module_name
                )));
            };

            let relocations = parse_relocations(module_name, relas, endian, is_mips64el, machine)?;

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
    module_name: &str,
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

        // the position of placeholder
        let offset = rela.r_offset(endian) as usize;
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
        let relocation_type = parse_relocation_type(module_name, machine, relocation_type_raw)?;

        // Common relocation type (r_type) includes:
        // - object::elf::R_X86_64_64 => "R_X86_64_64"
        // - object::elf::R_X86_64_PC32 => "R_X86_64_PC32"
        // - object::elf::R_X86_64_GOT32 => "R_X86_64_GOT32"
        // - object::elf::R_X86_64_PLT32 => "R_X86_64_PLT32"
        // - object::elf::R_X86_64_RELATIVE => "R_X86_64_RELATIVE"
        // - object::elf::R_X86_64_32 => "R_X86_64_32"

        let relocation = Relocation {
            relocation_type,
            offset,
            symbol_index: symbol_index as usize,
            addend,
        };
        relocations.push(relocation);
    }

    Ok(relocations)
}

fn parse_relocation_type(
    module_name: &str,
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
                    "Unsupported relocation type \"{relocation_type_raw}\" for x86_64 architecture in module \"{module_name}\""
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
                    "Unsupported relocation type \"{relocation_type_raw}\" for AArch64 architecture in module \"{module_name}\""
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
                    "Unsupported relocation type \"{relocation_type_raw}\" for RISC-V architecture in module \"{module_name}\""
                ))),
            }
        }
        Machine::LoongArch => {
            match relocation_type_raw {
                object::elf::R_LARCH_PCALA_HI20 => Ok(RelocationType::R_LARCH_PCALA_HI20),
                object::elf::R_LARCH_PCALA_LO12 => Ok(RelocationType::R_LARCH_PCALA_LO12),
                object::elf::R_LARCH_64 => Ok(RelocationType::R_LARCH_64),
                object::elf::R_LARCH_B26 => Ok(RelocationType::R_LARCH_B26),
                object::elf::R_LARCH_CALL36 => Ok(RelocationType::R_LARCH_CALL36),
                /* unsupported */
                _ => Err(LinkerError::new(&format!(
                    "Unsupported relocation type \"{relocation_type_raw}\" for LoongArch architecture in module \"{module_name}\""
                ))),
            }
        }
        Machine::PowerPC64 => {
            match relocation_type_raw {
                object::elf::R_PPC64_ADDR16_HI => Ok(RelocationType::R_PPC64_ADDR16_HI),
                object::elf::R_PPC64_ADDR16_LO => Ok(RelocationType::R_PPC64_ADDR16_LO),
                object::elf::R_PPC64_ADDR16_HIGHER => Ok(RelocationType::R_PPC64_ADDR16_HIGHER),
                object::elf::R_PPC64_ADDR16_HIGHERA => Ok(RelocationType::R_PPC64_ADDR16_HIGHERA),
                object::elf::R_PPC64_ADDR16_HIGHEST => Ok(RelocationType::R_PPC64_ADDR16_HIGHEST),
                object::elf::R_PPC64_ADDR16_HIGHESTA => Ok(RelocationType::R_PPC64_ADDR16_HIGHESTA),
                object::elf::R_PPC64_ADDR64 => Ok(RelocationType::R_PPC64_ADDR64),
                object::elf::R_PPC64_REL24 => Ok(RelocationType::R_PPC64_REL24),
                object::elf::R_PPC64_REL16_HA => Ok(RelocationType::R_PPC64_REL16_HA),
                object::elf::R_PPC64_REL16_LO => Ok(RelocationType::R_PPC64_REL16_LO),
                object::elf::R_PPC64_TOC16_HA => Ok(RelocationType::R_PPC64_TOC16_HA),
                object::elf::R_PPC64_TOC16_LO => Ok(RelocationType::R_PPC64_TOC16_LO),
                /* unsupported */
                _ => Err(LinkerError::new(&format!(
                    "Unsupported relocation type \"{relocation_type_raw}\" for PowerPC64 architecture in module \"{module_name}\""
                ))),
            }
        }
        Machine::S390 => {
            match relocation_type_raw {
                object::elf::R_390_PC32DBL => Ok(RelocationType::R_390_PC32DBL),
                object::elf::R_390_64 => Ok(RelocationType::R_390_64),
                object::elf::R_390_PLT32DBL => Ok(RelocationType::R_390_PLT32DBL),
                /* unsupported */
                _ => Err(LinkerError::new(&format!(
                    "Unsupported relocation type \"{relocation_type_raw}\" for S390 architecture in module \"{module_name}\""
                ))),
            }
        }
        Machine::Other(_) => unimplemented!(),
    }
}

pub fn read_program_headers(
    module_name: &str,
    elf: &FileHeader64<Endianness>,
    binary: &[u8],
) -> Result<Vec<super::module::ProgramHeader>, LinkerError> {
    let Ok(endian) = elf.endian() else {
        return Err(LinkerError::new(&format!(
            "Failed to determine endianness for module: {}",
            module_name
        )));
    };

    let Ok(segments) = elf.program_headers(endian, binary) else {
        return Err(LinkerError::new(&format!(
            "Failed to read program headers for module: {}",
            module_name
        )));
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

pub fn read_relocatable_module<'a>(
    name: &str,
    binary: &'a [u8],
) -> Result<RelocatableModule<'a>, LinkerError> {
    let elf = read_file(name, binary)?;
    let file_header = read_file_header(name, elf)?;

    if file_header.file_type != FileType::Relocatable {
        return Err(LinkerError::new(&format!(
            "Unsupported ELF type for module: {}, expected relocatable (ET_REL) file",
            name
        )));
    }

    // Check if the machine architecture, endianness, and file class are supported by the linker.
    if file_header.file_class != FileClass::Elf64 {
        return Err(LinkerError::new(&format!(
            "Unsupported file class: {} for module: {}, expected 64-bit ELF (ELFCLASS64)",
            file_header.file_class, name
        )));
    }

    match file_header.machine {
        Machine::X86_64
        | Machine::AArch64
        | Machine::RiscV
        | Machine::LoongArch
        | Machine::PowerPC64 => {
            if file_header.data_encoding != DataEncoding::LittleEndian {
                return Err(LinkerError::new(&format!(
                    "Unsupported data encoding: {} for machine architecture: {} in module: {}, expected little-endian (ELFDATA2LSB)",
                    file_header.data_encoding, file_header.machine, name
                )));
            }
        }
        Machine::S390 => {
            if file_header.data_encoding != DataEncoding::BigEndian {
                return Err(LinkerError::new(&format!(
                    "Unsupported data encoding: {} for machine architecture: {} in module: {}, expected big-endian (ELFDATA2MSB)",
                    file_header.data_encoding, file_header.machine, name
                )));
            }
        }
        _ => {
            return Err(LinkerError::new(&format!(
                "Unsupported machine architecture: {}",
                file_header.machine
            )));
        }
    }

    let sections = read_section_headers(name, elf, binary)?;
    let relocation_sections = read_relocation_sections(name, elf, binary)?;
    let symbols = read_symbols(name, elf, binary)?;

    let relocatable_module = RelocatableModule {
        name: name.to_string(),
        sections,
        symbols,
        relocation_sections,
    };

    Ok(relocatable_module)
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
            read_file, read_file_header, read_program_headers, read_relocatable_module,
            read_relocation_sections, read_section_headers, read_symbols,
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

        const FILE_NAME: &str = "minimal.o";

        for arch in IMPLEMENTED_ARCHS {
            let binary = get_example_file_binary(SourceType::Assembly, arch, FILE_NAME);
            let elf = read_file(FILE_NAME, &binary).unwrap();
            let file_header = read_file_header(FILE_NAME, elf).unwrap();

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

        const FILE_NAME: &str = "minimal.elf";
        for arch in IMPLEMENTED_ARCHS {
            let binary = get_example_file_binary(SourceType::Assembly, arch, FILE_NAME);
            let elf = read_file(FILE_NAME, &binary).unwrap();
            let file_header = read_file_header(FILE_NAME, elf).unwrap();

            assert_eq!(file_header.file_type, FileType::Executable);
        }
    }

    #[test]
    fn test_read_section_header_asm_minimal_o() {
        // Fields such as `size`, `binary`, `align`, and `offset` are not
        // intended to be tested here because they are not guaranteed
        // to be the same across different versions of the assembler and platforms.

        const FILE_NAME: &str = "minimal.o";
        for arch in IMPLEMENTED_ARCHS {
            // Manually check with command `readelf -S asm/ARCH/minimal.o`
            let binary = get_example_file_binary(SourceType::Assembly, arch, FILE_NAME);
            let elf = read_file(FILE_NAME, &binary).unwrap();
            let sections = read_section_headers(FILE_NAME, elf, &binary).unwrap();

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
        const FILE_NAME: &str = "function.o";
        for arch in IMPLEMENTED_ARCHS {
            let binary = get_example_file_binary(SourceType::Assembly, arch, FILE_NAME);
            let elf = read_file(FILE_NAME, &binary).unwrap();
            let sections = read_section_headers(FILE_NAME, elf, &binary).unwrap();

            // Check additional section types
            assert!(matches!(
                sections
                    .iter()
                    .find(|s| s.name == ".rodata"),
                Some(s) if s.section_type == SectionType::Progbits
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
        const FILE_NAME: &str = "minimal.o";
        for arch in IMPLEMENTED_ARCHS {
            let binary = get_example_file_binary(SourceType::Assembly, arch, FILE_NAME);
            let elf = read_file(FILE_NAME, &binary).unwrap();
            let symbols = read_symbols(FILE_NAME, elf, &binary).unwrap();

            // The first symbol table entry (index 0) is reserved and must be undefined.
            assert_eq!(symbols[0], Symbol::Null);

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
        // Manually check with command `readelf -s asm/ARCH/function.o`

        const FILE_NAME: &str = "function.o";
        for arch in IMPLEMENTED_ARCHS {
            let binary = get_example_file_binary(SourceType::Assembly, arch, FILE_NAME);
            let elf = read_file(FILE_NAME, &binary).unwrap();
            let symbols = read_symbols(FILE_NAME, elf, &binary).unwrap();

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

            // Assembler generates `Notype` for function symbols.
            assert!(matches!(
                symbols
                    .iter()
                    .find(|s| matches!(s, Symbol::Defined { name, .. } if name == "print_hello")),
                Some(Symbol::Defined {
                    bind: SymbolBind::Global,
                    symbol_type: SymbolType::Notype,
                    ..
                })
            ));

            assert!(matches!(
                symbols
                    .iter()
                    .find(|s| matches!(s, Symbol::Defined { name, .. } if name == "print_world")),
                Some(Symbol::Defined {
                    bind: SymbolBind::Global,
                    symbol_type: SymbolType::Notype,
                    ..
                })
            ));
        }
    }

    #[test]
    fn test_read_symbols_asm_data_o() {
        // Manually check with command `readelf -s asm/ARCH/data.o`

        const FILE_NAME: &str = "data.o";
        for arch in IMPLEMENTED_ARCHS {
            let binary = get_example_file_binary(SourceType::Assembly, arch, FILE_NAME);
            let elf = read_file(FILE_NAME, &binary).unwrap();
            let symbols = read_symbols(FILE_NAME, elf, &binary).unwrap();

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
                &["foo", "bar", "a", "b", "x", "y"],
            );

            // Check symbols names, types and binds, but not check the section index and offset,
            // because they may vary across different platforms and versions of the assembler.

            // Assembler generates `Notype` for data symbols.
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
        }
    }

    #[test]
    fn test_read_symbols_asm_symbol_export_o() {
        // Manually check with command `readelf -s asm/ARCH/symbol-export.o`

        const FILE_NAME: &str = "symbol-export.o";
        for arch in IMPLEMENTED_ARCHS {
            let binary = get_example_file_binary(SourceType::Assembly, arch, FILE_NAME);
            let elf = read_file(FILE_NAME, &binary).unwrap();
            let symbols = read_symbols(FILE_NAME, elf, &binary).unwrap();

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

        const FILE_NAME: &str = "symbol-import.o";
        for arch in IMPLEMENTED_ARCHS {
            let binary = get_example_file_binary(SourceType::Assembly, arch, FILE_NAME);
            let elf = read_file(FILE_NAME, &binary).unwrap();
            let symbols = read_symbols(FILE_NAME, elf, &binary).unwrap();

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

        const FILE_NAME: &str = "override-weak.o";
        for arch in IMPLEMENTED_ARCHS {
            let binary = get_example_file_binary(SourceType::Assembly, arch, FILE_NAME);
            let elf = read_file(FILE_NAME, &binary).unwrap();
            let symbols = read_symbols(FILE_NAME, elf, &binary).unwrap();

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

        const FILE_NAME: &str = "override-strong.o";
        for arch in IMPLEMENTED_ARCHS {
            let binary = get_example_file_binary(SourceType::Assembly, arch, FILE_NAME);
            let elf = read_file(FILE_NAME, &binary).unwrap();
            let symbols = read_symbols(FILE_NAME, elf, &binary).unwrap();

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
                    bind: SymbolBind::Global,
                    symbol_type: SymbolType::Notype,
                    ..
                })
            ));
        }
    }

    #[test]
    fn test_read_relocations_asm_function_o() {
        // Manually check with command `readelf -r asm/ARCH/function.o`

        const FILE_NAME: &str = "function.o";
        for arch in IMPLEMENTED_ARCHS {
            let binary = get_example_file_binary(SourceType::Assembly, arch, FILE_NAME);
            let elf = read_file(FILE_NAME, &binary).unwrap();
            let sections = read_section_headers(FILE_NAME, elf, &binary).unwrap();
            let symbols = read_symbols(FILE_NAME, elf, &binary).unwrap();
            let relocation_sections = read_relocation_sections(FILE_NAME, elf, &binary).unwrap();

            // Check relocation section `.rela.text`
            let relocation_section_opt =
                relocation_sections.iter().find(|s| s.name == ".rela.text");

            assert!(relocation_section_opt.is_some());

            let relocation_section = relocation_section_opt.unwrap();
            assert_eq!(
                sections[relocation_section.target_section_index].name,
                ".text"
            );

            // Check relocation entries in `.rela.text` section
            let relocations = &relocation_section.relocations;

            // Find the symbol index of `print_hello` in the symbol table
            // Note that the assembler generates relocations with offset that refers to
            // the section instead of the actual symbols if the symbols are local (not global).
            let symbol_index_opt = symbols
                .iter()
                .position(|s| matches!(s, Symbol::Defined { name, ..} if name == "print_hello"));
            assert!(symbol_index_opt.is_some());
            let symbol_index = symbol_index_opt.unwrap();

            // find the relocation entry that refers to the symbol index of `print_hello`
            // and check that its relocation type.
            let relocation_opt = relocations.iter().find(|r| r.symbol_index == symbol_index);
            assert!(relocation_opt.is_some());

            let relocation = relocation_opt.unwrap();

            match arch {
                Machine::X86_64 => {
                    assert_eq!(relocation.relocation_type, RelocationType::R_X86_64_PLT32);
                }
                Machine::AArch64 => {
                    assert_eq!(relocation.relocation_type, RelocationType::R_AARCH64_CALL26);
                }
                Machine::RiscV => {
                    assert_eq!(relocation.relocation_type, RelocationType::R_RISCV_CALL_PLT);
                }
                Machine::LoongArch => {
                    assert_eq!(relocation.relocation_type, RelocationType::R_LARCH_B26);
                }
                Machine::PowerPC64 => {
                    assert_eq!(relocation.relocation_type, RelocationType::R_PPC64_REL24);
                }
                Machine::S390 => {
                    assert_eq!(relocation.relocation_type, RelocationType::R_390_PC32DBL);
                }
                Machine::Other(_) => unimplemented!(),
            }
        }
    }

    #[test]
    fn test_read_relocations_asm_data_o() {
        // Manually check with command `readelf -r asm/ARCH/data.o`

        const FILE_NAME: &str = "data.o";
        for arch in IMPLEMENTED_ARCHS {
            let binary = get_example_file_binary(SourceType::Assembly, arch, FILE_NAME);
            let elf = read_file(FILE_NAME, &binary).unwrap();
            let sections = read_section_headers(FILE_NAME, elf, &binary).unwrap();
            let symbols = read_symbols(FILE_NAME, elf, &binary).unwrap();
            let mut relocation_sections =
                read_relocation_sections(FILE_NAME, elf, &binary).unwrap();

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

            // Find the symbol index of `a` in the symbol table
            // Note that the assembler generates relocations with offset that refers to
            // the section instead of the actual symbols if the symbols are local (not global).
            let symbol_index_opt = symbols
                .iter()
                .position(|s| matches!(s, Symbol::Defined { name, ..} if name == "a"));

            assert!(symbol_index_opt.is_some());
            let symbol_index = symbol_index_opt.unwrap();

            match arch {
                Machine::X86_64 => {
                    let relocation_opt =
                        relocations.iter().find(|r| r.symbol_index == symbol_index);
                    assert!(relocation_opt.is_some());

                    let relocation = relocation_opt.unwrap();
                    assert_eq!(relocation.relocation_type, RelocationType::R_X86_64_PC32);
                }
                Machine::AArch64 => {
                    let rels = relocations
                        .iter()
                        .filter(|r| r.symbol_index == symbol_index)
                        .collect::<Vec<_>>();

                    assert_eq!(
                        rels[0].relocation_type,
                        RelocationType::R_AARCH64_ADR_PREL_PG_HI21
                    );
                    assert_eq!(
                        rels[1].relocation_type,
                        RelocationType::R_AARCH64_LDST64_ABS_LO12_NC
                    );
                }
                Machine::RiscV => {
                    let relo_index_opt = relocations
                        .iter()
                        .position(|r| r.symbol_index == symbol_index);

                    assert!(relo_index_opt.is_some());

                    let relo_index = relo_index_opt.unwrap();

                    let relocation_hi = &relocations[relo_index];
                    assert_eq!(
                        relocation_hi.relocation_type,
                        RelocationType::R_RISCV_PCREL_HI20
                    );

                    let relocation_lo = &relocations[relo_index + 1];
                    assert_eq!(
                        relocation_lo.relocation_type,
                        RelocationType::R_RISCV_PCREL_LO12_I
                    );
                }
                Machine::LoongArch => {
                    let rels = relocations
                        .iter()
                        .filter(|r| r.symbol_index == symbol_index)
                        .collect::<Vec<_>>();

                    assert_eq!(rels[0].relocation_type, RelocationType::R_LARCH_PCALA_HI20);
                    assert_eq!(rels[1].relocation_type, RelocationType::R_LARCH_PCALA_LO12);
                }
                Machine::PowerPC64 => {
                    let rels = relocations
                        .iter()
                        .filter(|r| r.symbol_index == symbol_index)
                        .collect::<Vec<_>>();

                    assert_eq!(
                        rels[0].relocation_type,
                        RelocationType::R_PPC64_ADDR16_HIGHEST
                    );
                    assert_eq!(
                        rels[1].relocation_type,
                        RelocationType::R_PPC64_ADDR16_HIGHER
                    );
                    assert_eq!(rels[2].relocation_type, RelocationType::R_PPC64_ADDR16_HI);
                    assert_eq!(rels[3].relocation_type, RelocationType::R_PPC64_ADDR16_LO);
                }
                Machine::S390 => {
                    let relocation_opt =
                        relocations.iter().find(|r| r.symbol_index == symbol_index);
                    assert!(relocation_opt.is_some());

                    let relocation = relocation_opt.unwrap();
                    assert_eq!(relocation.relocation_type, RelocationType::R_390_PC32DBL);
                }
                Machine::Other(_) => unimplemented!(),
            }
        }
    }

    #[test]
    fn test_read_relocations_asm_relocate_within_data_o() {
        // Manually check with command `readelf -r asm/ARCH/relocate-within-data.o`

        const FILE_NAME: &str = "relocate-within-data.o";
        for arch in IMPLEMENTED_ARCHS {
            let binary = get_example_file_binary(SourceType::Assembly, arch, FILE_NAME);
            let elf = read_file(FILE_NAME, &binary).unwrap();
            let sections = read_section_headers(FILE_NAME, elf, &binary).unwrap();
            let symbols = read_symbols(FILE_NAME, elf, &binary).unwrap();
            let mut relocation_sections =
                read_relocation_sections(FILE_NAME, elf, &binary).unwrap();

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
                relocations.sort_by_key(|a| a.offset);

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
                        let relocation0 = &relocations[0];
                        let symbol0 = &symbols[relocation0.symbol_index];
                        assert_eq!(relocation0.relocation_type, RelocationType::R_PPC64_ADDR64);
                        assert!(matches!(symbol0, Symbol::Defined{name, ..} if name == "dec"));

                        let relocation1 = &relocations[1];
                        let symbol1 = &symbols[relocation1.symbol_index];
                        assert_eq!(relocation1.relocation_type, RelocationType::R_PPC64_ADDR64);
                        assert!(matches!(symbol1, Symbol::Defined{name, ..} if name == "inc"));
                    }
                    Machine::S390 => {
                        let relocation0 = &relocations[0];
                        let symbol0 = &symbols[relocation0.symbol_index];
                        assert_eq!(relocation0.relocation_type, RelocationType::R_390_64);
                        assert!(matches!(symbol0, Symbol::Defined{name, ..} if name == "dec"));

                        let relocation1 = &relocations[1];
                        let symbol1 = &symbols[relocation1.symbol_index];
                        assert_eq!(relocation1.relocation_type, RelocationType::R_390_64);
                        assert!(matches!(symbol1, Symbol::Defined{name, ..} if name == "inc"));
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
                relocations.sort_by_key(|a| a.offset);

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
                        // The assembler generates relocations with offset that refers to the section symbols, not the actual symbols.
                        let relocation0 = &relocations[0];
                        let symbol0 = &symbols[relocation0.symbol_index];
                        assert_eq!(relocation0.relocation_type, RelocationType::R_PPC64_ADDR64);
                        assert!(
                            matches!(symbol0, Symbol::Defined { section_index, ..} if sections[*section_index].name == ".data")
                        );

                        let relocation1 = &relocations[1];
                        let symbol1 = &symbols[relocation1.symbol_index];
                        assert_eq!(relocation1.relocation_type, RelocationType::R_PPC64_ADDR64);
                        assert!(
                            matches!(symbol1, Symbol::Defined { section_index, ..} if sections[*section_index].name == ".data")
                        );
                    }
                    Machine::S390 => {
                        // The assembler generates relocations with offset that refers to the section symbols, not the actual symbols.
                        let relocation0 = &relocations[0];
                        let symbol0 = &symbols[relocation0.symbol_index];
                        assert_eq!(relocation0.relocation_type, RelocationType::R_390_64);
                        assert!(
                            matches!(symbol0, Symbol::Defined { section_index, ..} if sections[*section_index].name == ".data")
                        );

                        let relocation1 = &relocations[1];
                        let symbol1 = &symbols[relocation1.symbol_index];
                        assert_eq!(relocation1.relocation_type, RelocationType::R_390_64);
                        assert!(
                            matches!(symbol1, Symbol::Defined { section_index, ..} if sections[*section_index].name == ".data")
                        );
                    }
                    Machine::Other(_) => unimplemented!(),
                }
            }
        }
    }

    #[test]
    fn test_read_program_headers_asm_minimal_o() {
        // Manually check with command `readelf -l asm/ARCH/minimal.o`

        const FILE_NAME: &str = "minimal.o";
        for arch in IMPLEMENTED_ARCHS {
            let binary = get_example_file_binary(SourceType::Assembly, arch, FILE_NAME);
            let elf = read_file(FILE_NAME, &binary).unwrap();
            let program_headers = read_program_headers(FILE_NAME, elf, &binary).unwrap();

            assert!(program_headers.is_empty());
        }
    }

    #[test]
    fn test_read_program_headers_asm_minimal_elf() {
        // Manually check with command `readelf -l asm/ARCH/minimal.elf`

        const FILE_NAME: &str = "minimal.elf";
        for arch in IMPLEMENTED_ARCHS {
            let binary = get_example_file_binary(SourceType::Assembly, arch, FILE_NAME);
            let elf = read_file(FILE_NAME, &binary).unwrap();
            let program_headers = read_program_headers(FILE_NAME, elf, &binary).unwrap();

            match arch {
                Machine::X86_64 => {
                    // Segment offset, virtual address, file size, memory size and alignment
                    // may vary across different versions of the assembler and platforms.
                    // Check segment types and flags, but not check the other fields.

                    // segment 0 covers file header and program headers
                    assert_eq!(program_headers[0].segment_type, SegmentType::Load);
                    assert_eq!(program_headers[0].segment_flags, vec![SegmentFlag::Read]);

                    // segment 1 covers .text
                    assert_eq!(program_headers[1].segment_type, SegmentType::Load);
                    assert_eq!(
                        program_headers[1].segment_flags,
                        vec![SegmentFlag::Execute, SegmentFlag::Read]
                    );
                }
                Machine::AArch64 => {
                    // segment 0 covers file header, program headers and .text
                    assert_eq!(program_headers[0].segment_type, SegmentType::Load);
                    assert_eq!(
                        program_headers[0].segment_flags,
                        vec![SegmentFlag::Execute, SegmentFlag::Read]
                    );
                }
                Machine::RiscV => {
                    // segment 0 is RISCV_ATTRIBUTE

                    // segment 1 covers file header, program headers and .text
                    assert_eq!(program_headers[1].segment_type, SegmentType::Load);
                    assert_eq!(
                        program_headers[1].segment_flags,
                        vec![SegmentFlag::Execute, SegmentFlag::Read]
                    );
                }
                Machine::LoongArch => {
                    // segment 0 covers file header, program headers and .text
                    assert_eq!(program_headers[0].segment_type, SegmentType::Load);
                    assert_eq!(
                        program_headers[0].segment_flags,
                        vec![SegmentFlag::Execute, SegmentFlag::Read]
                    );
                }
                Machine::PowerPC64 => {
                    // segment 0 covers file header, program headers and .text
                    assert_eq!(program_headers[0].segment_type, SegmentType::Load);
                    assert_eq!(
                        program_headers[0].segment_flags,
                        vec![SegmentFlag::Execute, SegmentFlag::Read]
                    );
                }
                Machine::S390 => {
                    // segment 0 covers file header, program headers and .text
                    assert_eq!(program_headers[0].segment_type, SegmentType::Load);
                    assert_eq!(
                        program_headers[0].segment_flags,
                        vec![SegmentFlag::Execute, SegmentFlag::Read]
                    );
                }
                Machine::Other(_) => unimplemented!(),
            }
        }
    }

    #[test]
    fn test_read_program_headers_asm_data_elf() {
        // Manually check with command `readelf -l asm/ARCH/data.elf`

        const FILE_NAME: &str = "data.elf";
        for arch in IMPLEMENTED_ARCHS {
            let binary = get_example_file_binary(SourceType::Assembly, arch, FILE_NAME);
            let elf = read_file(FILE_NAME, &binary).unwrap();
            let program_headers = read_program_headers(FILE_NAME, elf, &binary).unwrap();

            match arch {
                Machine::X86_64 => {
                    // Segment offset, virtual address, file size, memory size and alignment
                    // may vary across different versions of the assembler and platforms.
                    // Check segment types and flags, but not check the other fields.

                    // segment 0 covers file header and program headers
                    assert_eq!(program_headers[0].segment_type, SegmentType::Load);
                    assert_eq!(program_headers[0].segment_flags, vec![SegmentFlag::Read]);

                    // segment 1 covers .text
                    assert_eq!(program_headers[1].segment_type, SegmentType::Load);
                    assert_eq!(
                        program_headers[1].segment_flags,
                        vec![SegmentFlag::Execute, SegmentFlag::Read]
                    );

                    // segment 2 covers .rodata
                    assert_eq!(program_headers[2].segment_type, SegmentType::Load);
                    assert_eq!(program_headers[2].segment_flags, vec![SegmentFlag::Read]);

                    // segment 3 covers .data and .bss
                    assert_eq!(program_headers[3].segment_type, SegmentType::Load);
                    assert_eq!(
                        program_headers[3].segment_flags,
                        vec![SegmentFlag::Write, SegmentFlag::Read]
                    );
                }
                Machine::AArch64 => {
                    // segment 0 covers file header, program headers, .text and .rodata
                    assert_eq!(program_headers[0].segment_type, SegmentType::Load);
                    assert_eq!(
                        program_headers[0].segment_flags,
                        vec![SegmentFlag::Execute, SegmentFlag::Read]
                    );

                    // segment 1 covers .data and .bss
                    assert_eq!(program_headers[1].segment_type, SegmentType::Load);
                    assert_eq!(
                        program_headers[1].segment_flags,
                        vec![SegmentFlag::Write, SegmentFlag::Read]
                    );
                }
                Machine::RiscV => {
                    // Segment 0 is RISCV_ATTRIBUTE

                    // segment 1 covers file header, program headers, .text and .rodata
                    assert_eq!(program_headers[1].segment_type, SegmentType::Load);
                    assert_eq!(
                        program_headers[1].segment_flags,
                        vec![SegmentFlag::Execute, SegmentFlag::Read]
                    );

                    // segment 2 covers .data and .bss
                    assert_eq!(program_headers[2].segment_type, SegmentType::Load);
                    assert_eq!(
                        program_headers[2].segment_flags,
                        vec![SegmentFlag::Write, SegmentFlag::Read]
                    );
                }
                Machine::LoongArch => {
                    // segment 0 covers file header, program headers, .text and .rodata
                    assert_eq!(program_headers[0].segment_type, SegmentType::Load);
                    assert_eq!(
                        program_headers[0].segment_flags,
                        vec![SegmentFlag::Execute, SegmentFlag::Read]
                    );

                    // segment 1 covers .data and .bss
                    assert_eq!(program_headers[1].segment_type, SegmentType::Load);
                    assert_eq!(
                        program_headers[1].segment_flags,
                        vec![SegmentFlag::Write, SegmentFlag::Read]
                    );
                }
                Machine::PowerPC64 => {
                    // segment 0 covers file header, program headers, .text and .rodata
                    assert_eq!(program_headers[0].segment_type, SegmentType::Load);
                    assert_eq!(
                        program_headers[0].segment_flags,
                        vec![SegmentFlag::Execute, SegmentFlag::Read]
                    );

                    // segment 1 covers .data and .bss
                    assert_eq!(program_headers[1].segment_type, SegmentType::Load);
                    assert_eq!(
                        program_headers[1].segment_flags,
                        vec![SegmentFlag::Write, SegmentFlag::Read]
                    );
                }
                Machine::S390 => {
                    // segment 0 covers file header, program headers, .text and .rodata
                    assert_eq!(program_headers[0].segment_type, SegmentType::Load);
                    assert_eq!(
                        program_headers[0].segment_flags,
                        vec![SegmentFlag::Execute, SegmentFlag::Read]
                    );

                    // segment 1 covers .data and .bss
                    assert_eq!(program_headers[1].segment_type, SegmentType::Load);
                    assert_eq!(
                        program_headers[1].segment_flags,
                        vec![SegmentFlag::Write, SegmentFlag::Read]
                    );
                }
                Machine::Other(_) => unimplemented!(),
            }
        }
    }

    #[test]
    fn test_read_relocatable_module_asm_minimal_o() {
        const FILE_NAME: &str = "minimal.o";
        for arch in IMPLEMENTED_ARCHS {
            let binary = get_example_file_binary(SourceType::Assembly, arch, FILE_NAME);
            let relocatable_module_result = read_relocatable_module(FILE_NAME, &binary);
            assert!(relocatable_module_result.is_ok());
        }
    }

    // ===============================================
    // C programs compiled with `gcc` for testing purposes
    // ===============================================

    #[test]
    fn test_read_file_header_gcc_minimal_o() {
        // Manually check with command `readelf -h gcc/ARCH/minimal.o`

        const FILE_NAME: &str = "minimal.o";
        for arch in IMPLEMENTED_ARCHS {
            let binary = get_example_file_binary(SourceType::GCC, arch, FILE_NAME);
            let elf = read_file(FILE_NAME, &binary).unwrap();
            let file_header = read_file_header(FILE_NAME, elf).unwrap();

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

        const FILE_NAME: &str = "minimal.elf";
        for arch in IMPLEMENTED_ARCHS {
            let binary = get_example_file_binary(SourceType::GCC, arch, FILE_NAME);
            let elf = read_file(FILE_NAME, &binary).unwrap();
            let file_header = read_file_header(FILE_NAME, elf).unwrap();

            assert_eq!(file_header.file_type, FileType::Executable);
        }
    }

    #[test]
    fn test_read_section_header_gcc_minimal_o() {
        // Fields such as `size`, `binary`, `align`, and `offset` are not
        // intended to be tested here because they are not guaranteed
        // to be the same across different versions of the assembler and platforms.

        const FILE_NAME: &str = "minimal.o";
        for arch in IMPLEMENTED_ARCHS {
            // Manually check with command `readelf -S gcc/ARCH/minimal.o`
            let binary = get_example_file_binary(SourceType::GCC, arch, FILE_NAME);
            let elf = read_file(FILE_NAME, &binary).unwrap();
            let sections = read_section_headers(FILE_NAME, elf, &binary).unwrap();

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

        const FILE_NAME: &str = "function.o";
        for arch in IMPLEMENTED_ARCHS {
            let binary = get_example_file_binary(SourceType::GCC, arch, FILE_NAME);
            let elf = read_file(FILE_NAME, &binary).unwrap();
            let sections = read_section_headers(FILE_NAME, elf, &binary).unwrap();

            // Check additional section types
            assert!(matches!(
                sections
                    .iter()
                    .find(|s| s.name == ".rodata"),
                Some(s) if s.section_type == SectionType::Progbits
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
        const FILE_NAME: &str = "minimal.o";
        for arch in IMPLEMENTED_ARCHS {
            let binary = get_example_file_binary(SourceType::GCC, arch, FILE_NAME);
            let elf = read_file(FILE_NAME, &binary).unwrap();
            let symbols = read_symbols(FILE_NAME, elf, &binary).unwrap();

            // The first symbol table entry (index 0) is reserved and must be undefined.
            assert_eq!(symbols[0], Symbol::Null);

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
        // Manually check with command `readelf -s gcc/ARCH/function.o`

        const FILE_NAME: &str = "function.o";
        for arch in IMPLEMENTED_ARCHS {
            let binary = get_example_file_binary(SourceType::GCC, arch, FILE_NAME);
            let elf = read_file(FILE_NAME, &binary).unwrap();
            let symbols = read_symbols(FILE_NAME, elf, &binary).unwrap();

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

            // GCC generates correct symbol type for data and function symbols,
            // while the assembler generates `Notype` for data and function symbols.
            assert!(matches!(
                symbols
                    .iter()
                    .find(|s| matches!(s, Symbol::Defined { name, .. } if name == "print_hello")),
                Some(Symbol::Defined {
                    bind: SymbolBind::Global,
                    symbol_type: SymbolType::Func,
                    ..
                })
            ));

            assert!(matches!(
                symbols
                    .iter()
                    .find(|s| matches!(s, Symbol::Defined { name, .. } if name == "print_world")),
                Some(Symbol::Defined {
                    bind: SymbolBind::Global,
                    symbol_type: SymbolType::Func,
                    ..
                })
            ));
        }
    }

    #[test]
    fn test_read_symbols_gcc_data_o() {
        // Manually check with command `readelf -s gcc/ARCH/data.o`

        const FILE_NAME: &str = "data.o";

        for arch in IMPLEMENTED_ARCHS {
            let binary = get_example_file_binary(SourceType::GCC, arch, FILE_NAME);
            let elf = read_file(FILE_NAME, &binary).unwrap();
            let symbols = read_symbols(FILE_NAME, elf, &binary).unwrap();

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
                &["foo", "bar", "a", "b", "x", "y"],
            );

            // Check symbols names, types and binds, but not check the section index and offset,
            // because they may vary across different platforms and versions of the assembler.

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
        }
    }

    #[test]
    fn test_read_symbols_gcc_symbol_export_o() {
        // Manually check with command `readelf -s gcc/ARCH/symbol-export.o`

        const FILE_NAME: &str = "symbol-export.o";
        for arch in IMPLEMENTED_ARCHS {
            let binary = get_example_file_binary(SourceType::GCC, arch, FILE_NAME);
            let elf = read_file(FILE_NAME, &binary).unwrap();
            let symbols = read_symbols(FILE_NAME, elf, &binary).unwrap();

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

        const FILE_NAME: &str = "symbol-import.o";
        for arch in IMPLEMENTED_ARCHS {
            let binary = get_example_file_binary(SourceType::Assembly, arch, FILE_NAME);
            let elf = read_file(FILE_NAME, &binary).unwrap();
            let symbols = read_symbols(FILE_NAME, elf, &binary).unwrap();

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

        const FILE_NAME: &str = "override-weak.o";
        for arch in IMPLEMENTED_ARCHS {
            let binary = get_example_file_binary(SourceType::GCC, arch, FILE_NAME);
            let elf = read_file(FILE_NAME, &binary).unwrap();
            let symbols = read_symbols(FILE_NAME, elf, &binary).unwrap();

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

        const FILE_NAME: &str = "override-strong.o";
        for arch in IMPLEMENTED_ARCHS {
            let binary = get_example_file_binary(SourceType::GCC, arch, FILE_NAME);
            let elf = read_file(FILE_NAME, &binary).unwrap();
            let symbols = read_symbols(FILE_NAME, elf, &binary).unwrap();

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
                    bind: SymbolBind::Global,
                    symbol_type: SymbolType::Func,
                    ..
                })
            ));
        }
    }

    #[test]
    fn test_read_relocations_gcc_function_o() {
        // Manually check with command `readelf -r gcc/ARCH/function.o`

        const FILE_NAME: &str = "function.o";
        for arch in IMPLEMENTED_ARCHS {
            let binary = get_example_file_binary(SourceType::GCC, arch, FILE_NAME);
            let elf = read_file(FILE_NAME, &binary).unwrap();
            let sections = read_section_headers(FILE_NAME, elf, &binary).unwrap();
            let symbols = read_symbols(FILE_NAME, elf, &binary).unwrap();
            let relocation_sections = read_relocation_sections(FILE_NAME, elf, &binary).unwrap();

            // Check relocation section `.rela.text`
            let relocation_section_opt =
                relocation_sections.iter().find(|s| s.name == ".rela.text");

            assert!(relocation_section_opt.is_some());

            let relocation_section = relocation_section_opt.unwrap();
            assert_eq!(
                sections[relocation_section.target_section_index].name,
                ".text"
            );

            // Check relocation entries in `.rela.text` section
            let relocations = &relocation_section.relocations;

            // Find the symbol index of `print_hello` in the symbol table
            // Note that the assembler generates relocations with offset that refers to
            // the section instead of the actual symbols if the symbols are local (not global).
            let symbol_index_opt = symbols
                .iter()
                .position(|s| matches!(s, Symbol::Defined { name, ..} if name == "print_hello"));
            assert!(symbol_index_opt.is_some());
            let symbol_index = symbol_index_opt.unwrap();

            // find the relocation entry that refers to the symbol index of `print_hello`
            // and check that its relocation type.
            let relocation_opt = relocations.iter().find(|r| r.symbol_index == symbol_index);
            assert!(relocation_opt.is_some());

            let relocation = relocation_opt.unwrap();

            match arch {
                Machine::X86_64 => {
                    assert_eq!(relocation.relocation_type, RelocationType::R_X86_64_PLT32);
                }
                Machine::AArch64 => {
                    assert_eq!(relocation.relocation_type, RelocationType::R_AARCH64_CALL26);
                }
                Machine::RiscV => {
                    assert_eq!(relocation.relocation_type, RelocationType::R_RISCV_CALL_PLT);
                }
                Machine::LoongArch => {
                    // this type is different from the assembler-generated
                    assert_eq!(relocation.relocation_type, RelocationType::R_LARCH_CALL36);
                }
                Machine::PowerPC64 => {
                    assert_eq!(relocation.relocation_type, RelocationType::R_PPC64_REL24);
                }
                Machine::S390 => {
                    // this type is different from the assembler-generated
                    assert_eq!(relocation.relocation_type, RelocationType::R_390_PLT32DBL);
                }
                Machine::Other(_) => unimplemented!(),
            }
        }
    }

    #[test]
    fn test_read_relocations_gcc_data_o() {
        // Manually check with command `readelf -r gcc/ARCH/data.o`

        const FILE_NAME: &str = "data.o";
        for arch in IMPLEMENTED_ARCHS {
            let binary = get_example_file_binary(SourceType::GCC, arch, FILE_NAME);
            let elf = read_file(FILE_NAME, &binary).unwrap();
            let sections = read_section_headers(FILE_NAME, elf, &binary).unwrap();
            let symbols = read_symbols(FILE_NAME, elf, &binary).unwrap();
            let mut relocation_sections =
                read_relocation_sections(FILE_NAME, elf, &binary).unwrap();

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

            // Find the symbol index of `a` in the symbol table
            // Note that the assembler generates relocations with offset that refers to
            // the section instead of the actual symbols if the symbols are local (not global).
            let symbol_index_opt = symbols
                .iter()
                .position(|s| matches!(s, Symbol::Defined { name, ..} if name == "a"));

            assert!(symbol_index_opt.is_some());
            let symbol_index = symbol_index_opt.unwrap();

            match arch {
                Machine::X86_64 => {
                    let relocation_opt =
                        relocations.iter().find(|r| r.symbol_index == symbol_index);
                    assert!(relocation_opt.is_some());

                    let relocation = relocation_opt.unwrap();
                    assert_eq!(relocation.relocation_type, RelocationType::R_X86_64_PC32);
                }
                Machine::AArch64 => {
                    let rels = relocations
                        .iter()
                        .filter(|r| r.symbol_index == symbol_index)
                        .collect::<Vec<_>>();

                    assert_eq!(
                        rels[0].relocation_type,
                        RelocationType::R_AARCH64_ADR_PREL_PG_HI21
                    );
                    assert_eq!(
                        rels[1].relocation_type,
                        RelocationType::R_AARCH64_ADD_ABS_LO12_NC
                    );
                }
                Machine::RiscV => {
                    let rels = relocations
                        .iter()
                        .filter(|r| r.symbol_index == symbol_index)
                        .collect::<Vec<_>>();

                    assert_eq!(rels[0].relocation_type, RelocationType::R_RISCV_HI20);
                    assert_eq!(rels[1].relocation_type, RelocationType::R_RISCV_LO12_S);
                }
                Machine::LoongArch => {
                    // Relocation for symbol `a`: R_LARCH_PCALA_HI20 + R_LARCH_PCALA_LO12
                    let rels = relocations
                        .iter()
                        .filter(|r| r.symbol_index == symbol_index)
                        .collect::<Vec<_>>();

                    assert_eq!(rels[0].relocation_type, RelocationType::R_LARCH_PCALA_HI20);
                    assert_eq!(rels[1].relocation_type, RelocationType::R_LARCH_PCALA_LO12);
                }
                Machine::PowerPC64 => {
                    let rels = relocations
                        .iter()
                        .filter(|r| r.symbol_index == symbol_index)
                        .collect::<Vec<_>>();

                    assert_eq!(rels[0].relocation_type, RelocationType::R_PPC64_TOC16_HA);
                    assert_eq!(rels[1].relocation_type, RelocationType::R_PPC64_TOC16_LO);
                }
                Machine::S390 => {
                    let relocation_opt =
                        relocations.iter().find(|r| r.symbol_index == symbol_index);
                    assert!(relocation_opt.is_some());

                    let relocation = relocation_opt.unwrap();
                    assert_eq!(relocation.relocation_type, RelocationType::R_390_PC32DBL);
                }
                Machine::Other(_) => unimplemented!(),
            }
        }
    }

    #[test]
    fn test_read_relocations_gcc_relocate_within_data_o() {
        // Manually check with command `readelf -r gcc/ARCH/relocate-within-data.o`

        const FILE_NAME: &str = "relocate-within-data.o";
        for arch in IMPLEMENTED_ARCHS {
            let binary = get_example_file_binary(SourceType::GCC, arch, FILE_NAME);
            let elf = read_file(FILE_NAME, &binary).unwrap();
            let sections = read_section_headers(FILE_NAME, elf, &binary).unwrap();
            let symbols = read_symbols(FILE_NAME, elf, &binary).unwrap();
            let mut relocation_sections =
                read_relocation_sections(FILE_NAME, elf, &binary).unwrap();

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
                relocations.sort_by_key(|a| a.offset);

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
                        let relocation0 = &relocations[0];
                        let symbol0 = &symbols[relocation0.symbol_index];
                        assert_eq!(relocation0.relocation_type, RelocationType::R_PPC64_ADDR64);
                        assert!(matches!(symbol0, Symbol::Defined{name, ..} if name == "dec"));

                        let relocation1 = &relocations[1];
                        let symbol1 = &symbols[relocation1.symbol_index];
                        assert_eq!(relocation1.relocation_type, RelocationType::R_PPC64_ADDR64);
                        assert!(matches!(symbol1, Symbol::Defined{name, ..} if name == "inc"));
                    }
                    Machine::S390 => {
                        let relocation0 = &relocations[0];
                        let symbol0 = &symbols[relocation0.symbol_index];
                        assert_eq!(relocation0.relocation_type, RelocationType::R_390_64);
                        assert!(matches!(symbol0, Symbol::Defined{name, ..} if name == "dec"));

                        let relocation1 = &relocations[1];
                        let symbol1 = &symbols[relocation1.symbol_index];
                        assert_eq!(relocation1.relocation_type, RelocationType::R_390_64);
                        assert!(matches!(symbol1, Symbol::Defined{name, ..} if name == "inc"));
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
                relocations.sort_by_key(|a| a.offset);

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
                        let relocation0 = &relocations[0];
                        let symbol0 = &symbols[relocation0.symbol_index];
                        assert_eq!(relocation0.relocation_type, RelocationType::R_PPC64_ADDR64);
                        assert!(matches!(symbol0, Symbol::Defined{name, ..} if name == "foo"));

                        let relocation1 = &relocations[1];
                        let symbol1 = &symbols[relocation1.symbol_index];
                        assert_eq!(relocation1.relocation_type, RelocationType::R_PPC64_ADDR64);
                        assert!(matches!(symbol1, Symbol::Defined{name, ..} if name == "bar"));
                    }
                    Machine::S390 => {
                        let relocation0 = &relocations[0];
                        let symbol0 = &symbols[relocation0.symbol_index];
                        assert_eq!(relocation0.relocation_type, RelocationType::R_390_64);
                        assert!(matches!(symbol0, Symbol::Defined{name, ..} if name == "foo"));

                        let relocation1 = &relocations[1];
                        let symbol1 = &symbols[relocation1.symbol_index];
                        assert_eq!(relocation1.relocation_type, RelocationType::R_390_64);
                        assert!(matches!(symbol1, Symbol::Defined{name, ..} if name == "bar"));
                    }
                    Machine::Other(_) => unimplemented!(),
                }
            }
        }
    }

    #[test]
    fn test_read_program_headers_gcc_minimal_o() {
        // Manually check with command `readelf -l gcc/ARCH/minimal.o`

        const FILE_NAME: &str = "minimal.o";
        for arch in IMPLEMENTED_ARCHS {
            let binary = get_example_file_binary(SourceType::GCC, arch, FILE_NAME);
            let elf = read_file(FILE_NAME, &binary).unwrap();
            let program_headers = read_program_headers(FILE_NAME, elf, &binary).unwrap();

            assert!(program_headers.is_empty());
        }
    }

    #[test]
    fn test_read_program_headers_gcc_minimal_elf() {
        // Manually check with command `readelf -l gcc/ARCH/minimal.elf`

        const FILE_NAME: &str = "minimal.elf";
        for arch in IMPLEMENTED_ARCHS {
            let binary = get_example_file_binary(SourceType::GCC, arch, FILE_NAME);
            let elf = read_file(FILE_NAME, &binary).unwrap();
            let program_headers = read_program_headers(FILE_NAME, elf, &binary).unwrap();

            match arch {
                Machine::X86_64 => {
                    // Segment offset, virtual address, file size, memory size and alignment
                    // may vary across different versions of the assembler and platforms.
                    // Check segment types and flags, but not check the other fields.

                    // segment 0 covers file header and program headers
                    assert_eq!(program_headers[0].segment_type, SegmentType::Load);
                    assert_eq!(program_headers[0].segment_flags, vec![SegmentFlag::Read]);

                    // segment 1 covers .text
                    assert_eq!(program_headers[1].segment_type, SegmentType::Load);
                    assert_eq!(
                        program_headers[1].segment_flags,
                        vec![SegmentFlag::Execute, SegmentFlag::Read]
                    );
                }
                Machine::AArch64 => {
                    // segment 0 covers file header, program headers and .text
                    assert_eq!(program_headers[0].segment_type, SegmentType::Load);
                    assert_eq!(
                        program_headers[0].segment_flags,
                        vec![SegmentFlag::Execute, SegmentFlag::Read]
                    );
                }
                Machine::RiscV => {
                    // segment 0 is RISCV_ATTRIBUTE

                    // segment 1 covers file header, program headers and .text
                    assert_eq!(program_headers[1].segment_type, SegmentType::Load);
                    assert_eq!(
                        program_headers[1].segment_flags,
                        vec![SegmentFlag::Execute, SegmentFlag::Read]
                    );
                }
                Machine::LoongArch => {
                    // segment 0 covers file header, program headers and .text
                    assert_eq!(program_headers[0].segment_type, SegmentType::Load);
                    assert_eq!(
                        program_headers[0].segment_flags,
                        vec![SegmentFlag::Execute, SegmentFlag::Read]
                    );
                }
                Machine::PowerPC64 => {
                    // segment 0 covers file header, program headers and .text
                    assert_eq!(program_headers[0].segment_type, SegmentType::Load);
                    assert_eq!(
                        program_headers[0].segment_flags,
                        vec![SegmentFlag::Execute, SegmentFlag::Read]
                    );
                }
                Machine::S390 => {
                    // segment 0 covers file header, program headers and .text
                    assert_eq!(program_headers[0].segment_type, SegmentType::Load);
                    assert_eq!(
                        program_headers[0].segment_flags,
                        vec![SegmentFlag::Execute, SegmentFlag::Read]
                    );
                }
                Machine::Other(_) => unimplemented!(),
            }
        }
    }

    #[test]
    fn test_read_program_headers_gcc_data_elf() {
        // Manually check with command `readelf -l gcc/ARCH/data.elf`

        const FILE_NAME: &str = "data.elf";
        for arch in IMPLEMENTED_ARCHS {
            let binary = get_example_file_binary(SourceType::GCC, arch, FILE_NAME);
            let elf = read_file(FILE_NAME, &binary).unwrap();
            let program_headers = read_program_headers(FILE_NAME, elf, &binary).unwrap();

            match arch {
                Machine::X86_64 => {
                    // Segment offset, virtual address, file size, memory size and alignment
                    // may vary across different versions of the assembler and platforms.
                    // Check segment types and flags, but not check the other fields.

                    // segment 0 covers file header and program headers
                    assert_eq!(program_headers[0].segment_type, SegmentType::Load);
                    assert_eq!(program_headers[0].segment_flags, vec![SegmentFlag::Read]);

                    // segment 1 covers .text
                    assert_eq!(program_headers[1].segment_type, SegmentType::Load);
                    assert_eq!(
                        program_headers[1].segment_flags,
                        vec![SegmentFlag::Execute, SegmentFlag::Read]
                    );

                    // segment 2 covers .rodata
                    assert_eq!(program_headers[2].segment_type, SegmentType::Load);
                    assert_eq!(program_headers[2].segment_flags, vec![SegmentFlag::Read]);

                    // segment 3 covers .data and .bss
                    assert_eq!(program_headers[3].segment_type, SegmentType::Load);
                    assert_eq!(
                        program_headers[3].segment_flags,
                        vec![SegmentFlag::Write, SegmentFlag::Read]
                    );
                }
                Machine::AArch64 => {
                    // segment 0 covers file header, program headers, .text and .rodata
                    assert_eq!(program_headers[0].segment_type, SegmentType::Load);
                    assert_eq!(
                        program_headers[0].segment_flags,
                        vec![SegmentFlag::Execute, SegmentFlag::Read]
                    );

                    // segment 1 covers .data and .bss
                    assert_eq!(program_headers[1].segment_type, SegmentType::Load);
                    assert_eq!(
                        program_headers[1].segment_flags,
                        vec![SegmentFlag::Write, SegmentFlag::Read]
                    );
                }
                Machine::RiscV => {
                    // Segment 0 is RISCV_ATTRIBUTE

                    // segment 1 covers file header, program headers, .text and .rodata
                    assert_eq!(program_headers[1].segment_type, SegmentType::Load);
                    assert_eq!(
                        program_headers[1].segment_flags,
                        vec![SegmentFlag::Execute, SegmentFlag::Read]
                    );

                    // segment 2 covers .data and .bss
                    assert_eq!(program_headers[2].segment_type, SegmentType::Load);
                    assert_eq!(
                        program_headers[2].segment_flags,
                        vec![SegmentFlag::Write, SegmentFlag::Read]
                    );
                }
                Machine::LoongArch => {
                    // segment 0 covers file header, program headers, .text and .rodata
                    assert_eq!(program_headers[0].segment_type, SegmentType::Load);
                    assert_eq!(
                        program_headers[0].segment_flags,
                        vec![SegmentFlag::Execute, SegmentFlag::Read]
                    );

                    // segment 1 covers .data and .bss
                    assert_eq!(program_headers[1].segment_type, SegmentType::Load);
                    assert_eq!(
                        program_headers[1].segment_flags,
                        vec![SegmentFlag::Write, SegmentFlag::Read]
                    );
                }
                Machine::PowerPC64 => {
                    // segment 0 covers file header, program headers, .text and .rodata
                    assert_eq!(program_headers[0].segment_type, SegmentType::Load);
                    assert_eq!(
                        program_headers[0].segment_flags,
                        vec![SegmentFlag::Execute, SegmentFlag::Read]
                    );

                    // segment 1 covers .data and .bss
                    assert_eq!(program_headers[1].segment_type, SegmentType::Load);
                    assert_eq!(
                        program_headers[1].segment_flags,
                        vec![SegmentFlag::Write, SegmentFlag::Read]
                    );
                }
                Machine::S390 => {
                    // segment 0 covers file header, program headers, .text and .rodata
                    assert_eq!(program_headers[0].segment_type, SegmentType::Load);
                    assert_eq!(
                        program_headers[0].segment_flags,
                        vec![SegmentFlag::Execute, SegmentFlag::Read]
                    );

                    // segment 1 covers .data and .bss
                    assert_eq!(program_headers[1].segment_type, SegmentType::Load);
                    assert_eq!(
                        program_headers[1].segment_flags,
                        vec![SegmentFlag::Write, SegmentFlag::Read]
                    );
                }
                Machine::Other(_) => unimplemented!(),
            }
        }
    }

    #[test]
    fn test_read_relocatable_module_gcc_minimal_o() {
        const FILE_NAME: &str = "minimal.o";
        for arch in IMPLEMENTED_ARCHS {
            let binary = get_example_file_binary(SourceType::GCC, arch, FILE_NAME);
            let relocatable_module_result = read_relocatable_module(FILE_NAME, &binary);
            assert!(relocatable_module_result.is_ok());
        }
    }
}
