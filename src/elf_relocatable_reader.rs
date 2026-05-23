// Copyright (c) 2026 Hemashushu <hippospark@gmail.com>, All rights reserved.
//
// This Source Code Form is subject to the terms of
// the Mozilla Public License version 2.0 and additional exceptions.
// For more details, see the LICENSE, LICENSE.additional, and CONTRIBUTING files.

use object::{
    Endianness,
    read::elf::{FileHeader, Rela, SectionHeader, Sym, SymbolTable},
};

use crate::{
    elf_module::{Module, Relocation, RelocationType, Symbol, SymbolScope, SymbolSection},
    error::LinkerError,
};

const SECTION_NAME_TEXT: &str = ".text";
const SECTION_NAME_RODATA: &str = ".rodata";
const SECTION_NAME_TDATA: &str = ".tdata";
const SECTION_NAME_TBSS: &str = ".tbss";
const SECTION_NAME_DATA: &str = ".data";
const SECTION_NAME_BSS: &str = ".bss";
const SECTION_NAME_SYMTAB: &str = ".symtab";
const SECTION_NAME_RELA_TEXT: &str = ".rela.text";
const SECTION_NAME_RELA_RODATA: &str = ".rela.rodata";
const SECTION_NAME_RELA_DATA: &str = ".rela.data";
const SECTION_NAME_RELA_TDATA: &str = ".rela.tdata";

