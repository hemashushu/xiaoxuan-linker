// Copyright (c) 2026 Hemashushu <hippospark@gmail.com>, All rights reserved.
//
// This Source Code Form is subject to the terms of
// the Mozilla Public License version 2.0 and additional exceptions.
// For more details, see the LICENSE, LICENSE.additional, and CONTRIBUTING files.

use object::{
    Endianness,
    elf::{
        EF_LARCH_ABI_DOUBLE_FLOAT, EF_LARCH_OBJABI_V1, EF_RISCV_FLOAT_ABI_DOUBLE, EF_RISCV_RVC,
        ELFOSABI_NONE, ET_EXEC, PF_R, PF_W, PF_X, PT_LOAD, PT_PHDR, PT_TLS, SHF_ALLOC,
        SHF_EXECINSTR, SHF_TLS, SHF_WRITE, SHT_NOBITS, SHT_PROGBITS,
    },
    write::{
        StringId, WritableBuffer,
        elf::{FileHeader, ProgramHeader, SectionHeader, Writer},
    },
};

use crate::{
    elf::{
        merger::{MergedFileLayout, SectionName},
        module::{
            ELF_HEADER_SIZE, Machine, PROGRAM_HEADER_ENTRY_SIZE, SECTION_ALIGN_DATA,
            SECTION_ALIGN_SYMTAB, SECTION_NAME_BSS, SECTION_NAME_DATA, SECTION_NAME_GOT,
            SECTION_NAME_RODATA, SECTION_NAME_SHSTRTAB, SECTION_NAME_STRTAB, SECTION_NAME_SYMTAB,
            SECTION_NAME_TBSS, SECTION_NAME_TDATA, SECTION_NAME_TEXT, SECTION_NAME_TOC,
            SEGMENT_ALIGN_PHDR, SEGMENT_ALIGN_TLS, get_load_address_base, get_section_align_text,
            get_segment_align_page_size,
        },
        relocator::{RelocatedModule, RelocatedSectionBinary},
    },
    error::LinkerError,
};

