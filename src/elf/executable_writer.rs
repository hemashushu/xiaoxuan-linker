// Copyright (c) 2026 Hemashushu <hippospark@gmail.com>, All rights reserved.
//
// This Source Code Form is subject to the terms of
// the Mozilla Public License version 2.0 and additional exceptions.
// For more details, see the LICENSE, LICENSE.additional, and CONTRIBUTING files.

use object::{
    Endianness,
    elf::{
        ELFOSABI_NONE, EM_X86_64, ET_EXEC, PF_R, PF_W, PF_X, PT_LOAD, PT_PHDR, SHF_ALLOC,
        SHF_EXECINSTR, SHT_NOBITS, SHT_PROGBITS,
    },
    write::{
        StringId, WritableBuffer,
        elf::{FileHeader, ProgramHeader, SectionHeader, Writer},
    },
};

use crate::{
    elf::{
        consts::{
            DATA_ALIGN, ELF_HEADER_SIZE, LOAD_ADDR_BASE, PAGE_SIZE, PHDR_SEGMENT_ALIGN,
            PROGRAM_HEADER_ENTRY_SIZE, SYMTAB_ALIGN, TEXT_ALIGN, TLS_SEGMENT_ALIGN,
        },
        linker::LinkResult,
        module::Module,
    },
    error::LinkerError,
};