pub fn read(binary: &[u8]) -> Result<Module, LinkerError> {
    let Ok(elf) = object::elf::FileHeader64::<object::Endianness>::parse(binary) else {
        return Err(LinkerError::new("Failed to parse ELF64 file"));
    };

    let Ok(endian) = elf.endian() else {
        return Err(LinkerError::new("Failed to determine endianness"));
    };

    // ET_*, expect `ET_REL`
    if elf.e_type(endian) != object::elf::ET_REL {
        return Err(LinkerError::new("Unsupported ELF type, expected ET_REL"));
    }

    // EM_*, expect `EM_X86_64`, it determines the relocation types (e.g. R_X86_64_PC32, R_X86_64_PLT32)
    if elf.e_machine(endian) != object::elf::EM_X86_64 {
        return Err(LinkerError::new(
            "Unsupported ELF machine, expected EM_X86_64",
        ));
    }

    let Ok(section_table) = elf.sections(endian, binary) else {
        return Err(LinkerError::new("Failed to read section headers"));
    };

    let mut index_table = IndexTable::new();
    let mut module = Module::new();

    for (section_index, section_header) in section_table.enumerate() {
        // The section name is stored in the section header string table (shstrtab),
        // and the index of the section name in the shstrtab is given by the `sh_name` field in the section header.
        let section_name =
            str::from_utf8(section_table.section_name(endian, section_header).unwrap()).unwrap();
        let section_type = section_header.sh_type(endian);
        let section_tls = (section_header.sh_flags(endian) as u32) & object::elf::SHF_TLS != 0;

        match section_name {
            SECTION_NAME_TEXT if section_type == object::elf::SHT_PROGBITS => {
                module.code = section_header.data(endian, binary).unwrap().to_vec();
                index_table.code = section_index.0;
            }
            SECTION_NAME_RODATA if section_type == object::elf::SHT_PROGBITS => {
                module.rodata = section_header.data(endian, binary).unwrap().to_vec();
                index_table.rodata = section_index.0;
            }
            SECTION_NAME_TDATA if section_tls && section_type == object::elf::SHT_PROGBITS => {
                module.tdata = section_header.data(endian, binary).unwrap().to_vec();
                index_table.tdata = section_index.0;
            }
            SECTION_NAME_TBSS if section_tls && section_type == object::elf::SHT_NOBITS => {
                module.tbss_size = section_header.sh_size(endian) as usize;
                index_table.tbss = section_index.0;
            }
            SECTION_NAME_DATA if section_type == object::elf::SHT_PROGBITS => {
                module.data = section_header.data(endian, binary).unwrap().to_vec();
                index_table.data = section_index.0;
            }
            SECTION_NAME_BSS if section_type == object::elf::SHT_NOBITS => {
                module.bss_size = section_header.sh_size(endian) as usize;
                index_table.bss = section_index.0;
            }
            SECTION_NAME_SYMTAB if section_type == object::elf::SHT_SYMTAB => {
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
                module.symbols = read_symbols(&symbol_table, &index_table, endian)?;
            }
            SECTION_NAME_RELA_TEXT
            | SECTION_NAME_RELA_RODATA
            | SECTION_NAME_RELA_DATA
            | SECTION_NAME_RELA_TDATA
                if section_type == object::elf::SHT_RELA =>
            {
                // There are two types of relocation sections:
                // SHT_REL: relocation entries without addends, the addend is stored in the "placeholder".
                // SHT_RELA: relocation entries with addends, the addend is stored in the relocation entry.
                //
                // In general, one relocatable object file (ET_REL) may have multiple relocation sections (SHT_REL or SHT_RELA),
                // each of them corresponds to a section that contains placeholders (e.g. `.text`),
                // and the name of the relocation section is usually `.rel.text` or `.rela.text`.

                let is_mips64el = elf.is_mips64el(endian);

                let Ok(Some((relas, _linked_symbol_table_section_index))) =
                    section_header.rela(endian, binary)
                else {
                    return Err(LinkerError::new("Failed to read relocation entries"));
                };

                // There are two fields provide more information about the relocation section:
                // - `sh_link`: it gives the index of the symbol table section linked by the
                //   relocation section, and the relocation entries refer to the symbols in that symbol table.
                // - `sh_info`: it gives the index of the section to which the relocation entries apply
                //   (e.g. the `.text` section).
                // But we don't need these two fields because we assume that there is
                // only one symbol table section and one code section.

                match section_name {
                    SECTION_NAME_RELA_TEXT => {
                        module.relocations_code = read_relocations(relas, endian, is_mips64el)?;
                    }
                    SECTION_NAME_RELA_RODATA => {
                        module.relocations_rodata = read_relocations(relas, endian, is_mips64el)?;
                    }
                    SECTION_NAME_RELA_DATA => {
                        module.relocations_data = read_relocations(relas, endian, is_mips64el)?;
                    }
                    SECTION_NAME_RELA_TDATA => {
                        module.relocations_tdata = read_relocations(relas, endian, is_mips64el)?;
                    }
                    _ => {
                        // Unreachable, because we have already matched the section name in the outer match statement.
                        unreachable!()
                    }
                }
            }
            _ => {
                // Ignore other sections.
            }
        }
    }

    Ok(module)
}

fn read_symbols(
    symbol_table: &SymbolTable<object::elf::FileHeader64<Endianness>>,
    index_table: &IndexTable,
    endian: Endianness,
) -> Result<Vec<Symbol>, LinkerError> {
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
        let section_header_index = sym.st_shndx(endian);
        let symbol = if section_header_index == object::elf::SHN_UNDEF {
            if symbol_index.0 == 0 {
                // The first symbol table entry (index 0) is reserved and must be undefined.
                Symbol::Other
            } else {
                // External symbol
                Symbol::External(symbol_name.to_string())
            }
        } else {
            match index_table.get_symbol_section(section_header_index as usize) {
                Some(symbol_section) => {
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
                    let scope = match sym.st_bind() {
                        object::elf::STB_LOCAL => SymbolScope::Local,
                        object::elf::STB_GLOBAL => SymbolScope::Global,
                        object::elf::STB_WEAK => SymbolScope::Weak,
                        bind @ _ => {
                            return Err(LinkerError::new(&format!(
                                "Unsupported symbol bind: {bind}, expected STB_LOCAL, STB_GLOBAL, or STB_WEAK"
                            )));
                        }
                    };

                    // The low 2 bits of the `st_other` field encode the symbol visibility:
                    // - STV_DEFAULT: the symbol is visible to all modules.
                    // - STV_INTERNAL: the symbol is visible only within the module.
                    // - STV_HIDDEN: the symbol is hidden from other modules.
                    // - STV_PROTECTED: the symbol is visible to other modules but cannot be overridden.
                    //
                    // This linker only supports STV_DEFAULT, which is the default visibility for symbols.

                    let offset_origin = sym.st_value(endian) as usize;
                    Symbol::Defined {
                        name: symbol_name.to_string(),
                        symbol_section,
                        scope,
                        offset_origin,
                        offset: 0,
                    }
                }
                None => {
                    // Invalid section index
                    Symbol::Other
                }
            }
        };

        symbols.push(symbol);
    }

    Ok(symbols)
}

