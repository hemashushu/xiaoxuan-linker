// Copyright (c) 2026 Hemashushu <hippospark@gmail.com>, All rights reserved.
//
// This Source Code Form is subject to the terms of
// the Mozilla Public License version 2.0 and additional exceptions.
// For more details, see the LICENSE, LICENSE.additional, and CONTRIBUTING files.

use object::write::WritableBuffer;
use sha2::Digest;

use crate::{
    error::LinkerError,
    macho::{
        merger::{MergedFileLayout, SectionName},
        module::{CpuType, RelocatableModule, RelocationType},
        relocator::{RelocatedModule, RelocatedSectionBinary},
    },
};

/// Writes the final Mach-O executable file.
pub fn write_executable(
    relocated_modules: &[RelocatedModule],
    merged_file_layout: &MergedFileLayout,
    entry_point: u64,
    _cpu_type: CpuType,
    output_buffer: &mut dyn WritableBuffer,
) -> Result<(), LinkerError> {
    let mut file_buf = Vec::new();

    // 1. Gather sections and binary data
    let text_info = merged_file_layout.get_non_empty_section_info(SectionName::Text);
    let const_info = merged_file_layout.get_non_empty_section_info(SectionName::Const);
    let data_const_info = merged_file_layout.get_non_empty_section_info(SectionName::DataConst);
    let data_info = merged_file_layout.get_non_empty_section_info(SectionName::Data);
    let bss_info = merged_file_layout.get_non_empty_section_info(SectionName::BSS);

    let mut text_data = Vec::new();
    let mut const_data = Vec::new();
    let mut data_const_data = Vec::new();
    let mut data_data = Vec::new();

    for module in relocated_modules {
        if let Some(sect) = module.sections.get(&SectionName::Text) {
            append_section_data(&mut text_data, &sect.binary);
        }
        if let Some(sect) = module.sections.get(&SectionName::Const) {
            append_section_data(&mut const_data, &sect.binary);
        }
        if let Some(sect) = module.sections.get(&SectionName::DataConst) {
            append_section_data(&mut data_const_data, &sect.binary);
        }
        if let Some(sect) = module.sections.get(&SectionName::Data) {
            append_section_data(&mut data_data, &sect.binary);
        }
    }

    // Prepare LINKEDIT contents and calculate offsets
    let dylib_names = &merged_file_layout.dylib_symbol_names;
    let nsyms = dylib_names.len() as u32;

    let linkedit_fileoff = merged_file_layout.linkedit_segment_fileoff as usize;
    let linkedit_vmaddr = merged_file_layout.linkedit_segment_vmaddr;

    let indirectsymoff = if nsyms > 0 {
        linkedit_fileoff as u32
    } else {
        0
    };
    let indirectsyms_size = (nsyms * 4) as usize;

    let symoff = if nsyms > 0 {
        (linkedit_fileoff + indirectsyms_size) as u32
    } else {
        linkedit_fileoff as u32
    };
    let syms_size = (nsyms * 16) as usize;

    let stroff = if nsyms > 0 {
        (symoff as usize + syms_size) as u32
    } else {
        symoff
    };

    let mut strtab_bytes = vec![0u8]; // string table starts with null byte
    let mut nlist_entries = Vec::new();

    for sym_name in dylib_names {
        let n_strx = strtab_bytes.len() as u32;
        strtab_bytes.extend_from_slice(sym_name.as_bytes());
        strtab_bytes.push(0);

        let mut nlist = Vec::with_capacity(16);
        nlist.extend_from_slice(&n_strx.to_le_bytes()); // n_strx
        nlist.push(0x01); // n_type = N_UNDF | N_EXT
        nlist.push(0); // n_sect
        nlist.extend_from_slice(&0x0100u16.to_le_bytes()); // n_desc = 1 << 8 (1st dylib: libSystem)
        nlist.extend_from_slice(&0u64.to_le_bytes()); // n_value
        nlist_entries.extend_from_slice(&nlist);
    }

    let strsize = if nsyms > 0 {
        strtab_bytes.len() as u32
    } else {
        0
    };

    // Collect local rebase relocations from ARM64_RELOC_UNSIGNED entries
    let mut locrel_bytes = Vec::new();

    for module in relocated_modules {
        for (sect_name, sect) in &module.sections {
            if matches!(sect_name, SectionName::DataConst | SectionName::Data) {
                if let Some(info) = merged_file_layout.get_non_empty_section_info(*sect_name) {
                    let binary_data = match &sect.binary {
                        RelocatedSectionBinary::Owned(d) => d.as_slice(),
                        RelocatedSectionBinary::Referenced(d) => *d,
                        RelocatedSectionBinary::None => &[],
                    };

                    for offset in (0..binary_data.len()).step_by(8) {
                        if offset + 8 <= binary_data.len() {
                            let val = u64::from_le_bytes(
                                binary_data[offset..offset + 8].try_into().unwrap(),
                            );
                            if val >= 0x100000000 {
                                let r_address = (info.virtual_address + offset as u64
                                    - merged_file_layout.text_segment_vmaddr)
                                    as u32;
                                let r_info = 0x06000000u32; // ARM64_RELOC_UNSIGNED (length = 3, 64-bit)
                                locrel_bytes.extend_from_slice(&r_address.to_le_bytes());
                                locrel_bytes.extend_from_slice(&r_info.to_le_bytes());
                            }
                        }
                    }
                }
            }
        }
    }

    let nlocrel = (locrel_bytes.len() / 8) as u32;
    let locrel_size = locrel_bytes.len();
    let locreloff = if nlocrel > 0 {
        ((stroff as usize + strsize as usize) + 7) & !7
    } else {
        0
    };

    let sig_off = if nlocrel > 0 {
        ((locreloff + locrel_size) + 15) & !15
    } else if nsyms > 0 {
        ((stroff as usize + strsize as usize) + 15) & !15
    } else {
        linkedit_fileoff + 16
    } as u32;

    let page_size = 4096u32;
    let n_code_slots = ((sig_off + page_size - 1) / page_size) as usize;
    let cd_size = 44 + 6 + (n_code_slots * 32) as u32;
    let sig_size = 12 + 8 + cd_size;
    let linkedit_filesize = (sig_off as u64 + sig_size as u64) - linkedit_fileoff as u64;
    let linkedit_vmsize = (linkedit_filesize + 0x3fff) & !0x3fff;

    // 2. Count load commands and size
    let num_text_sections = 1 + if const_info.is_some() { 1 } else { 0 };
    let text_seg_cmd_size = 72 + 80 * num_text_sections;

    let num_data_sections = (if data_const_info.is_some() { 1 } else { 0 })
        + (if data_info.is_some() { 1 } else { 0 })
        + (if bss_info.is_some() { 1 } else { 0 });

    let has_data_seg = num_data_sections > 0;
    let data_seg_cmd_size = if has_data_seg {
        72 + 80 * num_data_sections
    } else {
        0
    };

    let mut ncmds = 10u32; // __PAGEZERO, __TEXT, __LINKEDIT, LC_MAIN, LC_LOAD_DYLINKER, LC_LOAD_DYLIB, LC_SYMTAB, LC_DYSYMTAB, LC_BUILD_VERSION, LC_CODE_SIGNATURE
    if has_data_seg {
        ncmds += 1;
    }

    let sizeofcmds = (72 // __PAGEZERO
        + text_seg_cmd_size
        + data_seg_cmd_size
        + 72 // __LINKEDIT
        + 24 // LC_MAIN
        + 32 // LC_LOAD_DYLINKER
        + 56 // LC_LOAD_DYLIB
        + 24 // LC_SYMTAB
        + 80 // LC_DYSYMTAB
        + 32 // LC_BUILD_VERSION
        + 16) as u32; // LC_CODE_SIGNATURE

    // 3. Write Mach-O 64-bit Header (32 bytes)
    file_buf.extend_from_slice(&0xfeedfacfu32.to_le_bytes()); // MH_MAGIC_64
    file_buf.extend_from_slice(&0x0100000cu32.to_le_bytes()); // CPU_TYPE_ARM64
    file_buf.extend_from_slice(&0x00000000u32.to_le_bytes()); // CPU_SUBTYPE_ARM64_ALL
    file_buf.extend_from_slice(&0x00000002u32.to_le_bytes()); // MH_EXECUTE
    file_buf.extend_from_slice(&ncmds.to_le_bytes()); // ncmds
    file_buf.extend_from_slice(&sizeofcmds.to_le_bytes()); // sizeofcmds
    file_buf.extend_from_slice(&0x00200085u32.to_le_bytes()); // flags: MH_NOUNDEFS | MH_DYLDLINK | MH_TWOLEVEL | MH_PIE
    file_buf.extend_from_slice(&0x00000000u32.to_le_bytes()); // reserved

    let nsyms = merged_file_layout.dylib_symbol_names.len() as u32;

    // 4. Write Load Commands
    // Cmd 0: __PAGEZERO
    write_segment_command(&mut file_buf, "__PAGEZERO", 0x0, 0x100000000, 0, 0, 0, 0, 0);

    // Cmd 1: __TEXT
    write_segment_command(
        &mut file_buf,
        "__TEXT",
        merged_file_layout.text_segment_vmaddr,
        merged_file_layout.text_segment_vmsize,
        0,
        merged_file_layout.text_segment_filesize,
        0x5, // READ | EXEC
        0x5,
        num_text_sections as u32,
    );

    // __TEXT sections
    if let Some(info) = text_info {
        write_section_header(
            &mut file_buf,
            "__text",
            "__TEXT",
            info.virtual_address,
            info.size,
            info.offset_in_merged_file as u32,
            2,          // 4-byte align
            0x80000400, // S_REGULAR | S_ATTR_PURE_INSTRUCTIONS | S_ATTR_SOME_INSTRUCTIONS
        );
    }

    if let Some(info) = const_info {
        write_section_header(
            &mut file_buf,
            "__const",
            "__TEXT",
            info.virtual_address,
            info.size,
            info.offset_in_merged_file as u32,
            3, // 8-byte align
            0x0,
        );
    }

    // Cmd 2: __DATA (if present)
    if has_data_seg {
        write_segment_command(
            &mut file_buf,
            "__DATA",
            merged_file_layout.data_segment_vmaddr,
            merged_file_layout.data_segment_vmsize,
            merged_file_layout.data_segment_fileoff,
            merged_file_layout.data_segment_filesize,
            0x3, // READ | WRITE
            0x3,
            num_data_sections as u32,
        );

        if let Some(info) = data_const_info {
            write_section_header(
                &mut file_buf,
                if nsyms > 0 { "__got" } else { "__const" },
                "__DATA",
                info.virtual_address,
                info.size,
                info.offset_in_merged_file as u32,
                3,
                if nsyms > 0 { 0x6 } else { 0x0 }, // S_NON_LAZY_SYMBOL_POINTERS or S_REGULAR
            );
        }

        if let Some(info) = data_info {
            write_section_header(
                &mut file_buf,
                "__data",
                "__DATA",
                info.virtual_address,
                info.size,
                info.offset_in_merged_file as u32,
                3,
                0x0,
            );
        }

        if let Some(info) = bss_info {
            write_section_header(
                &mut file_buf,
                "__bss",
                "__DATA",
                info.virtual_address,
                info.size,
                0,
                3,
                0x1, // S_ZEROFILL
            );
        }
    }

    // Cmd 3: __LINKEDIT
    write_segment_command(
        &mut file_buf,
        "__LINKEDIT",
        linkedit_vmaddr,
        linkedit_vmsize,
        linkedit_fileoff as u64,
        linkedit_filesize,
        0x1, // READ
        0x1,
        0,
    );

    // Cmd 4: LC_MAIN
    let entry_offset = (entry_point - merged_file_layout.text_segment_vmaddr) as u64;
    file_buf.extend_from_slice(&0x80000028u32.to_le_bytes()); // LC_MAIN
    file_buf.extend_from_slice(&24u32.to_le_bytes()); // cmdsize
    file_buf.extend_from_slice(&entry_offset.to_le_bytes()); // entryoff
    file_buf.extend_from_slice(&0u64.to_le_bytes()); // stacksize

    // Cmd 5: LC_LOAD_DYLINKER
    file_buf.extend_from_slice(&0x0eu32.to_le_bytes()); // LC_LOAD_DYLINKER
    file_buf.extend_from_slice(&32u32.to_le_bytes()); // cmdsize (4 + 4 + 4 + 20 = 32)
    file_buf.extend_from_slice(&12u32.to_le_bytes()); // name offset
    file_buf.extend_from_slice(b"/usr/lib/dyld\0\0\0\0\0\0\0"); // 14 + 6 = 20 bytes

    // Cmd 6: LC_LOAD_DYLIB
    file_buf.extend_from_slice(&0x0cu32.to_le_bytes()); // LC_LOAD_DYLIB
    file_buf.extend_from_slice(&56u32.to_le_bytes()); // cmdsize
    file_buf.extend_from_slice(&24u32.to_le_bytes()); // name offset
    file_buf.extend_from_slice(&2u32.to_le_bytes()); // timestamp
    file_buf.extend_from_slice(&0x00010000u32.to_le_bytes()); // current_version
    file_buf.extend_from_slice(&0x00010000u32.to_le_bytes()); // compatibility_version
    file_buf.extend_from_slice(b"/usr/lib/libSystem.B.dylib\0\0\0\0\0\0");

    // Cmd 7: LC_SYMTAB
    file_buf.extend_from_slice(&0x02u32.to_le_bytes()); // LC_SYMTAB
    file_buf.extend_from_slice(&24u32.to_le_bytes()); // cmdsize
    file_buf.extend_from_slice(&symoff.to_le_bytes());
    file_buf.extend_from_slice(&nsyms.to_le_bytes());
    file_buf.extend_from_slice(&stroff.to_le_bytes());
    file_buf.extend_from_slice(&strsize.to_le_bytes());

    // Cmd 8: LC_DYSYMTAB
    file_buf.extend_from_slice(&0x0bu32.to_le_bytes()); // LC_DYSYMTAB
    file_buf.extend_from_slice(&80u32.to_le_bytes()); // cmdsize
    file_buf.extend_from_slice(&0u32.to_le_bytes()); // ilocalsym
    file_buf.extend_from_slice(&0u32.to_le_bytes()); // nlocalsym
    file_buf.extend_from_slice(&0u32.to_le_bytes()); // iextdefsym
    file_buf.extend_from_slice(&0u32.to_le_bytes()); // nextdefsym
    file_buf.extend_from_slice(&0u32.to_le_bytes()); // iundefsym
    file_buf.extend_from_slice(&nsyms.to_le_bytes()); // nundefsym
    file_buf.extend_from_slice(&[0u8; 24]); // tocoff .. extrefsymoff (6 * 4 = 24)
    file_buf.extend_from_slice(&(indirectsymoff as u32).to_le_bytes()); // indirectsymoff
    file_buf.extend_from_slice(&nsyms.to_le_bytes()); // nindirectsyms
    file_buf.extend_from_slice(&0u32.to_le_bytes()); // extreloff
    file_buf.extend_from_slice(&0u32.to_le_bytes()); // nextrel
    file_buf.extend_from_slice(&(locreloff as u32).to_le_bytes()); // locreloff
    file_buf.extend_from_slice(&nlocrel.to_le_bytes()); // nlocrel

    // Cmd 9: LC_BUILD_VERSION
    file_buf.extend_from_slice(&0x32u32.to_le_bytes()); // LC_BUILD_VERSION
    file_buf.extend_from_slice(&32u32.to_le_bytes()); // cmdsize
    file_buf.extend_from_slice(&1u32.to_le_bytes()); // platform: PLATFORM_MACOS
    file_buf.extend_from_slice(&0x000e0000u32.to_le_bytes()); // minos: 14.0.0
    file_buf.extend_from_slice(&0x000e0000u32.to_le_bytes()); // sdk: 14.0.0
    file_buf.extend_from_slice(&1u32.to_le_bytes()); // ntools: 1
    file_buf.extend_from_slice(&3u32.to_le_bytes()); // tool: TOOL_LD
    file_buf.extend_from_slice(&0x00010000u32.to_le_bytes()); // version: 1.0.0

    // Cmd 9: LC_CODE_SIGNATURE
    file_buf.extend_from_slice(&0x1du32.to_le_bytes()); // LC_CODE_SIGNATURE
    file_buf.extend_from_slice(&16u32.to_le_bytes()); // cmdsize
    file_buf.extend_from_slice(&sig_off.to_le_bytes());
    file_buf.extend_from_slice(&sig_size.to_le_bytes());

    // 5. Pad header + load commands to text_section_offset
    let text_section_offset = text_info
        .map(|i| i.offset_in_merged_file as usize)
        .unwrap_or(1024);
    if file_buf.len() < text_section_offset {
        file_buf.resize(text_section_offset, 0);
    }

    // 6. Write __TEXT section data
    if let Some(info) = text_info {
        if file_buf.len() < info.offset_in_merged_file as usize {
            file_buf.resize(info.offset_in_merged_file as usize, 0);
        }
        file_buf.extend_from_slice(&text_data);
    }

    if let Some(info) = const_info {
        if file_buf.len() < info.offset_in_merged_file as usize {
            file_buf.resize(info.offset_in_merged_file as usize, 0);
        }
        file_buf.extend_from_slice(&const_data);
    }

    // 7. Pad to __DATA segment
    if has_data_seg {
        let data_seg_off = merged_file_layout.data_segment_fileoff as usize;
        if file_buf.len() < data_seg_off {
            file_buf.resize(data_seg_off, 0);
        }

        if let Some(info) = data_const_info {
            if file_buf.len() < info.offset_in_merged_file as usize {
                file_buf.resize(info.offset_in_merged_file as usize, 0);
            }
            file_buf.extend_from_slice(&data_const_data);
        }

        if let Some(info) = data_info {
            if file_buf.len() < info.offset_in_merged_file as usize {
                file_buf.resize(info.offset_in_merged_file as usize, 0);
            }
            file_buf.extend_from_slice(&data_data);
        }
    }

    // 8. Write LINKEDIT contents (indirectsyms, symtab, strtab, locrel)
    if nsyms > 0 {
        if file_buf.len() < indirectsymoff as usize {
            file_buf.resize(indirectsymoff as usize, 0);
        }
        for i in 0..nsyms {
            file_buf.extend_from_slice(&i.to_le_bytes());
        }

        if file_buf.len() < symoff as usize {
            file_buf.resize(symoff as usize, 0);
        }
        file_buf.extend_from_slice(&nlist_entries);

        if file_buf.len() < stroff as usize {
            file_buf.resize(stroff as usize, 0);
        }
        file_buf.extend_from_slice(&strtab_bytes);
    }

    if nlocrel > 0 {
        if file_buf.len() < locreloff {
            file_buf.resize(locreloff, 0);
        }
        file_buf.extend_from_slice(&locrel_bytes);
    }

    // Pad to code signature offset
    if file_buf.len() < sig_off as usize {
        file_buf.resize(sig_off as usize, 0);
    }

    let sig_blob = generate_adhoc_code_signature(&file_buf, sig_off);

    // Append code signature blob at the exact end of file
    file_buf.extend_from_slice(&sig_blob);

    // 9. Write to output buffer
    output_buffer.write_bytes(&file_buf);

    Ok(())
}