/// Writes the final executable file after all modules have been linked together.
///
/// Note that the type of the output ELF file is ET_EXEC, not ET_DYN (PIE/DSO),
/// and there is no symbol or relocation in our final executable.
pub fn write_executable(
    modules: &mut [Module],
    link_result: &LinkResult,
    output_buffer: &mut dyn WritableBuffer,
) -> Result<(), LinkerError> {
    // -------------------------------------------------------------------------
    // Assemble the ELF file with object::write::elf::Writer
    // -------------------------------------------------------------------------
    //
    // Writing uses a two phase approach. The first phase builds up all of the information that
    // may need to be known ahead of time:
    //
    // - build string tables
    // - reserve section indices
    // - reserve symbol indices
    // - reserve file ranges for headers and sections
    //
    // Some of the information has ordering requirements. For example, strings must be added
    // to string tables before reserving the file range for the string table. Symbol indices
    // must be reserved after reserving the section indices they reference. There are debug
    // asserts to check some of these requirements.
    //
    // The second phase writes everything out in order. Thus the caller must ensure writing
    // is in the same order that file ranges were reserved. There are debug asserts to assist
    // with checking this.
    //
    // References:
    // https://docs.rs/object/latest/object/write/elf/struct.Writer.html

    // Both `Vec<u8>` and `object::write::StreamingBuffer` implement `WritableBuffer`.
    // `Vec<u8>` is simpler for this example, but `StreamingBuffer` can be used to
    // write directly to a file without buffering the entire contents in memory.
    let mut writer = Writer::new(Endianness::Little, true, output_buffer);

    // -------------------------------------------------------------------------
    // Phase 1: build string tables
    // -------------------------------------------------------------------------

    let section_name_text = writer.add_section_name(b".text");
    let section_name_rodata = writer.add_section_name(b".rodata");
    let mut section_name_tdata_opt: Option<StringId> = None;
    let mut section_name_tbss_opt: Option<StringId> = None;
    if link_result.existing_tls {
        section_name_tdata_opt.replace(writer.add_section_name(b".tdata"));
        section_name_tbss_opt.replace(writer.add_section_name(b".tbss"));
    }
    let section_name_data = writer.add_section_name(b".data");
    let section_name_bss = writer.add_section_name(b".bss");
    writer.add_section_name(b".symtab");
    writer.add_section_name(b".strtab");
    writer.add_section_name(b".shstrtab");

    // -------------------------------------------------------------------------
    // Phase 2: reserve section indices
    // -------------------------------------------------------------------------

    // `reserve_section_index()` returns the reserved section index, which
    // can be used for writing symbol table entries (e.g. `st_shndx` field) later.
    // But we have no symbol or relocation in our final executable,
    // so we don't need to keep track of the reserved section indices.

    writer.reserve_null_section_index(); // null section
    writer.reserve_section_index(); // .text section
    writer.reserve_section_index(); // .rodata section
    if link_result.existing_tls {
        writer.reserve_section_index(); // .tdata section
        writer.reserve_section_index(); // .tbss section
    }
    writer.reserve_section_index(); // .data section
    writer.reserve_section_index(); // .bss section
    writer.reserve_symtab_section_index(); // .symtab section
    writer.reserve_strtab_section_index(); // .strtab section
    writer.reserve_shstrtab_section_index(); // .shstrtab section

    // -------------------------------------------------------------------------
    // Phase 3: reserve symbol indices
    // -------------------------------------------------------------------------

    writer.reserve_null_symbol_index(); // null symbol

    // Call `reserve_symbol_index(None)` to reserve a symbol index for a symbol,
    // however, since we have no symbol in our final executable,
    // we don't need to keep track of the reserved symbol indices.

    // -------------------------------------------------------------------------
    // Phase 4: reserve file ranges for headers
    // -------------------------------------------------------------------------

    writer.reserve_file_header();
    writer.reserve_program_headers(link_result.number_of_program_headers as u32);

    // -------------------------------------------------------------------------
    // Phase 5: reserve file ranges for sections
    // -------------------------------------------------------------------------

    // Reserve space for section `.text`
    let actual_section_offset_text = writer.reserve(link_result.section_size.text, PAGE_SIZE);
    debug_assert_eq!(
        actual_section_offset_text, modules[0].section_offsets.text,
        ".text offset mismatch"
    );

    // Reserve space for section `.rodata`
    let actual_section_offset_rodata = writer.reserve(link_result.section_size.rodata, PAGE_SIZE);
    debug_assert_eq!(
        actual_section_offset_rodata, modules[0].section_offsets.rodata,
        ".rodata offset mismatch"
    );

    // Reserve space for section `.tdata`
    if link_result.existing_tls {
        let actual_section_offset_tdata = writer.reserve(link_result.section_size.tdata, PAGE_SIZE);
        debug_assert_eq!(
            actual_section_offset_tdata, modules[0].section_offsets.tdata,
            ".tdata offset mismatch"
        );
    }

    // Reserve space for section `.data`

    let section_align_data = if link_result.existing_tls {
        // If there is TLS data, the `.data` section must be aligned to `DATA_ALIGN` instead of `PAGE_SIZE`, because
        // it may be merged with `.tdata` section, and the alignment of the merged
        // section must satisfy the strictest alignment requirement among the merged sections.
        DATA_ALIGN
    } else {
        PAGE_SIZE
    };

    let actual_section_offset_data =
        writer.reserve(link_result.section_size.data, section_align_data);
    debug_assert_eq!(
        actual_section_offset_data, modules[0].section_offsets.data,
        ".data offset mismatch"
    );

    writer.reserve_symtab();
    writer.reserve_strtab();
    writer.reserve_shstrtab();
    writer.reserve_section_headers();

    // -------------------------------------------------------------------------
    // Phase 6: write file header binary data
    // -------------------------------------------------------------------------

    // Write ELF header
    writer
        .write_file_header(&FileHeader {
            os_abi: ELFOSABI_NONE,
            abi_version: 0,
            e_type: ET_EXEC,
            e_machine: EM_X86_64,
            e_entry: link_result.entry_point as u64,
            e_flags: 0,
        })
        .expect("failed to write ELF file header");

    // -------------------------------------------------------------------------
    // Phase 7: write program headers binary data
    // -------------------------------------------------------------------------

    // Write padding between ELF header and program headers
    writer.write_align_program_headers();

    // Write PHDR segment header
    let segment_phdr_offset = ELF_HEADER_SIZE;
    let segment_phdr_size = PROGRAM_HEADER_ENTRY_SIZE * link_result.number_of_program_headers;
    let segment_phdr_virtual_address = LOAD_ADDR_BASE + ELF_HEADER_SIZE;

    writer.write_program_header(&ProgramHeader {
        p_type: PT_PHDR,
        p_flags: PF_R,
        p_offset: segment_phdr_offset as u64,
        p_vaddr: segment_phdr_virtual_address as u64,
        p_paddr: segment_phdr_virtual_address as u64,
        p_filesz: segment_phdr_size as u64,
        p_memsz: segment_phdr_size as u64,
        p_align: PHDR_SEGMENT_ALIGN as u64,
    });

    // Write metadata segment header
    let segment_metadata_offset = 0_usize;
    let segment_metadata_size =
        ELF_HEADER_SIZE + PROGRAM_HEADER_ENTRY_SIZE * link_result.number_of_program_headers;
    let segment_metadata_virtual_address = LOAD_ADDR_BASE;

    writer.write_program_header(&ProgramHeader {
        p_type: PT_LOAD,
        p_flags: PF_R,
        p_offset: segment_metadata_offset as u64,
        p_vaddr: segment_metadata_virtual_address as u64,
        p_paddr: segment_metadata_virtual_address as u64,
        p_filesz: segment_metadata_size as u64,
        p_memsz: segment_metadata_size as u64,
        p_align: PAGE_SIZE as u64,
    });

    // Write code segment header
    writer.write_program_header(&ProgramHeader {
        p_type: PT_LOAD,
        p_flags: PF_R | PF_X,
        p_offset: modules[0].section_offsets.text as u64,
        p_vaddr: modules[0].section_virtual_addresses.text as u64,
        p_paddr: modules[0].section_virtual_addresses.text as u64,
        p_filesz: link_result.section_size.text as u64,
        p_memsz: link_result.section_size.text as u64,
        p_align: PAGE_SIZE as u64,
    });

    // Write read-only data segment header
    writer.write_program_header(&ProgramHeader {
        p_type: PT_LOAD,
        p_flags: PF_R,
        p_offset: modules[0].section_offsets.rodata as u64,
        p_vaddr: modules[0].section_virtual_addresses.rodata as u64,
        p_paddr: modules[0].section_virtual_addresses.rodata as u64,
        p_filesz: link_result.section_size.rodata as u64,
        p_memsz: link_result.section_size.rodata as u64,
        p_align: PAGE_SIZE as u64,
    });

    // Write writable data segment header
    let (
        segment_writable_data_offset,
        segment_writable_data_virtual_address,
        segment_writable_data_file_size,
        segment_writable_data_memory_size,
    ) = if link_result.existing_tls {
        (
            modules[0].section_offsets.tdata,
            modules[0].section_virtual_addresses.tdata,
            align_up(link_result.section_size.tdata, DATA_ALIGN) + link_result.section_size.data,
            align_up(link_result.section_size.tdata, DATA_ALIGN)
                + align_up(link_result.section_size.tbss, DATA_ALIGN)
                + align_up(link_result.section_size.data, DATA_ALIGN)
                + link_result.section_size.bss,
        )
    } else {
        (
            modules[0].section_offsets.data,
            modules[0].section_virtual_addresses.data,
            link_result.section_size.data,
            align_up(link_result.section_size.data, DATA_ALIGN) + link_result.section_size.bss,
        )
    };
    writer.write_program_header(&ProgramHeader {
        p_type: PT_LOAD,
        p_flags: PF_R,
        p_offset: segment_writable_data_offset as u64,
        p_vaddr: segment_writable_data_virtual_address as u64,
        p_paddr: segment_writable_data_virtual_address as u64,
        p_filesz: segment_writable_data_file_size as u64,
        p_memsz: segment_writable_data_memory_size as u64,
        p_align: PAGE_SIZE as u64,
    });

    // Write TLS segment header if there is TLS data
    if link_result.existing_tls {
        let segment_tls_file_size = link_result.section_size.tdata;
        let segment_tls_memory_size =
            align_up(link_result.section_size.tdata, DATA_ALIGN) + link_result.section_size.tbss;

        writer.write_program_header(&ProgramHeader {
            p_type: PT_LOAD,
            p_flags: PF_R | PF_W,
            p_offset: modules[0].section_offsets.tdata as u64,
            p_vaddr: modules[0].section_virtual_addresses.tdata as u64,
            p_paddr: modules[0].section_virtual_addresses.tdata as u64,
            p_filesz: segment_tls_file_size as u64,
            p_memsz: segment_tls_memory_size as u64,
            p_align: TLS_SEGMENT_ALIGN as u64,
        });
    }

    // -------------------------------------------------------------------------
    // Phase 8: write sections binary data
    // -------------------------------------------------------------------------

    // Write .text section data
    writer.write_align(PAGE_SIZE);
    for module in modules.iter() {
        writer.write_align(TEXT_ALIGN);
        writer.write(&module.section_binary.text);
    }

    // Write .rodata section data
    writer.write_align(PAGE_SIZE);
    for module in modules.iter() {
        writer.write_align(TEXT_ALIGN);
        writer.write(&module.section_binary.rodata);
    }

    // Write .tdata section data
    if link_result.existing_tls {
        writer.write_align(PAGE_SIZE);
        for module in modules.iter() {
            writer.write_align(DATA_ALIGN);
            writer.write(&module.section_binary.tdata);
        }
    }

    // Note that there is no need to write .tbss and .bss section data,
    // because they are zero-initialized

    // Write .data section data
    writer.write_align(section_align_data);
    for module in modules.iter() {
        writer.write_align(DATA_ALIGN);
        writer.write(&module.section_binary.data);
    }

    // Note that there is no need to write .tbss and .bss section data,
    // because they are zero-initialized

    // -------------------------------------------------------------------------
    // Phase 9: write symbol table
    // -------------------------------------------------------------------------

    // Write symbol table
    writer.write_align(SYMTAB_ALIGN);

    // Note that there is no symbol or relocation in our final executable,

    /*
     * The details of `null` symbol:
     *
     * ```rust
     * writer.write_symbol(&Sym {
     *     name: None,
     *
     *     // Section `.symtab_shndx` index.
     *     // When the section index is beyond 0xffff, the actual section index is stored in
     *     // the `.symtab_shndx` section and the `st_shndx` field here is set to SHN_XINDEX (0xffff).
     *     section: None,
     *
     *     // high 4 bits is the binding (e.g. STB_GLOBAL, STB_LOCAL, and STB_WEAK),
     *     // low 4 bits is the type (e.g. STT_FUNC, STT_OBJECT, STT_SECTION, STT_FILE, and STT_COMMON)
     *     st_info: STB_LOCAL << 4 | STT_NOTYPE,
     *
     *     // Symbol visibility.
     *     // Possible values are STV_DEFAULT, STV_HIDDEN, and STV_PROTECTED etc,.
     *     st_other: STV_DEFAULT,
     *
     *     // section index of the symbol, e.g. 1 for .text, 2 for .rodata,
     *     // and special indices like SHN_UNDEF for undefined symbols and SHN_ABS for absolute symbols
     *     st_shndx: SHN_UNDEF,
     *
     *     // virtual address of the symbol in memory (for defined symbols) or 0 (for undefined symbols).
     *     st_value: 0,
     *
     *     // usually 0
     *     st_size: 0,
     * });
     * ```
     */
    writer.write_null_symbol();

    // -------------------------------------------------------------------------
    // Phase 10: write string tables
    // -------------------------------------------------------------------------

    // Write .strtab section data
    writer.write_strtab();

    // Write .shstrtab section data
    writer.write_shstrtab();

    // -------------------------------------------------------------------------
    // Phase 11: write section headers
    // -------------------------------------------------------------------------

    // Write section header: null
    writer.write_null_section_header();

    // Write section header: .text
    writer.write_section_header(&SectionHeader {
        name: Some(section_name_text),
        sh_type: SHT_PROGBITS,
        sh_flags: (SHF_ALLOC | SHF_EXECINSTR) as u64,
        sh_addr: modules[0].section_virtual_addresses.text as u64,
        sh_offset: modules[0].section_offsets.text as u64,
        sh_size: link_result.section_size.text as u64,

        // depends on the section type, for SHT_PROGBITS it is usually 0
        // for section `.rela.text`, it is the index of the section `.symtab` that holds the symbols.
        // for section `.symtab`, it is the index of the associated string table section (`.strtab`),
        sh_link: 0,

        // depends on the section type, for SHT_PROGBITS it is usually 0
        // for section `.rela.text`, it is the index of the section to which the relocations apply (e.g. `.text`)
        // for section `.symtab`, it is the index of the first non-local symbol (i.e. the number of local symbols)
        sh_info: 0,

        // code sections are usually aligned to 16 bytes
        sh_addralign: TEXT_ALIGN as u64,
        sh_entsize: 0,
    });

    // Write section header: .rodata
    writer.write_section_header(&SectionHeader {
        name: Some(section_name_rodata),
        sh_type: SHT_PROGBITS,
        sh_flags: SHF_ALLOC as u64,
        sh_addr: modules[0].section_virtual_addresses.rodata as u64,
        sh_offset: modules[0].section_offsets.rodata as u64,
        sh_size: link_result.section_size.rodata as u64,
        sh_link: 0,
        sh_info: 0,
        // read-only data sections are usually aligned to 8 or 4 bytes
        sh_addralign: DATA_ALIGN as u64,
        sh_entsize: 0,
    });

    if link_result.existing_tls {
        // Write section header: .tdata
        writer.write_section_header(&SectionHeader {
            name: section_name_tdata_opt,
            sh_type: SHT_PROGBITS,
            sh_flags: (SHF_ALLOC | PF_W) as u64,
            sh_addr: modules[0].section_virtual_addresses.tdata as u64,
            sh_offset: modules[0].section_offsets.tdata as u64,
            sh_size: link_result.section_size.tdata as u64,
            sh_link: 0,
            sh_info: 0,
            // data sections are usually aligned to 8 or 4 bytes
            sh_addralign: DATA_ALIGN as u64,
            sh_entsize: 0,
        });

        // Write section header: .tbss
        writer.write_section_header(&SectionHeader {
            name: section_name_tbss_opt,
            sh_type: SHT_NOBITS,
            sh_flags: (SHF_ALLOC | PF_W) as u64,
            sh_addr: modules[0].section_virtual_addresses.tbss as u64,
            sh_offset: modules[0].section_offsets.tbss as u64,
            // .bss has no data in the file
            sh_size: 0,
            sh_link: 0,
            sh_info: 0,
            // .bss sections are usually aligned to 8 or 4 bytes
            sh_addralign: DATA_ALIGN as u64,
            sh_entsize: 0,
        });
    }

    // Write section header: .data
    writer.write_section_header(&SectionHeader {
        name: Some(section_name_data),
        sh_type: SHT_PROGBITS,
        sh_flags: (SHF_ALLOC | PF_W) as u64,
        sh_addr: modules[0].section_virtual_addresses.data as u64,
        sh_offset: modules[0].section_offsets.data as u64,
        sh_size: link_result.section_size.data as u64,
        sh_link: 0,
        sh_info: 0,
        // data sections are usually aligned to 8 or 4 bytes
        sh_addralign: DATA_ALIGN as u64,
        sh_entsize: 0,
    });

    // Write section header: .bss
    writer.write_section_header(&SectionHeader {
        name: Some(section_name_bss),
        sh_type: SHT_NOBITS,
        sh_flags: (SHF_ALLOC | PF_W) as u64,
        sh_addr: modules[0].section_virtual_addresses.bss as u64,
        sh_offset: modules[0].section_offsets.bss as u64,
        // .bss has no data in the file
        sh_size: 0,
        sh_link: 0,
        sh_info: 0,
        // .bss sections are usually aligned to 8 or 4 bytes
        sh_addralign: DATA_ALIGN as u64,
        sh_entsize: 0,
    });

    // Write section header: .symtab
    let num_local = 1; // only one symbol `null`
    writer.write_symtab_section_header(num_local);

    // Write section header: .strtab
    writer.write_strtab_section_header();

    // Write section header: .shstrtab
    writer.write_shstrtab_section_header();

    Ok(())
}