/// Writes the final executable file after all modules have been linked together.
///
/// Note that the type of the output ELF file is ET_EXEC, not ET_DYN (PIE/DSO),
/// and there is no symbol or relocation in our final executable.
pub fn write_executable(
    relocated_modules: &[RelocatedModule],
    merged_file_layout: &MergedFileLayout,
    entry_point: u64,
    arch: Machine,
    endian: Endianness,
    output_buffer: &mut dyn WritableBuffer,
) -> Result<(), LinkerError> {
    #[allow(non_snake_case)]
    let SEGMENT_ALIGN_PAGE_SIZE = get_segment_align_page_size(arch);

    #[allow(non_snake_case)]
    let LOAD_ADDR_BASE = get_load_address_base(arch);

    #[allow(non_snake_case)]
    let SECTION_ALIGN_TEXT = get_section_align_text(arch);

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
    let mut writer = Writer::new(endian, true, output_buffer);

    // -------------------------------------------------------------------------
    // Phase 1: build string tables
    // -------------------------------------------------------------------------

    // Call `writer.add_string(b"len");` to add a string to the string table and
    // get a `StringId` that can be used for writing symbol table entries later.
    // Since we have no symbol in our final executable,
    // we don't need to add any string to the string table.

    let mut section_name_text_opt: Option<StringId> = None;
    let mut section_name_rodata_opt: Option<StringId> = None;
    let mut section_name_got_opt: Option<StringId> = None;
    let mut section_name_tdata_opt: Option<StringId> = None;
    let mut section_name_tbss_opt: Option<StringId> = None;
    let mut section_name_data_opt: Option<StringId> = None;
    let mut section_name_toc_opt: Option<StringId> = None;
    let mut section_name_bss_opt: Option<StringId> = None;

    {
        section_name_text_opt.replace(writer.add_section_name(SECTION_NAME_TEXT.as_bytes()));
    }

    if merged_file_layout.contains_non_empty_section(SectionName::ROData) {
        section_name_rodata_opt.replace(writer.add_section_name(SECTION_NAME_RODATA.as_bytes()));
    }

    if merged_file_layout.contains_non_empty_section(SectionName::GOT) {
        section_name_got_opt.replace(writer.add_section_name(SECTION_NAME_GOT.as_bytes()));
    }

    if merged_file_layout.contains_non_empty_section(SectionName::TData) {
        section_name_tdata_opt.replace(writer.add_section_name(SECTION_NAME_TDATA.as_bytes()));
    }

    if merged_file_layout.contains_non_empty_section(SectionName::TBSS) {
        section_name_tbss_opt.replace(writer.add_section_name(SECTION_NAME_TBSS.as_bytes()));
    }

    if merged_file_layout.contains_non_empty_section(SectionName::Data) {
        section_name_data_opt.replace(writer.add_section_name(SECTION_NAME_DATA.as_bytes()));
    }

    if merged_file_layout.contains_non_empty_section(SectionName::TOC) {
        section_name_toc_opt.replace(writer.add_section_name(SECTION_NAME_TOC.as_bytes()));
    }

    if merged_file_layout.contains_non_empty_section(SectionName::BSS) {
        section_name_bss_opt.replace(writer.add_section_name(SECTION_NAME_BSS.as_bytes()));
    }

    writer.add_section_name(SECTION_NAME_SYMTAB.as_bytes());
    writer.add_section_name(SECTION_NAME_STRTAB.as_bytes());
    writer.add_section_name(SECTION_NAME_SHSTRTAB.as_bytes());

    // -------------------------------------------------------------------------
    // Phase 2: reserve section indices
    // -------------------------------------------------------------------------

    // `reserve_section_index()` returns the reserved section index, which
    // can be used for writing symbol table entries (e.g. `st_shndx` field) later.
    // But we have no symbol or relocation in our final executable,
    // so we don't need to keep track of the reserved section indices.

    writer.reserve_null_section_index(); // null section

    {
        writer.reserve_section_index(); // .text section
    }

    if merged_file_layout.contains_non_empty_section(SectionName::ROData) {
        writer.reserve_section_index(); // .rodata section
    }

    if merged_file_layout.contains_non_empty_section(SectionName::GOT) {
        writer.reserve_section_index(); // .got section
    }

    if merged_file_layout.contains_non_empty_section(SectionName::TData) {
        writer.reserve_section_index(); // .tdata section
    }

    if merged_file_layout.contains_non_empty_section(SectionName::TBSS) {
        writer.reserve_section_index(); // .tbss section
    }

    if merged_file_layout.contains_non_empty_section(SectionName::Data) {
        writer.reserve_section_index(); // .data section
    }

    if merged_file_layout.contains_non_empty_section(SectionName::TOC) {
        writer.reserve_section_index(); // .toc section
    }

    if merged_file_layout.contains_non_empty_section(SectionName::BSS) {
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
    writer.reserve_program_headers(merged_file_layout.program_header_count as u32);

    // -------------------------------------------------------------------------
    // Phase 5: reserve file ranges for sections
    // -------------------------------------------------------------------------

    // Reserve space for section `.text`
    if let Some(section_info_text) =
        merged_file_layout.get_non_empty_section_info(SectionName::Text)
    {
        let actual_section_offset_text =
            writer.reserve(section_info_text.size, SEGMENT_ALIGN_PAGE_SIZE);

        debug_assert_eq!(
            actual_section_offset_text,
            section_info_text.offset_in_merged_file
        );
    } else {
        // If there is no `.text` section, it is an error because
        // executables must have a `.text` section containing the code.
        return Err(LinkerError::Message(
            "No .text section found in the merged modules.".to_string(),
        ));
    }

    // Reserve space for section `.rodata`
    if let Some(section_info_rodata) =
        merged_file_layout.get_non_empty_section_info(SectionName::ROData)
    {
        let actual_section_offset_rodata =
            writer.reserve(section_info_rodata.size, SEGMENT_ALIGN_PAGE_SIZE);

        debug_assert_eq!(
            actual_section_offset_rodata,
            section_info_rodata.offset_in_merged_file
        );
    }

    if let Some(section_info_got) = merged_file_layout.get_non_empty_section_info(SectionName::GOT)
    {
        let section_align_data =
            if merged_file_layout.contains_non_empty_section(SectionName::ROData) {
                SECTION_ALIGN_DATA
            } else {
                SEGMENT_ALIGN_PAGE_SIZE
            };

        let actual_section_offset_got = writer.reserve(section_info_got.size, section_align_data);

        debug_assert_eq!(
            actual_section_offset_got,
            section_info_got.offset_in_merged_file
        );
    }

    // Reserve space for section `.tdata`
    if let Some(section_info_tdata) =
        merged_file_layout.get_non_empty_section_info(SectionName::TData)
    {
        let actual_section_offset_tdata =
            writer.reserve(section_info_tdata.size, SEGMENT_ALIGN_PAGE_SIZE);

        debug_assert_eq!(
            actual_section_offset_tdata,
            section_info_tdata.offset_in_merged_file
        );
    }

    // Reserve space for section `.data`
    if let Some(section_info_data) =
        merged_file_layout.get_non_empty_section_info(SectionName::Data)
    {
        // If there is TLS data, the `.data` section must be aligned to `DATA_ALIGN` instead of `PAGE_SIZE`,
        // because it is merged into the writable data segment.
        let section_align_data =
            if merged_file_layout.contains_non_empty_section(SectionName::TData) {
                SECTION_ALIGN_DATA
            } else {
                SEGMENT_ALIGN_PAGE_SIZE
            };

        let actual_section_offset_data = writer.reserve(section_info_data.size, section_align_data);

        debug_assert_eq!(
            actual_section_offset_data,
            section_info_data.offset_in_merged_file
        );
    }

    if let Some(section_info_toc) = merged_file_layout.get_non_empty_section_info(SectionName::TOC)
    {
        // If there is TLS data or a `.data` section, the `.toc` section must be aligned to `DATA_ALIGN`
        // instead of `PAGE_SIZE`, because it is merged into the writable data segment.
        let section_align_data = if merged_file_layout
            .contains_non_empty_section(SectionName::TData)
            || merged_file_layout.contains_non_empty_section(SectionName::Data)
        {
            SECTION_ALIGN_DATA
        } else {
            SEGMENT_ALIGN_PAGE_SIZE
        };

        let actual_section_offset_toc = writer.reserve(section_info_toc.size, section_align_data);
        debug_assert_eq!(
            actual_section_offset_toc,
            section_info_toc.offset_in_merged_file
        );
    }

    writer.reserve_symtab();
    writer
        .reserve_strtab()
        .map_err(|_| LinkerError::new("Failed to reserve .strtab section"))?;
    writer
        .reserve_shstrtab()
        .map_err(|_| LinkerError::new("Failed to reserve .shstrtab section"))?;
    writer.reserve_section_headers();

    // -------------------------------------------------------------------------
    // Phase 6: write file header binary data
    // -------------------------------------------------------------------------

    let flags = match arch {
        Machine::RiscV => EF_RISCV_RVC | EF_RISCV_FLOAT_ABI_DOUBLE, // RVC, double-float ABI
        Machine::LoongArch => EF_LARCH_OBJABI_V1 | EF_LARCH_ABI_DOUBLE_FLOAT, // DOUBLE-FLOAT, OBJ-v1
        Machine::PowerPC64 => object::elf::FileFlags(0x2),                    // abiv2
        _ => object::elf::FileFlags(0),
    };

    // Write ELF header
    writer
        .write_file_header(&FileHeader {
            os_abi: ELFOSABI_NONE,
            abi_version: 0,
            e_type: ET_EXEC,
            e_machine: arch.into(),
            e_entry: entry_point,
            e_flags: flags,
        })
        .expect("failed to write ELF file header");

    // -------------------------------------------------------------------------
    // Phase 7: write program headers binary data
    // -------------------------------------------------------------------------

    // Write padding between ELF header and program headers
    writer.write_align_program_headers();

    // Write PHDR segment header
    //
    // Some compilers (e.g. GCC) generate an optional `.note.gnu.build-id` section,
    // which may be included in the PHDR segment. This linker does not generate
    // that section, so the PHDR segment contains only the ELF header and program headers.
    //
    // P.S.: using the command `readelf -n FILE` to show the notes.

    let segment_phdr_offset = ELF_HEADER_SIZE;
    let segment_phdr_size =
        PROGRAM_HEADER_ENTRY_SIZE * merged_file_layout.program_header_count as u64;
    let segment_phdr_virtual_address = LOAD_ADDR_BASE + ELF_HEADER_SIZE;

    writer.write_program_header(&ProgramHeader {
        p_type: PT_PHDR,
        p_flags: PF_R,
        p_offset: segment_phdr_offset,
        p_vaddr: segment_phdr_virtual_address,
        p_paddr: segment_phdr_virtual_address,
        p_filesz: segment_phdr_size,
        p_memsz: segment_phdr_size,
        p_align: SEGMENT_ALIGN_PHDR,
    });

    // Common segment type (p_type) includes:
    // - object::elf::PT_NULL
    // - object::elf::PT_PHDR
    // - object::elf::PT_LOAD
    // - object::elf::PT_TLS

    // Write metadata segment header
    // The metadata segment contains the ELF header and program headers,
    // which are required for the loader to load the executable.
    let segment_metadata_offset = 0;
    let segment_metadata_size = ELF_HEADER_SIZE
        + PROGRAM_HEADER_ENTRY_SIZE * merged_file_layout.program_header_count as u64;
    let segment_metadata_virtual_address = LOAD_ADDR_BASE;

    writer.write_program_header(&ProgramHeader {
        p_type: PT_LOAD,
        p_flags: PF_R,
        p_offset: segment_metadata_offset,
        p_vaddr: segment_metadata_virtual_address,
        p_paddr: segment_metadata_virtual_address,
        p_filesz: segment_metadata_size,
        p_memsz: segment_metadata_size,
        p_align: SEGMENT_ALIGN_PAGE_SIZE,
    });

    // Write code segment header
    {
        let Some(section_info_text) = merged_file_layout
            .merged_section_infos
            .get(&SectionName::Text)
        else {
            unreachable!()
        };

        writer.write_program_header(&ProgramHeader {
            p_type: PT_LOAD,
            p_flags: PF_R | PF_X,
            p_offset: section_info_text.offset_in_merged_file,
            p_vaddr: section_info_text.virtual_address,
            p_paddr: section_info_text.virtual_address,
            p_filesz: section_info_text.size,
            p_memsz: section_info_text.size,
            p_align: SEGMENT_ALIGN_PAGE_SIZE,
        });
    }

    // Write read-only data segment header
    if merged_file_layout.contains_read_only_data {
        let merged_section_infos = &merged_file_layout.merged_section_infos;

        let Some(first_read_only_section) = merged_section_infos
            .get(&SectionName::ROData)
            .or_else(|| merged_section_infos.get(&SectionName::GOT))
        else {
            unreachable!()
        };

        let segment_read_only_data_offset = first_read_only_section.offset_in_merged_file;
        let segment_read_only_data_virtual_address = first_read_only_section.virtual_address;

        let section_rodata_size = merged_section_infos
            .get(&SectionName::ROData)
            .map_or(0, |s| s.size);
        let section_got_size = merged_section_infos
            .get(&SectionName::GOT)
            .map_or(0, |s| s.size);

        let segment_read_only_data_file_size =
            align_up(section_rodata_size, SECTION_ALIGN_DATA) + section_got_size;

        writer.write_program_header(&ProgramHeader {
            p_type: PT_LOAD,
            p_flags: PF_R,
            p_offset: segment_read_only_data_offset,
            p_vaddr: segment_read_only_data_virtual_address,
            p_paddr: segment_read_only_data_virtual_address,
            p_filesz: segment_read_only_data_file_size,
            p_memsz: segment_read_only_data_file_size,
            p_align: SEGMENT_ALIGN_PAGE_SIZE,
        });
    }

    // Write writable data segment header
    if merged_file_layout.contains_writable_data {
        let merged_section_infos = &merged_file_layout.merged_section_infos;

        let Some(first_writable_section) = merged_section_infos
            .get(&SectionName::TData)
            .or_else(|| merged_section_infos.get(&SectionName::TBSS))
            .or_else(|| merged_section_infos.get(&SectionName::Data))
            .or_else(|| merged_section_infos.get(&SectionName::TOC))
            .or_else(|| merged_section_infos.get(&SectionName::BSS))
        else {
            unreachable!()
        };

        let segment_writable_data_offset = first_writable_section.offset_in_merged_file;
        let segment_writable_data_virtual_address = first_writable_section.virtual_address;

        let section_tdata_size = merged_section_infos
            .get(&SectionName::TData)
            .map_or(0, |s| s.size);
        let section_tbss_size = merged_section_infos
            .get(&SectionName::TBSS)
            .map_or(0, |s| s.size);
        let section_data_size = merged_section_infos
            .get(&SectionName::Data)
            .map_or(0, |s| s.size);
        let section_toc_size = merged_section_infos
            .get(&SectionName::TOC)
            .map_or(0, |s| s.size);
        let section_bss_size = merged_section_infos
            .get(&SectionName::BSS)
            .map_or(0, |s| s.size);

        let segment_writable_data_file_size = align_up(section_tdata_size, SECTION_ALIGN_DATA)
            + align_up(section_data_size, SECTION_ALIGN_DATA)
            + section_toc_size;

        let segment_writable_data_memory_size = align_up(section_tdata_size, SECTION_ALIGN_DATA)
            + align_up(section_tbss_size, SECTION_ALIGN_DATA)
            + align_up(section_data_size, SECTION_ALIGN_DATA)
            + align_up(section_toc_size, SECTION_ALIGN_DATA)
            + section_bss_size;

        writer.write_program_header(&ProgramHeader {
            p_type: PT_LOAD,
            p_flags: PF_R | PF_W, // writable data
            p_offset: segment_writable_data_offset,
            p_vaddr: segment_writable_data_virtual_address,
            p_paddr: segment_writable_data_virtual_address,
            p_filesz: segment_writable_data_file_size,
            p_memsz: segment_writable_data_memory_size,
            p_align: SEGMENT_ALIGN_PAGE_SIZE,
        });
    }

    // Write TLS segment header if there is TLS data
    if merged_file_layout.contains_tls_data {
        let merged_section_infos = &merged_file_layout.merged_section_infos;

        let Some(first_tls_section) = merged_section_infos
            .get(&SectionName::TData)
            .or_else(|| merged_section_infos.get(&SectionName::TBSS))
        else {
            unreachable!()
        };

        let segment_tls_data_offset = first_tls_section.offset_in_merged_file;
        let segment_tls_data_virtual_address = first_tls_section.virtual_address;

        let section_tdata_size = merged_section_infos
            .get(&SectionName::TData)
            .map_or(0, |s| s.size);
        let section_tbss_size = merged_section_infos
            .get(&SectionName::TBSS)
            .map_or(0, |s| s.size);

        let segment_tls_file_size = section_tdata_size;
        let segment_tls_memory_size =
            align_up(section_tdata_size, SECTION_ALIGN_DATA) + section_tbss_size;

        writer.write_program_header(&ProgramHeader {
            p_type: PT_TLS,
            p_flags: PF_R,
            p_offset: segment_tls_data_offset,
            p_vaddr: segment_tls_data_virtual_address,
            p_paddr: segment_tls_data_virtual_address,
            p_filesz: segment_tls_file_size,
            p_memsz: segment_tls_memory_size,
            p_align: SEGMENT_ALIGN_TLS,
        });
    }

    // -------------------------------------------------------------------------
    // Phase 8: write sections binary data
    // -------------------------------------------------------------------------

    // Write .text section data
    {
        writer.write_align(SEGMENT_ALIGN_PAGE_SIZE);
        for relocated_module in relocated_modules {
            if let Some(relocated_section) = relocated_module.sections.get(&SectionName::Text)
                && relocated_section.size > 0
            {
                writer.write_align(SECTION_ALIGN_TEXT);
                match &relocated_section.binary {
                    RelocatedSectionBinary::Referenced(data) => {
                        writer.write(data);
                    }
                    RelocatedSectionBinary::Owned(data) => {
                        writer.write(data);
                    }
                    RelocatedSectionBinary::None => {
                        //
                    }
                }
            }
        }
    }

    // Write .rodata section data
    if merged_file_layout.contains_non_empty_section(SectionName::ROData) {
        writer.write_align(SEGMENT_ALIGN_PAGE_SIZE);
        for relocated_module in relocated_modules {
            if let Some(relocated_section) = relocated_module.sections.get(&SectionName::ROData)
                && relocated_section.size > 0
            {
                writer.write_align(SECTION_ALIGN_DATA);
                match &relocated_section.binary {
                    RelocatedSectionBinary::Referenced(data) => {
                        writer.write(data);
                    }
                    RelocatedSectionBinary::Owned(data) => {
                        writer.write(data);
                    }
                    RelocatedSectionBinary::None => {
                        //
                    }
                }
            }
        }
    }

    // Write .got section data
    if merged_file_layout.contains_non_empty_section(SectionName::GOT) {
        // The .got section must be aligned to `DATA_ALIGN` instead of `PAGE_SIZE`
        // if it is merged with the .rodata section, because they are in the same read-only data segment.
        if !merged_file_layout.contains_non_empty_section(SectionName::ROData) {
            writer.write_align(SEGMENT_ALIGN_PAGE_SIZE);
        }

        for relocated_module in relocated_modules {
            if let Some(relocated_section) = relocated_module.sections.get(&SectionName::GOT)
                && relocated_section.size > 0
            {
                writer.write_align(SECTION_ALIGN_DATA);
                match &relocated_section.binary {
                    RelocatedSectionBinary::Referenced(data) => {
                        writer.write(data);
                    }
                    RelocatedSectionBinary::Owned(data) => {
                        writer.write(data);
                    }
                    RelocatedSectionBinary::None => {
                        //
                    }
                }
            }
        }
    }

    // Write .tdata and .data section data
    //
    // Note that there is no need to write .tbss and .bss section data,
    // because they are empty in the file.
    //
    // write .tdata section data
    if merged_file_layout.contains_non_empty_section(SectionName::TData) {
        writer.write_align(SEGMENT_ALIGN_PAGE_SIZE);

        for relocated_module in relocated_modules {
            if let Some(relocated_section) = relocated_module.sections.get(&SectionName::TData)
                && relocated_section.size > 0
            {
                writer.write_align(SECTION_ALIGN_DATA);
                match &relocated_section.binary {
                    RelocatedSectionBinary::Referenced(data) => {
                        writer.write(data);
                    }
                    RelocatedSectionBinary::Owned(data) => {
                        writer.write(data);
                    }
                    RelocatedSectionBinary::None => {
                        //
                    }
                }
            }
        }
    }

    // write .data section data
    if merged_file_layout.contains_non_empty_section(SectionName::Data) {
        // The .data section must be aligned to `DATA_ALIGN` instead of `PAGE_SIZE` if
        // it is merged with the .tdata section, because they are in the same writable data segment.
        if !merged_file_layout.contains_non_empty_section(SectionName::TData) {
            writer.write_align(SEGMENT_ALIGN_PAGE_SIZE);
        }

        for relocated_module in relocated_modules {
            if let Some(relocated_section) = relocated_module.sections.get(&SectionName::Data)
                && relocated_section.size > 0
            {
                writer.write_align(SECTION_ALIGN_DATA);
                match &relocated_section.binary {
                    RelocatedSectionBinary::Referenced(data) => {
                        writer.write(data);
                    }
                    RelocatedSectionBinary::Owned(data) => {
                        writer.write(data);
                    }
                    RelocatedSectionBinary::None => {
                        //
                    }
                }
            }
        }
    }

    // write .toc section data
    if merged_file_layout.contains_non_empty_section(SectionName::TOC) {
        writer.write_align(SECTION_ALIGN_DATA);
        for relocated_module in relocated_modules {
            if let Some(relocated_section) = relocated_module.sections.get(&SectionName::TOC)
                && relocated_section.size > 0
            {
                match &relocated_section.binary {
                    RelocatedSectionBinary::Referenced(data) => writer.write(data),
                    RelocatedSectionBinary::Owned(data) => writer.write(data),
                    RelocatedSectionBinary::None => {}
                }
            }
        }
    }

    // -------------------------------------------------------------------------
    // Phase 9: write symbol table
    // -------------------------------------------------------------------------

    // Write symbol table
    writer.write_align(SECTION_ALIGN_SYMTAB);

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
    if let Some(section_info) = merged_file_layout.get_non_empty_section_info(SectionName::Text) {
        writer.write_section_header(&SectionHeader {
            sh_name: writer.section_name_offset(section_name_text_opt),
            sh_type: SHT_PROGBITS,
            sh_flags: SHF_ALLOC | SHF_EXECINSTR,
            sh_addr: section_info.virtual_address,
            sh_offset: section_info.offset_in_merged_file,
            sh_size: section_info.size,

            // depends on the section type, for SHT_PROGBITS it is usually 0
            // for section `.rela.text`, it is the index of the section `.symtab` that holds the symbols.
            // for section `.symtab`, it is the index of the associated string table section (`.strtab`),
            sh_link: 0,

            // depends on the section type, for SHT_PROGBITS it is usually 0
            // for section `.rela.text`, it is the index of the section to which the relocations apply (e.g. `.text`)
            // for section `.symtab`, it is the index of the first non-local symbol (i.e. the number of local symbols)
            sh_info: 0,

            // code sections are usually aligned to 16 bytes
            sh_addralign: SECTION_ALIGN_TEXT,
            sh_entsize: 0,
        });
    }

    // Write section header: .rodata
    if let Some(section_info) = merged_file_layout.get_non_empty_section_info(SectionName::ROData) {
        writer.write_section_header(&SectionHeader {
            sh_name: writer.section_name_offset(section_name_rodata_opt),
            sh_type: SHT_PROGBITS,
            sh_flags: SHF_ALLOC,
            sh_addr: section_info.virtual_address,
            sh_offset: section_info.offset_in_merged_file,
            sh_size: section_info.size,
            sh_link: 0,
            sh_info: 0,
            // read-only data sections are usually aligned to 8 or 4 bytes
            sh_addralign: SECTION_ALIGN_DATA,
            sh_entsize: 0,
        });
    }

    // Write section header: .got
    if let Some(section_info) = merged_file_layout.get_non_empty_section_info(SectionName::GOT) {
        writer.write_section_header(&SectionHeader {
            sh_name: writer.section_name_offset(section_name_got_opt),
            sh_type: SHT_PROGBITS,
            sh_flags: SHF_ALLOC,
            sh_addr: section_info.virtual_address,
            sh_offset: section_info.offset_in_merged_file,
            sh_size: section_info.size,
            sh_link: 0,
            sh_info: 0,
            sh_addralign: SECTION_ALIGN_DATA,
            sh_entsize: 0,
        });
    }

    // Write section header: .tdata
    if let Some(section_info) = merged_file_layout.get_non_empty_section_info(SectionName::TData) {
        writer.write_section_header(&SectionHeader {
            sh_name: writer.section_name_offset(section_name_tdata_opt),
            sh_type: SHT_PROGBITS,
            sh_flags: (SHF_ALLOC | SHF_WRITE | SHF_TLS),
            sh_addr: section_info.virtual_address,
            sh_offset: section_info.offset_in_merged_file,
            sh_size: section_info.size,
            sh_link: 0,
            sh_info: 0,
            // data sections are usually aligned to 8 or 4 bytes
            sh_addralign: SECTION_ALIGN_DATA,
            sh_entsize: 0,
        });
    }

    // Write section header: .tbss
    if let Some(section_info) = merged_file_layout.get_non_empty_section_info(SectionName::TBSS) {
        writer.write_section_header(&SectionHeader {
            sh_name: writer.section_name_offset(section_name_tbss_opt),
            sh_type: SHT_NOBITS,
            sh_flags: (SHF_ALLOC | SHF_WRITE | SHF_TLS),
            sh_addr: section_info.virtual_address,
            sh_offset: section_info.offset_in_merged_file,
            // .bss has no data in the file
            sh_size: 0,
            sh_link: 0,
            sh_info: 0,
            // .bss sections are usually aligned to 8 or 4 bytes
            sh_addralign: SECTION_ALIGN_DATA,
            sh_entsize: 0,
        });
    }

    // Write section header: .data
    if let Some(section_info) = merged_file_layout.get_non_empty_section_info(SectionName::Data) {
        writer.write_section_header(&SectionHeader {
            sh_name: writer.section_name_offset(section_name_data_opt),
            sh_type: SHT_PROGBITS,
            sh_flags: (SHF_ALLOC | SHF_WRITE),
            sh_addr: section_info.virtual_address,
            sh_offset: section_info.offset_in_merged_file,
            sh_size: section_info.size,
            sh_link: 0,
            sh_info: 0,
            // data sections are usually aligned to 8 or 4 bytes
            sh_addralign: SECTION_ALIGN_DATA,
            sh_entsize: 0,
        });
    }

    if let Some(section_info) = merged_file_layout.get_non_empty_section_info(SectionName::TOC) {
        writer.write_section_header(&SectionHeader {
            sh_name: writer.section_name_offset(section_name_toc_opt),
            sh_type: SHT_PROGBITS,
            sh_flags: (SHF_ALLOC | SHF_WRITE),
            sh_addr: section_info.virtual_address,
            sh_offset: section_info.offset_in_merged_file,
            sh_size: section_info.size,
            sh_link: 0,
            sh_info: 0,
            sh_addralign: SECTION_ALIGN_DATA,
            sh_entsize: 0,
        });
    }

    // Write section header: .bss
    if let Some(section_info) = merged_file_layout.get_non_empty_section_info(SectionName::BSS) {
        writer.write_section_header(&SectionHeader {
            sh_name: writer.section_name_offset(section_name_bss_opt),
            sh_type: SHT_NOBITS,
            sh_flags: (SHF_ALLOC | SHF_WRITE),
            sh_addr: section_info.virtual_address,
            sh_offset: section_info.offset_in_merged_file,
            // .bss has no data in the file
            sh_size: 0,
            sh_link: 0,
            sh_info: 0,
            // .bss sections are usually aligned to 8 or 4 bytes
            sh_addralign: SECTION_ALIGN_DATA,
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

fn align_up(val: u64, align: u64) -> u64 {
    (val + align - 1) & !(align - 1)
}

#[cfg(test)]
mod tests {

    use std::{
        collections::HashMap,
        fmt::Display,
        os::unix::fs::PermissionsExt,
        path::{Path, PathBuf},
        process::Command,
        time::Duration,
    };

    use object::{
        Endianness,
        write::{StreamingBuffer, WritableBuffer},
    };

    use crate::elf::{
        external_symbol_resolver::{ResolvedAsset, find_entry_point, resolve},
        merger::{
            GlobalSymbolMapEntry, GlobalSymbolValue, MergedAsset, MergedFileLayout, SectionName,
            merge,
        },
        module::{Machine, RelocatableModule, get_load_address_base},
        reader::read_relocatable_module,
        relocator::relocate,
        writer::write_executable,
    };

    #[derive(Debug, PartialEq, Clone, Copy)]
    enum SourceType {
        Assembly,

        #[allow(dead_code)]
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
        Machine::S390,
        Machine::PowerPC64,
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

    fn get_example_file_binaries(
        source_type: SourceType,
        arch: Machine,
        file_names: &[&str],
    ) -> Vec<Vec<u8>> {
        file_names
            .iter()
            .map(|file_name| get_example_file_binary(source_type, arch, file_name))
            .collect()
    }

    fn get_example_file_module<'a>(name: &str, file_binary: &'a [u8]) -> RelocatableModule<'a> {
        read_relocatable_module(name, file_binary).unwrap()
    }

    fn get_example_file_modules<'a>(
        names: &[&str],
        file_binaries: &[&'a [u8]],
    ) -> Vec<RelocatableModule<'a>> {
        names
            .iter()
            .zip(file_binaries.iter())
            .map(|(name, file_binary)| get_example_file_module(name, file_binary))
            .collect()
    }

    fn add_additional_linker_generated_symbols(
        arch: Machine,
        linker_generated_symbols: &mut HashMap<String, GlobalSymbolMapEntry>,
        merged_file_layout: &MergedFileLayout,
    ) {
        match arch {
            Machine::RiscV => {
                linker_generated_symbols.insert(
                    "__global_pointer$".to_string(),
                    GlobalSymbolMapEntry::new(
                        GlobalSymbolValue::from_defined(SectionName::Text, 0x1000),
                        false,
                    ),
                );
            }
            Machine::PowerPC64 => {
                let toc_address = merged_file_layout
                    .get_non_empty_section_info(SectionName::TOC)
                    .map(|section| section.virtual_address)
                    .unwrap_or_else(|| get_load_address_base(arch));

                linker_generated_symbols.insert(
                    ".TOC.".to_string(),
                    GlobalSymbolMapEntry::new(
                        GlobalSymbolValue::Absolute(toc_address + 0x8000),
                        false,
                    ),
                );
            }
            _ => {
                // No additional linker-generated symbols for other architectures
            }
        }
    }

    fn link_example_files(
        file_names: &[&str],
        source_type: SourceType,
        arch: Machine,
        endian: Endianness,
        output_buffer: &mut dyn WritableBuffer,
    ) {
        let file_binaries = get_example_file_binaries(source_type, arch, file_names);
        let file_binaries_ref: Vec<&[u8]> = file_binaries.iter().map(|b| b.as_slice()).collect();
        let modules: Vec<RelocatableModule> =
            get_example_file_modules(file_names, &file_binaries_ref);

        let MergedAsset {
            fragment_modules,
            mut linker_generated_symbols,
            merged_file_layout,
        } = merge(modules, arch).unwrap();

        add_additional_linker_generated_symbols(
            arch,
            &mut linker_generated_symbols,
            &merged_file_layout,
        );

        let ResolvedAsset {
            resolved_modules,
            global_symbols,
        } = resolve(fragment_modules, &linker_generated_symbols).unwrap();

        let entry_point = find_entry_point(&global_symbols).unwrap();

        let relocated_modules = relocate(&merged_file_layout, resolved_modules, arch).unwrap();

        write_executable(
            &relocated_modules,
            &merged_file_layout,
            entry_point,
            arch,
            endian,
            output_buffer,
        )
        .unwrap();
    }

    fn generate_example_executable(
        file_names: &[&str],
        source_type: SourceType,
        arch: Machine,
        output_base_name: &str,
    ) -> PathBuf {
        let output_file_name = format!(
            "test_{}_{}_{}.elf",
            source_type,
            get_arch_dir_name(arch),
            output_base_name
        );

        let endian = match arch {
            Machine::S390 => Endianness::Big,
            _ => Endianness::Little,
        };

        let tmp_dir = std::env::temp_dir();
        let path = tmp_dir.join(output_file_name);
        let mut file = std::fs::File::create(&path).unwrap();
        let mut buffer = StreamingBuffer::new(&mut file);
        link_example_files(file_names, source_type, arch, endian, &mut buffer);
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("failed to set permissions");
        path
    }

    fn execute_and_assert(
        arch: Machine,
        file_path: &PathBuf,
        expected_exit_code: i32,
        expected_output: &str,
    ) {
        // Sleep 5ms to avoid `Os { code: 26, kind: ExecutableFileBusy, message: "Text file busy" }` error.
        const RETRY_INTERVAL: Duration = Duration::from_millis(5);
        std::thread::sleep(RETRY_INTERVAL);

        // Use QEMU to run the executable on non-native architectures.
        //
        // For example, to run the aarch64 executable on x86_64, you can use:
        //
        // qemu-aarch64 -L $(aarch64-linux-gnu-gcc -print-sysroot) ./executable_file

        let current_arch = std::env::consts::ARCH;
        let output_result = match arch {
            Machine::X86_64 => {
                if current_arch == "x86_64" {
                    Command::new(file_path).output()
                } else {
                    Command::new("qemu-x86_64")
                        .arg("-L")
                        .arg("$(x86_64-linux-gnu-gcc -print-sysroot)")
                        .arg(file_path)
                        .output()
                }
            }
            Machine::AArch64 => {
                if current_arch == "aarch64" {
                    Command::new(file_path).output()
                } else {
                    Command::new("qemu-aarch64")
                        .arg("-L")
                        .arg("$(aarch64-linux-gnu-gcc -print-sysroot)")
                        .arg(file_path)
                        .output()
                }
            }
            Machine::RiscV => {
                if current_arch == "riscv64" {
                    Command::new(file_path).output()
                } else {
                    Command::new("qemu-riscv64")
                        .arg("-L")
                        .arg("$(riscv64-linux-gnu-gcc -print-sysroot)")
                        .arg(file_path)
                        .output()
                }
            }
            Machine::LoongArch => {
                if current_arch == "loongarch64" {
                    Command::new(file_path).output()
                } else {
                    Command::new("qemu-loongarch64")
                        .arg("-L")
                        .arg("$(loongarch64-linux-gnu-gcc -print-sysroot)")
                        .arg(file_path)
                        .output()
                }
            }
            Machine::PowerPC64 => {
                if current_arch == "powerpc64" {
                    Command::new(file_path).output()
                } else {
                    Command::new("qemu-ppc64le")
                        .arg("-L")
                        .arg("$(powerpc64le-linux-gnu-gcc -print-sysroot)")
                        .arg(file_path)
                        .output()
                }
            }
            Machine::S390 => {
                if current_arch == "s390x" {
                    Command::new(file_path).output()
                } else {
                    Command::new("qemu-s390x")
                        .arg("-L")
                        .arg("$(s390x-linux-gnu-gcc -print-sysroot)")
                        .arg(file_path)
                        .output()
                }
            }
            Machine::Other(_) => unimplemented!(),
        };

        let output = match output_result {
            Ok(output) => output,
            Err(e) => panic!("Failed to execute the executable: {}. arch: {}", e, arch),
        };

        let exit_code_opt = output.status.code();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        let exit_code = match exit_code_opt {
            Some(code) => code,
            None => {
                panic!(
                    "Executable terminated by signal. stdout: {}, stderr: {}, arch: {}",
                    stdout, stderr, arch
                );
            }
        };

        assert_eq!(
            exit_code, expected_exit_code,
            "Executable returned unexpected exit code. expected: {}, actual: {}, arch: {}",
            expected_exit_code, exit_code, arch
        );

        assert_eq!(
            stdout, expected_output,
            "Executable returned unexpected output. expected: {}, actual: {}, arch: {}",
            expected_output, stdout, arch
        );
    }

    fn delete_temporary_file(path: &Path) {
        if path.exists() {
            std::fs::remove_file(path)
                .unwrap_or_else(|_| panic!("failed to delete temporary file: {}", path.display()));
        }
    }

    #[test]
    fn test_write_asm_minimal() {
        for arch in IMPLEMENTED_ARCHS {
            let file =
                generate_example_executable(&["minimal.o"], SourceType::Assembly, arch, "minimal");
            execute_and_assert(arch, &file, 42, "");
            delete_temporary_file(&file);
        }
    }

    #[test]
    fn test_write_asm_function() {
        for arch in IMPLEMENTED_ARCHS {
            let file = generate_example_executable(
                &["function.o"],
                SourceType::Assembly,
                arch,
                "function",
            );
            execute_and_assert(arch, &file, 0, "Hello, world!\n");
            delete_temporary_file(&file);
        }
    }

    #[test]
    fn test_write_asm_data() {
        for arch in IMPLEMENTED_ARCHS {
            let file = generate_example_executable(&["data.o"], SourceType::Assembly, arch, "data");
            execute_and_assert(arch, &file, 24, "");
            delete_temporary_file(&file);
        }
    }

    #[test]
    fn test_write_asm_relocate_within_data() {
        for arch in IMPLEMENTED_ARCHS {
            let file = generate_example_executable(
                &["relocate-within-data.o"],
                SourceType::Assembly,
                arch,
                "relocate-within-data",
            );
            execute_and_assert(arch, &file, 24, "");
            delete_temporary_file(&file);
        }
    }

    #[test]
    fn test_write_asm_symbol() {
        for arch in IMPLEMENTED_ARCHS {
            let file = generate_example_executable(
                &["symbol-import.o", "symbol-export.o"],
                SourceType::Assembly,
                arch,
                "symbol",
            );
            execute_and_assert(arch, &file, 24, "");
            delete_temporary_file(&file);
        }
    }

    #[test]
    fn test_write_asm_override() {
        for arch in IMPLEMENTED_ARCHS {
            let file = generate_example_executable(
                &["override-strong.o", "override-weak.o"],
                SourceType::Assembly,
                arch,
                "override",
            );
            execute_and_assert(arch, &file, 53, "");
            delete_temporary_file(&file);
        }
    }

    #[test]
    fn test_write_gcc_minimal() {
        for arch in IMPLEMENTED_ARCHS {
            let file =
                generate_example_executable(&["minimal.o"], SourceType::GCC, arch, "minimal");
            execute_and_assert(arch, &file, 42, "");
            delete_temporary_file(&file);
        }
    }

    #[test]
    fn test_write_gcc_function() {
        for arch in IMPLEMENTED_ARCHS {
            let file =
                generate_example_executable(&["function.o"], SourceType::GCC, arch, "function");
            execute_and_assert(arch, &file, 0, "Hello, world!\n");
            delete_temporary_file(&file);
        }
    }

    #[test]
    fn test_write_gcc_data() {
        for arch in IMPLEMENTED_ARCHS {
            let file = generate_example_executable(&["data.o"], SourceType::GCC, arch, "data");
            execute_and_assert(arch, &file, 24, "");
            delete_temporary_file(&file);
        }
    }

    #[test]
    fn test_write_gcc_relocate_within_data() {
        for arch in IMPLEMENTED_ARCHS {
            let file = generate_example_executable(
                &["relocate-within-data.o"],
                SourceType::GCC,
                arch,
                "relocate-within-data",
            );
            execute_and_assert(arch, &file, 24, "");
            delete_temporary_file(&file);
        }
    }

    #[test]
    fn test_write_gcc_symbol() {
        for arch in IMPLEMENTED_ARCHS {
            let file = generate_example_executable(
                &["symbol-import.o", "symbol-export.o"],
                SourceType::GCC,
                arch,
                "symbol",
            );
            execute_and_assert(arch, &file, 24, "");
            delete_temporary_file(&file);
        }
    }

    #[test]
    fn test_write_gcc_override() {
        for arch in IMPLEMENTED_ARCHS {
            let file = generate_example_executable(
                &["override-strong.o", "override-weak.o"],
                SourceType::GCC,
                arch,
                "override",
            );
            execute_and_assert(arch, &file, 53, "");
            delete_temporary_file(&file);
        }
    }
}