fn append_section_data(target: &mut Vec<u8>, binary: &RelocatedSectionBinary) {
    match binary {
        RelocatedSectionBinary::Referenced(d) => target.extend_from_slice(d),
        RelocatedSectionBinary::Owned(d) => target.extend_from_slice(d),
        RelocatedSectionBinary::None => {}
    }
}

fn write_segment_command(
    buf: &mut Vec<u8>,
    segname: &str,
    vmaddr: u64,
    vmsize: u64,
    fileoff: u64,
    filesize: u64,
    maxprot: u32,
    initprot: u32,
    nsects: u32,
) {
    let cmdsize = 72 + 80 * nsects;
    buf.extend_from_slice(&0x19u32.to_le_bytes()); // LC_SEGMENT_64
    buf.extend_from_slice(&cmdsize.to_le_bytes());

    let mut name_bytes = [0u8; 16];
    let name_src = segname.as_bytes();
    let len = name_src.len().min(16);
    name_bytes[..len].copy_from_slice(&name_src[..len]);
    buf.extend_from_slice(&name_bytes);

    buf.extend_from_slice(&vmaddr.to_le_bytes());
    buf.extend_from_slice(&vmsize.to_le_bytes());
    buf.extend_from_slice(&fileoff.to_le_bytes());
    buf.extend_from_slice(&filesize.to_le_bytes());
    buf.extend_from_slice(&maxprot.to_le_bytes());
    buf.extend_from_slice(&initprot.to_le_bytes());
    buf.extend_from_slice(&nsects.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes()); // flags
}