fn read_relocations(
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

struct IndexTable {
    code: usize,
    rodata: usize,
    tdata: usize,
    tbss: usize,
    data: usize,
    bss: usize,
}

impl IndexTable {
    fn new() -> Self {
        Self {
            code: 0,
            rodata: 0,
            tdata: 0,
            tbss: 0,
            data: 0,
            bss: 0,
        }
    }

    fn get_symbol_section(&self, index: usize) -> Option<SymbolSection> {
        if index == self.code {
            Some(SymbolSection::Code)
        } else if index == self.rodata {
            Some(SymbolSection::Rodata)
        } else if index == self.tdata {
            Some(SymbolSection::ThreadData)
        } else if index == self.tbss {
            Some(SymbolSection::ThreadBss)
        } else if index == self.data {
            Some(SymbolSection::Data)
        } else if index == self.bss {
            Some(SymbolSection::Bss)
        } else {
            None
        }
    }
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

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::elf_relocatable_reader::read;

    fn get_example_file_binary(file_name: &str) -> Vec<u8> {
        let file_path = std::env::current_dir()
            .unwrap()
            .join("resources/examples/x86_64-linux")
            .join(file_name);

        fs::read(file_path).unwrap()
    }

    #[test]
    fn test_read_mini_o() {
        let binary = get_example_file_binary("mini.o");
        let module = read(&binary).unwrap();
        println!("{:#?}", module);
    }

    #[test]
    fn test_read_hello_world_o() {
        let binary = get_example_file_binary("hello-world.o");
        let module = read(&binary).unwrap();
        println!("{:#?}", module);
    }

    #[test]
    fn test_read_simple_lib_o() {
        let binary = get_example_file_binary("simple-lib.o");
        let module = read(&binary).unwrap();
        println!("{:#?}", module);
    }

    #[test]
    fn test_read_simple_app_o() {
        let binary = get_example_file_binary("simple-app.o");
        let module = read(&binary).unwrap();
        println!("{:#?}", module);
    }

    #[test]
    fn test_read_weak_symbol_lib_o() {
        let binary = get_example_file_binary("weak-symbol-lib.o");
        let module = read(&binary).unwrap();
        println!("{:#?}", module);
    }

    #[test]
    fn test_read_weak_symbol_app_o() {
        let binary = get_example_file_binary("weak-symbol-app.o");
        let module = read(&binary).unwrap();
        println!("{:#?}", module);
    }

    #[test]
    fn test_read_pointer_in_data_o() {
        let binary = get_example_file_binary("pointer-in-data.o");
        let module = read(&binary).unwrap();
        println!("{:#?}", module);
    }

    #[test]
    fn test_read_pointer_in_rodata_o() {
        let binary = get_example_file_binary("pointer-in-rodata.o");
        let module = read(&binary).unwrap();
        println!("{:#?}", module);
    }

    #[test]
    fn test_read_tls_o() {
        let binary = get_example_file_binary("tls.o");
        let module = read(&binary).unwrap();
        println!("{:#?}", module);
    }
}
