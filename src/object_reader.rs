// Copyright (c) 2026 Hemashushu <hippospark@gmail.com>, All rights reserved.
//
// This Source Code Form is subject to the terms of
// the Mozilla Public License version 2.0 and additional exceptions.
// For more details, see the LICENSE, LICENSE.additional, and CONTRIBUTING files.

#[cfg(test)]
mod tests {
    use std::fs;

    use object::{
        Object, ObjectSection,
        read::elf::{FileHeader, Rela, SectionHeader, Sym},
    };

    fn get_file_binary(file_name: &str) -> Vec<u8> {
        let file_path = std::env::current_dir()
            .unwrap()
            .join("resources/object-file-examples")
            .join(file_name);

        let binary_vec = fs::read(file_path).unwrap();
        binary_vec
    }

    /// Test read ELF64 sections low-levelly
    #[test]
    fn test_read_elf64_sections() {
        let binary_vec = get_file_binary("single.o");
        let binary = binary_vec.as_slice();

        let Ok(elf) = object::elf::FileHeader64::<object::Endianness>::parse(binary) else {
            panic!("Failed to parse ELF64 file");
        };

        let Ok(endian) = elf.endian() else {
            panic!("Failed to determine endianness");
        };

        // Print file header, only architecture and kind are necessary for a linker
        println!("ELF Header:");

        // ET_*, expect `ET_REL`
        println!("  Type: {}", elf.e_type(endian));
        // EM_*, expect `EM_X86_64`, it determines the relocation types (e.g. R_X86_64_PC32, R_X86_64_PLT32)
        println!("  Machine: {}", elf.e_machine(endian));

        let Ok(sections) = elf.sections(endian, binary) else {
            panic!("Failed to get sections");
        };

        println!("Section Headers:");

        for (section_index, section) in sections.enumerate() {
            // The section name is stored in the section header string table (shstrtab),
            // and the index of the section name in the shstrtab is given by the `sh_name` field in the section header.
            let name = str::from_utf8(sections.section_name(endian, section).unwrap()).unwrap();

            println!(
                "index: {}, name: {}, type: {}, virtual address: 0x{:x}, offset (in file): 0x{:x}, size: 0x{:x}, flags: {}, alignment: {}",
                section_index,
                name,
                section.sh_type(endian),
                section.sh_addr(endian),
                section.sh_offset(endian),
                section.sh_size(endian),
                section.sh_flags(endian),
                section.sh_addralign(endian),
            );
        }
    }

    /// Test read ELF64 symbols low-levelly
    #[test]
    fn test_read_elf64_symbols() {
        let binary_vec = get_file_binary("single.o");
        let binary = binary_vec.as_slice();

        let Ok(elf) = object::elf::FileHeader64::<object::Endianness>::parse(binary) else {
            panic!("Failed to parse ELF64 file");
        };

        let Ok(endian) = elf.endian() else {
            panic!("Failed to determine endianness");
        };

        let Ok(sections) = elf.sections(endian, binary) else {
            panic!("Failed to get sections");
        };

        println!("Symbol Table:");

        for (section_index, section) in sections.enumerate() {
            // SHT_SYMTAB: for linkers (for development tools)
            // SHT_DYNSYM: for dynamic linking (for runtime loader)
            //
            // In general, one relocatable object file (ET_REL) only has one symbol table section (SHT_SYMTAB),
            // which name is usually `.symtab`.
            if section.sh_type(endian) == object::elf::SHT_SYMTAB {
                let Ok(Some(symbols)) = section.symbols(endian, binary, &sections, section_index)
                else {
                    panic!("Failed to get symbols");
                };

                for (symbol_index, symbol) in symbols.enumerate() {
                    // The symbol name is stored in the string table (strtab) linked by the symbol table section,
                    // and the index of the symbol name in the strtab is given by the `st_name` field in the symbol table entry.
                    let name =
                        str::from_utf8(symbol.name(endian, symbols.strings()).unwrap()).unwrap();

                    // high 4 bits is the binding (e.g. STB_GLOBAL, STB_LOCAL, and STB_WEAK),
                    // low 4 bits is the type (e.g. STT_FUNC, STT_OBJECT, STT_SECTION, STT_FILE, and STT_COMMON)
                    //
                    // Obtains symbol binding and type from the `st_info` field:
                    //
                    // ```rust
                    // let info = symbol.st_info();
                    // let symbol_bind = info >> 4;
                    // let symbol_type = info & 0x0f;
                    // ```
                    //
                    // Or from the `symbol` trait methods:
                    //
                    // ```rust
                    // symbol.st_bind(),
                    // symbol.st_type()
                    // ```

                    let bind = match symbol.st_bind() {
                        object::elf::STB_LOCAL => "LOCAL",
                        object::elf::STB_GLOBAL => "GLOBAL",
                        object::elf::STB_WEAK => "WEAK",
                        _ => "UNKNOWN",
                    };

                    let type_ = match symbol.st_type() {
                        object::elf::STT_NOTYPE => "NOTYPE",
                        object::elf::STT_OBJECT => "OBJECT",
                        object::elf::STT_FUNC => "FUNC",
                        object::elf::STT_SECTION => "SECTION",
                        object::elf::STT_FILE => "FILE",
                        object::elf::STT_COMMON => "COMMON",
                        _ => "UNKNOWN",
                    };

                    println!(
                        "index: {}, name: {}, value (offset in section): 0x{:x}, size: 0x{:x}, section index: {}, bind: {}, type: {}",
                        symbol_index,
                        name,
                        symbol.st_value(endian),
                        symbol.st_size(endian),
                        symbol.st_shndx(endian),
                        bind,
                        type_
                    );
                }
            }
        }
    }