fn write_section_header(
    buf: &mut Vec<u8>,
    sectname: &str,
    segname: &str,
    addr: u64,
    size: u64,
    offset: u32,
    align: u32,
    flags: u32,
) {
    let mut sect_bytes = [0u8; 16];
    let sect_src = sectname.as_bytes();
    let sect_len = sect_src.len().min(16);
    sect_bytes[..sect_len].copy_from_slice(&sect_src[..sect_len]);
    buf.extend_from_slice(&sect_bytes);

    let mut seg_bytes = [0u8; 16];
    let seg_src = segname.as_bytes();
    let seg_len = seg_src.len().min(16);
    seg_bytes[..seg_len].copy_from_slice(&seg_src[..seg_len]);
    buf.extend_from_slice(&seg_bytes);

    buf.extend_from_slice(&addr.to_le_bytes());
    buf.extend_from_slice(&size.to_le_bytes());
    buf.extend_from_slice(&offset.to_le_bytes());
    buf.extend_from_slice(&align.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes()); // reloff
    buf.extend_from_slice(&0u32.to_le_bytes()); // nreloc
    buf.extend_from_slice(&flags.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes()); // reserved1
    buf.extend_from_slice(&0u32.to_le_bytes()); // reserved2
    buf.extend_from_slice(&0u32.to_le_bytes()); // reserved3
}

