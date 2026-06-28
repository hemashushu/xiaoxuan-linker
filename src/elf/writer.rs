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
        linker::{
            DATA_ALIGN, ELF_HEADER_SIZE, LOAD_ADDR_BASE, LinkResult, PAGE_SIZE, PHDR_SEGMENT_ALIGN,
            PROGRAM_HEADER_ENTRY_SIZE, SYMTAB_ALIGN, TEXT_ALIGN, TLS_SEGMENT_ALIGN,
        },
        relocatable::{
            RelocatableModule, RelocatableSection, RelocatableSectionType, SectionBinary,
        },
    },
    error::LinkerError,
};

/// Writes the final executable file after all modules have been linked together.
///
/// Note that the type of the output ELF file is ET_EXEC, not ET_DYN (PIE/DSO),
/// and there is no symbol or relocation in our final executable.
pub fn write_executable(
    modules: &mut [RelocatableModule],
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

    // Call `writer.add_string(b"len");` to add a string to the string table and
    // get a `StringId` that can be used for writing symbol table entries later.
    // Since we have no symbol in our final executable,
    // we don't need to add any string to the string table.

    let section_name_text = writer.add_section_name(b".text");
    let mut section_name_rodata_opt: Option<StringId> = None;
    let mut section_name_tdata_opt: Option<StringId> = None;
    let mut section_name_tbss_opt: Option<StringId> = None;
    let mut section_name_data_opt: Option<StringId> = None;
    let mut section_name_bss_opt: Option<StringId> = None;

    if link_result.merged_section_size.rodata > 0 {
        section_name_rodata_opt.replace(writer.add_section_name(b".rodata"));
    }
    if link_result.merged_section_size.tdata > 0 {
        section_name_tdata_opt.replace(writer.add_section_name(b".tdata"));
    }
    if link_result.merged_section_size.tbss > 0 {
        section_name_tbss_opt.replace(writer.add_section_name(b".tbss"));
    }
    if link_result.merged_section_size.data > 0 {
        section_name_data_opt.replace(writer.add_section_name(b".data"));
    }
    if link_result.merged_section_size.bss > 0 {
        section_name_bss_opt.replace(writer.add_section_name(b".bss"));
    }

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

    if link_result.merged_section_size.rodata > 0 {
        writer.reserve_section_index(); // .rodata section
    }
    if link_result.merged_section_size.tdata > 0 {
        writer.reserve_section_index(); // .tdata section
    }

    if link_result.merged_section_size.tbss > 0 {
        writer.reserve_section_index(); // .tbss section
    }
    if link_result.merged_section_size.data > 0 {
        writer.reserve_section_index(); // .data section
    }
    if link_result.merged_section_size.bss > 0 {
        writer.reserve_section_index(); // .bss section
    }

    writer.reserve_symtab_section_index(); // .symtab section
    writer.reserve_strtab_section_index(); // .strtab section
    writer.reserve_shstrtab_section_index(); // .shstrtab section

    // -------------------------------------------------------------------------
    // Phase 3: reserve symbol indices
    // -------------------------------------------------------------------------

    writer.reserve_null_symbol_index(); // null symbol

    // Call `writer.reserve_symbol_index(None)` to reserve a symbol index for a symbol.
    // Since we have no symbol in our final executable,
    // we don't need to keep track of the reserved symbol indices.

    // -------------------------------------------------------------------------
    // Phase 4: reserve file ranges for headers
    // -------------------------------------------------------------------------

    writer.reserve_file_header();
    writer.reserve_program_headers(link_result.program_header_count as u32);

    // -------------------------------------------------------------------------
    // Phase 5: reserve file ranges for sections
    // -------------------------------------------------------------------------

    // Reserve space for section `.text`
    {
        let actual_section_offset_text =
            writer.reserve(link_result.merged_section_size.text, PAGE_SIZE);

        let first_section_text =
            get_first_not_null_section(modules, &RelocatableSectionType::Text).unwrap();
        debug_assert_eq!(
            actual_section_offset_text,
            first_section_text.resolved_offset
        );
    }

    // Reserve space for section `.rodata`
    if link_result.merged_section_size.rodata > 0 {
        let actual_section_offset_rodata =
            writer.reserve(link_result.merged_section_size.rodata, PAGE_SIZE);
        let first_section_rodata =
            get_first_not_null_section(modules, &RelocatableSectionType::RoData).unwrap();
        debug_assert_eq!(
            actual_section_offset_rodata,
            first_section_rodata.resolved_offset
        );
    }

    // Reserve space for section `.tdata`
    if link_result.merged_section_size.tdata > 0 {
        let actual_section_offset_tdata =
            writer.reserve(link_result.merged_section_size.tdata, PAGE_SIZE);

        let first_section_tdata =
            get_first_not_null_section(modules, &RelocatableSectionType::TData).unwrap();
        debug_assert_eq!(
            actual_section_offset_tdata,
            first_section_tdata.resolved_offset
        );
    }

    // Reserve space for section `.data`
    if link_result.merged_section_size.data > 0 {
        // If there is TLS data, the `.data` section must be aligned to `DATA_ALIGN` instead of `PAGE_SIZE`,
        // because it is merged into the writable data segment.
        let section_align_data = if link_result.merged_section_size.tdata > 0 {
            DATA_ALIGN
        } else {
            PAGE_SIZE
        };

        let actual_section_offset_data =
            writer.reserve(link_result.merged_section_size.data, section_align_data);

        let first_section_data =
            get_first_not_null_section(modules, &RelocatableSectionType::Data).unwrap();
        debug_assert_eq!(
            actual_section_offset_data,
            first_section_data.resolved_offset
        );
    }

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
    let segment_phdr_size = PROGRAM_HEADER_ENTRY_SIZE * link_result.program_header_count;
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

    // Common segment type (p_type) includes:
    // - object::elf::PT_NULL
    // - object::elf::PT_PHDR
    // - object::elf::PT_LOAD
    // - object::elf::PT_TLS

    // Write metadata segment header
    // The metadata segment contains the ELF header and program headers,
    // which are required for the loader to load the executable.
    let segment_metadata_offset = 0_usize;
    let segment_metadata_size =
        ELF_HEADER_SIZE + PROGRAM_HEADER_ENTRY_SIZE * link_result.program_header_count;
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
    let first_section_text =
        get_first_not_null_section(modules, &RelocatableSectionType::Text).unwrap();
    writer.write_program_header(&ProgramHeader {
        p_type: PT_LOAD,
        p_flags: PF_R | PF_X,
        p_offset: first_section_text.resolved_offset as u64,
        p_vaddr: first_section_text.resolved_virtual_address as u64,
        p_paddr: first_section_text.resolved_virtual_address as u64,
        p_filesz: link_result.merged_section_size.text as u64,
        p_memsz: link_result.merged_section_size.text as u64,
        p_align: PAGE_SIZE as u64,
    });

    // Write read-only data segment header
    if link_result.has_read_only_data {
        let first_section_rodata =
            get_first_not_null_section(modules, &RelocatableSectionType::RoData).unwrap();
        writer.write_program_header(&ProgramHeader {
            p_type: PT_LOAD,
            p_flags: PF_R,
            p_offset: first_section_rodata.resolved_offset as u64,
            p_vaddr: first_section_rodata.resolved_virtual_address as u64,
            p_paddr: first_section_rodata.resolved_virtual_address as u64,
            p_filesz: link_result.merged_section_size.rodata as u64,
            p_memsz: link_result.merged_section_size.rodata as u64,
            p_align: PAGE_SIZE as u64,
        });
    }

    // Write writable data segment header
    if link_result.has_writable_data {
        let first_writable_section =
            get_first_not_null_section(modules, &RelocatableSectionType::TData)
                .or_else(|| get_first_not_null_section(modules, &RelocatableSectionType::TBss))
                .or_else(|| get_first_not_null_section(modules, &RelocatableSectionType::Data))
                .or_else(|| get_first_not_null_section(modules, &RelocatableSectionType::Bss))
                .unwrap();

        let segment_writable_data_offset = first_writable_section.resolved_offset;
        let segment_writable_data_virtual_address = first_writable_section.resolved_virtual_address;

        let segment_writable_data_file_size = if link_result.has_tls {
            align_up(link_result.merged_section_size.tdata, DATA_ALIGN)
                + link_result.merged_section_size.data
        } else {
            link_result.merged_section_size.data
        };

        let segment_writable_data_memory_size = if link_result.has_tls {
            align_up(link_result.merged_section_size.tdata, DATA_ALIGN)
                + align_up(link_result.merged_section_size.tbss, DATA_ALIGN)
                + align_up(link_result.merged_section_size.data, DATA_ALIGN)
                + link_result.merged_section_size.bss
        } else {
            align_up(link_result.merged_section_size.data, DATA_ALIGN)
                + link_result.merged_section_size.bss
        };

        writer.write_program_header(&ProgramHeader {
            p_type: PT_LOAD,
            p_flags: PF_R | PF_W, // writable data
            p_offset: segment_writable_data_offset as u64,
            p_vaddr: segment_writable_data_virtual_address as u64,
            p_paddr: segment_writable_data_virtual_address as u64,
            p_filesz: segment_writable_data_file_size as u64,
            p_memsz: segment_writable_data_memory_size as u64,
            p_align: PAGE_SIZE as u64,
        });
    }

    // Write TLS segment header if there is TLS data
    if link_result.has_tls {
        let segment_tls_file_size = link_result.merged_section_size.tdata;
        let segment_tls_memory_size = align_up(link_result.merged_section_size.tdata, DATA_ALIGN)
            + link_result.merged_section_size.tbss;

        let first_writable_section =
            get_first_not_null_section(modules, &RelocatableSectionType::TData)
                .or_else(|| get_first_not_null_section(modules, &RelocatableSectionType::TBss))
                .unwrap();

        writer.write_program_header(&ProgramHeader {
            p_type: PT_LOAD,
            p_flags: PF_R | PF_W,
            p_offset: first_writable_section.resolved_offset as u64,
            p_vaddr: first_writable_section.resolved_virtual_address as u64,
            p_paddr: first_writable_section.resolved_virtual_address as u64,
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
        if let Some(section) = module.sections.get(&RelocatableSectionType::Text)
            && section.size > 0
        {
            writer.write_align(TEXT_ALIGN);
            match section.binary {
                SectionBinary::Reference(data) => {
                    writer.write(data);
                }
                SectionBinary::Owned(ref data) => {
                    writer.write(data);
                }
                SectionBinary::None => {
                    //
                }
            }
        }
    }

    // Write .rodata section data
    if link_result.merged_section_size.rodata > 0 {
        writer.write_align(PAGE_SIZE);
        for module in modules.iter() {
            if let Some(section) = module.sections.get(&RelocatableSectionType::RoData)
                && section.size > 0
            {
                writer.write_align(DATA_ALIGN);
                match section.binary {
                    SectionBinary::Reference(data) => {
                        writer.write(data);
                    }
                    SectionBinary::Owned(ref data) => {
                        writer.write(data);
                    }
                    SectionBinary::None => {
                        //
                    }
                }
            }
        }
    }

    // Write .tdata and .data section data
    //
    // Note that there is no need to write .tbss and .bss section data,
    // because they are zero-initialized
    if link_result.merged_section_size.tdata > 0 || link_result.merged_section_size.data > 0 {
        writer.write_align(PAGE_SIZE);

        for module in modules.iter() {
            if let Some(section) = module.sections.get(&RelocatableSectionType::TData)
                && section.size > 0
            {
                writer.write_align(DATA_ALIGN);
                match section.binary {
                    SectionBinary::Reference(data) => {
                        writer.write(data);
                    }
                    SectionBinary::Owned(ref data) => {
                        writer.write(data);
                    }
                    SectionBinary::None => {
                        //
                    }
                }
            }
        }

        for module in modules.iter() {
            if let Some(section) = module.sections.get(&RelocatableSectionType::Data)
                && section.size > 0
            {
                writer.write_align(DATA_ALIGN);
                match section.binary {
                    SectionBinary::Reference(data) => {
                        writer.write(data);
                    }
                    SectionBinary::Owned(ref data) => {
                        writer.write(data);
                    }
                    SectionBinary::None => {
                        //
                    }
                }
            }
        }
    }

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
    {
        let first_section_text =
            get_first_not_null_section(modules, &RelocatableSectionType::Text).unwrap();
        writer.write_section_header(&SectionHeader {
            name: Some(section_name_text),
            sh_type: SHT_PROGBITS,
            sh_flags: (SHF_ALLOC | SHF_EXECINSTR) as u64,
            sh_addr: first_section_text.resolved_virtual_address as u64,
            sh_offset: first_section_text.resolved_offset as u64,
            sh_size: link_result.merged_section_size.text as u64,

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
    }

    // Write section header: .rodata
    if link_result.merged_section_size.rodata > 0 {
        let first_section_rodata =
            get_first_not_null_section(modules, &RelocatableSectionType::RoData).unwrap();
        writer.write_section_header(&SectionHeader {
            name: section_name_rodata_opt,
            sh_type: SHT_PROGBITS,
            sh_flags: SHF_ALLOC as u64,
            sh_addr: first_section_rodata.resolved_virtual_address as u64,
            sh_offset: first_section_rodata.resolved_offset as u64,
            sh_size: link_result.merged_section_size.rodata as u64,
            sh_link: 0,
            sh_info: 0,
            // read-only data sections are usually aligned to 8 or 4 bytes
            sh_addralign: DATA_ALIGN as u64,
            sh_entsize: 0,
        });
    }

    // Write section header: .tdata
    if link_result.merged_section_size.tdata > 0 {
        let first_section_tdata =
            get_first_not_null_section(modules, &RelocatableSectionType::TData).unwrap();
        writer.write_section_header(&SectionHeader {
            name: section_name_tdata_opt,
            sh_type: SHT_PROGBITS,
            sh_flags: (SHF_ALLOC | PF_W) as u64,
            sh_addr: first_section_tdata.resolved_virtual_address as u64,
            sh_offset: first_section_tdata.resolved_offset as u64,
            sh_size: link_result.merged_section_size.tdata as u64,
            sh_link: 0,
            sh_info: 0,
            // data sections are usually aligned to 8 or 4 bytes
            sh_addralign: DATA_ALIGN as u64,
            sh_entsize: 0,
        });
    }

    // Write section header: .tbss
    if link_result.merged_section_size.tbss > 0 {
        let first_section_tbss =
            get_first_not_null_section(modules, &RelocatableSectionType::TBss).unwrap();
        writer.write_section_header(&SectionHeader {
            name: section_name_tbss_opt,
            sh_type: SHT_NOBITS,
            sh_flags: (SHF_ALLOC | PF_W) as u64,
            sh_addr: first_section_tbss.resolved_virtual_address as u64,
            sh_offset: first_section_tbss.resolved_offset as u64,
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
    if link_result.merged_section_size.data > 0 {
        let first_section_data =
            get_first_not_null_section(modules, &RelocatableSectionType::Data).unwrap();
        writer.write_section_header(&SectionHeader {
            name: section_name_data_opt,
            sh_type: SHT_PROGBITS,
            sh_flags: (SHF_ALLOC | PF_W) as u64,
            sh_addr: first_section_data.resolved_virtual_address as u64,
            sh_offset: first_section_data.resolved_offset as u64,
            sh_size: link_result.merged_section_size.data as u64,
            sh_link: 0,
            sh_info: 0,
            // data sections are usually aligned to 8 or 4 bytes
            sh_addralign: DATA_ALIGN as u64,
            sh_entsize: 0,
        });
    }

    // Write section header: .bss
    if link_result.merged_section_size.bss > 0 {
        let first_section_bss =
            get_first_not_null_section(modules, &RelocatableSectionType::Bss).unwrap();
        writer.write_section_header(&SectionHeader {
            name: section_name_bss_opt,
            sh_type: SHT_NOBITS,
            sh_flags: (SHF_ALLOC | PF_W) as u64,
            sh_addr: first_section_bss.resolved_virtual_address as u64,
            sh_offset: first_section_bss.resolved_offset as u64,
            // .bss has no data in the file
            sh_size: 0,
            sh_link: 0,
            sh_info: 0,
            // .bss sections are usually aligned to 8 or 4 bytes
            sh_addralign: DATA_ALIGN as u64,
            sh_entsize: 0,
        });
    }

    // Write section header: .symtab
    let local_symbol_count = 1; // only one symbol - `null`
    writer.write_symtab_section_header(local_symbol_count);

    // Write section header: .strtab
    writer.write_strtab_section_header();

    // Write section header: .shstrtab
    writer.write_shstrtab_section_header();

    Ok(())
}

fn align_up(val: usize, align: usize) -> usize {
    (val + align - 1) & !(align - 1)
}

fn get_first_not_null_section<'a>(
    modules: &'a [RelocatableModule],
    section_type: &RelocatableSectionType,
) -> Option<&'a RelocatableSection<'a>> {
    for module in modules {
        if let Some((_, section)) = module
            .sections
            .iter()
            .find(|(t, s)| *t == section_type && s.size > 0)
        {
            return Some(section);
        }
    }
    None
}

#[cfg(test)]
mod tests {

    use std::{fs, os::unix::fs::PermissionsExt};

    use object::write::{StreamingBuffer, WritableBuffer};

    use crate::elf::{
        linker::link,
        relocatable::{RelocatableModule, read_relocatable},
        writer::write_executable,
    };

    fn get_example_file_binary(file_name: &str) -> Vec<u8> {
        let file_path = std::env::current_dir()
            .unwrap()
            .join("resources/examples/x86_64-linux")
            .join(file_name);

        fs::read(file_path).unwrap()
    }

    fn get_example_file_binaries(file_names: &[&str]) -> Vec<Vec<u8>> {
        file_names
            .iter()
            .map(|file_name| get_example_file_binary(file_name))
            .collect()
    }

    fn get_example_file_module<'a>(file_binary: &'a [u8]) -> RelocatableModule<'a> {
        read_relocatable(file_binary).unwrap()
    }

    fn get_example_file_modules<'a>(file_binaries: &[&'a [u8]]) -> Vec<RelocatableModule<'a>> {
        file_binaries
            .iter()
            .map(|file_binary| get_example_file_module(file_binary))
            .collect()
    }

    fn link_example_files(file_names: &[&str], output_buffer: &mut dyn WritableBuffer) {
        let file_binaries = get_example_file_binaries(file_names);
        let file_binaries_ref: Vec<&[u8]> = file_binaries.iter().map(|b| b.as_slice()).collect();
        let mut modules: Vec<RelocatableModule> = get_example_file_modules(&file_binaries_ref);
        let link_result = link(&mut modules).unwrap();
        write_executable(&mut modules, &link_result, output_buffer).unwrap();
    }

    fn link_example_file_to_executable(file_names: &[&str], output_file_path: &str) {
        let tmp_dir = std::env::temp_dir();
        let path = tmp_dir.join(output_file_path);
        let mut file = fs::File::create(&path).unwrap();
        let mut buffer = StreamingBuffer::new(&mut file);
        link_example_files(file_names, &mut buffer);
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("failed to set permissions");
    }

    #[test]
    fn test_write_minimal() {
        link_example_file_to_executable(&["minimal.o"], "test-minimal.elf");
    }

    #[test]
    fn test_write_function() {
        link_example_file_to_executable(&["function.o"], "test-function.elf");
    }

    #[test]
    fn test_write_data() {
        link_example_file_to_executable(&["data.o"], "test-data.elf");
    }

    #[test]
    fn test_write_symbol() {
        link_example_file_to_executable(&["symbol-export.o", "symbol-import.o"], "test-symbol.elf");
    }

    #[test]
    fn test_write_override() {
        link_example_file_to_executable(
            &["override-weak.o", "override-strong.o"],
            "test-override.elf",
        );
    }

    #[test]
    fn test_write_relocate_within_data() {
        link_example_file_to_executable(
            &["relocate-within-data.o"],
            "test-relocate-within-data.elf",
        );
    }
}