fn align_up(val: usize, align: usize) -> usize {
    (val + align - 1) & !(align - 1)
}

#[cfg(test)]
mod tests {

    use std::{fs, os::unix::fs::PermissionsExt};

    use object::write::{StreamingBuffer, WritableBuffer};

    use crate::elf::{
        executable_writer::write_executable, linker::link, module::Module,
        relocatable_reader::read_relocatable,
    };

    fn get_example_file_binary(file_name: &str) -> Vec<u8> {
        let file_path = std::env::current_dir()
            .unwrap()
            .join("resources/examples/x86_64-linux")
            .join(file_name);

        fs::read(file_path).unwrap()
    }

    fn get_example_file_module(file_name: &str) -> Module {
        let file_binary = get_example_file_binary(file_name);
        read_relocatable(&file_binary).unwrap()
    }

    fn link_example_files(file_names: &[&str], output_buffer: &mut dyn WritableBuffer) {
        let mut modules: Vec<Module> = file_names
            .iter()
            .map(|file_name| get_example_file_module(file_name))
            .collect();

        let link_result = link(&mut modules).unwrap();

        write_executable(&mut modules, &link_result, output_buffer).unwrap();
    }

    #[test]
    fn test_write_executable() {
        let tmp_dir = std::env::temp_dir();
        let path = tmp_dir.join("test-hello-world.elf");

        let mut file = fs::File::create(&path).unwrap();
        let mut buffer = StreamingBuffer::new(&mut file);
        link_example_files(&["hello-world.o"], &mut buffer);

        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("failed to set permissions");
    }
}