/// Generates a valid Mach-O ad-hoc code signature SuperBlob.
fn generate_adhoc_code_signature(file_data: &[u8], code_limit: u32) -> Vec<u8> {
    let page_size = 4096u32;
    let n_code_slots = ((code_limit + page_size - 1) / page_size) as usize;

    let ident = b"a.out\0";
    let ident_offset = 44u32;
    let hash_offset = ident_offset + ident.len() as u32;

    let cd_header_size = hash_offset;
    let cd_size = cd_header_size + (n_code_slots * 32) as u32;

    let super_blob_size = 12 + 8 + cd_size;

    let mut blob = Vec::with_capacity(super_blob_size as usize);

    // SuperBlob Header (big endian)
    blob.extend_from_slice(&0xfade0cc0u32.to_be_bytes()); // magic: CSBLOB_WKWITH_MAGIC
    blob.extend_from_slice(&super_blob_size.to_be_bytes()); // length
    blob.extend_from_slice(&1u32.to_be_bytes()); // count = 1

    // BlobIndex
    blob.extend_from_slice(&0u32.to_be_bytes()); // type: CSSLOT_CODEDIRECTORY
    blob.extend_from_slice(&20u32.to_be_bytes()); // offset: 20

    // CodeDirectory
    blob.extend_from_slice(&0xfade0c02u32.to_be_bytes()); // magic: CSMAGIC_CODEDIRECTORY
    blob.extend_from_slice(&cd_size.to_be_bytes());
    blob.extend_from_slice(&0x20100u32.to_be_bytes()); // version
    blob.extend_from_slice(&0u32.to_be_bytes()); // flags: adhoc
    blob.extend_from_slice(&hash_offset.to_be_bytes());
    blob.extend_from_slice(&ident_offset.to_be_bytes());
    blob.extend_from_slice(&0u32.to_be_bytes()); // nSpecialSlots
    blob.extend_from_slice(&(n_code_slots as u32).to_be_bytes()); // nCodeSlots
    blob.extend_from_slice(&code_limit.to_be_bytes());
    blob.extend_from_slice(&32u8.to_be_bytes()); // hashSize
    blob.extend_from_slice(&2u8.to_be_bytes()); // hashType: CS_HASHTYPE_SHA256
    blob.extend_from_slice(&0u8.to_be_bytes()); // platform
    blob.extend_from_slice(&12u8.to_be_bytes()); // pageSize (2^12 = 4096)
    blob.extend_from_slice(&0u32.to_be_bytes()); // spare2

    // Identifier
    blob.extend_from_slice(ident);

    // Calculate SHA256 for each 4096-byte page of the executable
    for i in 0..n_code_slots {
        let start = i * 4096;
        let end = ((i + 1) * 4096).min(code_limit as usize);
        let page_data = if start < file_data.len() {
            &file_data[start..end.min(file_data.len())]
        } else {
            &[]
        };

        let hash = sha2::Sha256::digest(page_data);
        blob.extend_from_slice(&hash);
    }

    blob
}