    /// Test read ELF64 relocations low-levelly
    #[test]
    fn test_read_elf64_relocations() {
        let binary_vec = get_file_binary("single.o");
        let binary = binary_vec.as_slice();

        let Ok(elf) = object::elf::FileHeader64::<object::Endianness>::parse(binary) else {
            panic!("Failed to parse ELF64 file");
        };

        let Ok(endian) = elf.endian() else {
            panic!("Failed to determine endianness");
        };

        let Ok(sections) = elf.sections(endian, binary) else {
            panic!("Failed to get sections");
        };

        println!("Relocations:");

        for (current_section_index, section) in sections.enumerate() {
            // SHT_REL: relocation entries without addends, the addend is stored in the "placeholder".
            // SHT_RELA: relocation entries with addends, the addend is stored in the relocation entry itself.
            //
            // In general, one relocatable object file (ET_REL) may have multiple relocation sections (SHT_REL or SHT_RELA),
            // each of them corresponds to a section that contains placeholders (e.g. `.text`),
            // and the name of the relocation section is usually `.rel.text` or `.rela.text`.
            if section.sh_type(endian) == object::elf::SHT_RELA {
                let Ok(Some((relocations, linked_symbol_table_section_index))) =
                    section.rela(endian, binary)
                else {
                    panic!("Failed to get relocations");
                };

                let apply_target_section_index = section.sh_info(endian);

                println!(
                    "Relocation section index: {}, apply to section index: {}",
                    current_section_index, apply_target_section_index
                );

                for relocation in relocations {
                    // The `r_info` field encodes both the symbol index and the relocation type.
                    // high 32 bits is the symbol index,
                    // low 32 bits is the relocation type
                    //
                    // Obtains symbol index and relocation type from the `r_info` field:
                    //
                    // ```rust
                    // let info = relocation.r_info(endian, elf.is_mips64el(endian));
                    // let symbol_index = info >> 32;
                    // let relocation_type = info & 0xffffffff;
                    // ```
                    //
                    // Or from the `relocation` trait methods:
                    //
                    // ```rust
                    // let symbol_index =relocation.r_sym(endian, elf.is_mips64el(endian));
                    // let relocation_type = relocation.r_type(endian, elf.is_mips64el(endian));
                    // ```

                    let symbols = sections
                        .symbol_table_by_index(endian, binary, linked_symbol_table_section_index)
                        .unwrap();
                    let symbol_index = relocation.symbol(endian, elf.is_mips64el(endian)).unwrap();
                    let symbol = symbols.symbol(symbol_index).unwrap();

                    let name =
                        str::from_utf8(symbol.name(endian, symbols.strings()).unwrap()).unwrap();
                    let offset = symbol.st_value(endian);

                    // Note:
                    // final_patch_value = S (symbol source address = secion address [+ offset] ) + A (addend) - P (placeholder address)

                    println!(
                        "Placeholder offset: 0x{:x}, type: 0x{:x}, sym_name: {}, sym_value (offset in section): 0x{:x}, addend: {}",
                        relocation.r_offset(endian),
                        relocation.r_type(endian, elf.is_mips64el(endian)),
                        name,
                        offset,
                        relocation.r_addend(endian)
                    );
                }
            }
        }
    }

    #[test]
    fn test_read_sections() {
        let binary_vec = get_file_binary("single.o");
        let binary = binary_vec.as_slice();

        let file = object::File::parse(binary).unwrap();

        /* Print file header, only architecture and kind are necessary for a linker */

        // ET_*, expect `ET_REL`
        println!("Kind: {:?}", file.kind());

        // EM_*, expect `EM_X86_64`, it determines the relocation types (e.g. R_X86_64_PC32, R_X86_64_PLT32)
        println!("Architecture: {:?}", file.architecture());

        // Print sections
        for section in file.sections() {
            println!(
                "index: {}, name: {}, kind: {:?}, virtual address: 0x{:x}, file offset : 0x{:x}, size: 0x{:x}",
                section.index().0,
                section.name().unwrap(),
                section.kind(),
                section.address(),
                section.file_range().unwrap().0,
                section.file_range().unwrap().1
            );
        }
    }
}