/*
fn sha256(data: &[u8]) -> [u8; 32] {
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let k: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];

    let bit_len = (data.len() as u64) * 8;
    let mut padded = data.to_vec();
    padded.push(0x80);
    while (padded.len() % 64) != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in padded.chunks_exact(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes(chunk[i * 4..(i + 1) * 4].try_into().unwrap());
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h_val] = h;

        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h_val
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(k[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            h_val = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(h_val);
    }

    let mut out = [0u8; 32];
    for i in 0..8 {
        out[i * 4..(i + 1) * 4].copy_from_slice(&h[i].to_be_bytes());
    }
    out
}
*/

#[cfg(all(target_os = "macos", test))]
mod tests {
    use super::*;
    use crate::macho::{
        filter, find_entry_point, merge, reader::read_relocatable_module, relocate, resolve,
    };
    use std::{fs, process::Command};

    fn get_clang_example_binary(file_name: &str) -> Vec<u8> {
        let file_path = std::env::current_dir()
            .unwrap()
            .join("resources/examples/mach-o/clang/aarch64")
            .join(file_name);

        fs::read(file_path).unwrap()
    }

    fn link_and_verify(
        output_name: &str,
        input_files: &[&str],
        expected_exit_code: i32,
        expected_stdout: &str,
    ) {
        let binaries: Vec<Vec<u8>> = input_files
            .iter()
            .map(|f| get_clang_example_binary(f))
            .collect();
        let mut modules = Vec::new();
        for (i, file) in input_files.iter().enumerate() {
            let module = read_relocatable_module(file, &binaries[i]).unwrap();
            modules.push(module);
        }

        let filtered_modules = filter(
            modules,
            &["_main".to_string(), "__mh_execute_header".to_string()],
        )
        .unwrap();
        let merged_asset = merge(filtered_modules, CpuType::ARM64).unwrap();
        let resolved_asset = resolve(
            merged_asset.fragment_modules,
            &merged_asset.linker_generated_symbols,
        )
        .unwrap();
        let entry_point = find_entry_point(&resolved_asset.global_symbols).unwrap();
        let relocated_modules = relocate(
            &merged_asset.merged_file_layout,
            resolved_asset.resolved_modules,
            CpuType::ARM64,
        )
        .unwrap();

        let mut output_buf = Vec::new();
        write_executable(
            &relocated_modules,
            &merged_asset.merged_file_layout,
            entry_point,
            CpuType::ARM64,
            &mut output_buf,
        )
        .unwrap();

        fs::write("/tmp/my_minimal.macho", &output_buf).unwrap();

        assert!(!output_buf.is_empty());

        use std::time::Duration;

        use wait_timeout::ChildExt;

        let temp_dir = std::path::PathBuf::from("/tmp");
        let exe_path = temp_dir.join(format!("test_macho_linker_{}", output_name));
        {
            use std::io::Write;
            let mut f = fs::File::create(&exe_path).unwrap();
            f.write_all(&output_buf).unwrap();
            f.sync_all().unwrap();
        }

        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&exe_path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&exe_path, perms).unwrap();

        let cs_output = Command::new("codesign")
            .args(["-f", "-s", "-", exe_path.to_str().unwrap()])
            .output();

        if let Err(e) = cs_output {
            panic!("codesign fail: {:?}", e);
        }

        let cs_output = cs_output.unwrap();

        if !cs_output.status.success() {
            panic!(
                "codesign failed for {}: {}",
                output_name,
                String::from_utf8_lossy(&cs_output.stderr)
            );
        }

        let mut child = Command::new(&exe_path)
            .spawn()
            .expect("Failed to execute linked binary");
        let output = match child
            .wait_timeout(Duration::from_secs(5))
            .expect("Failed to wait on child process")
        {
            Some(status) => {
                let mut stdout = Vec::new();
                let mut stderr = Vec::new();
                if let Some(mut child_stdout) = child.stdout.take() {
                    use std::io::Read;
                    child_stdout.read_to_end(&mut stdout).unwrap();
                }
                if let Some(mut child_stderr) = child.stderr.take() {
                    use std::io::Read;
                    child_stderr.read_to_end(&mut stderr).unwrap();
                }
                std::process::Output {
                    status,
                    stdout,
                    stderr,
                }
            }
            None => {
                // Timeout reached, kill the process
                child.kill().expect("Failed to kill child process");
                panic!("Execution timed out for {}", output_name);
            }
        };

        let actual_exit_code = output.status.code().unwrap_or(-1);
        let actual_stdout = String::from_utf8_lossy(&output.stdout);

        if !output.status.success() {
            println!(
                "Execution failed for {}: status={:?}, stderr={}",
                output_name,
                output.status,
                String::from_utf8_lossy(&output.stderr)
            );
        }

        assert_eq!(
            actual_exit_code, expected_exit_code,
            "Exit code mismatch for {}: expected {}, got {}",
            output_name, expected_exit_code, actual_exit_code
        );
        assert_eq!(
            actual_stdout, expected_stdout,
            "Stdout mismatch for {}: expected {:?}, got {:?}",
            output_name, expected_stdout, actual_stdout
        );

        let _ = fs::remove_file(&exe_path);
    }

    #[test]
    fn test_link_minimal() {
        link_and_verify("minimal", &["minimal.o"], 42, "");
    }

    #[test]
    fn test_link_function() {
        link_and_verify("function", &["function.o"], 0, "Hello, world!\n");
    }

    #[test]
    fn test_link_data() {
        let binary = get_clang_example_binary("data.o");
        let module = read_relocatable_module("data.o", &binary).unwrap();
        for (i, sym) in module.symbols.iter().enumerate() {
            println!("SYM {}: {:?}", i, sym);
        }
        link_and_verify("data", &["data.o"], 24, "");
    }

    #[test]
    fn test_link_symbol() {
        link_and_verify("symbol", &["symbol-import.o", "symbol-export.o"], 24, "");
    }

    #[test]
    fn test_link_override() {
        link_and_verify(
            "override",
            &["override-strong.o", "override-weak.o"],
            53,
            "",
        );
    }

    #[test]
    fn test_link_relocate_within_data() {
        let binary = get_clang_example_binary("relocate-within-data.o");
        let module = read_relocatable_module("relocate-within-data.o", &binary).unwrap();
        for (i, sym) in module.symbols.iter().enumerate() {
            println!("SYM {}: {:?}", i, sym);
        }
        link_and_verify("relocate-within-data", &["relocate-within-data.o"], 24, "");
    }
}
