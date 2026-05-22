// Copyright (c) 2026 Hemashushu <hippospark@gmail.com>, All rights reserved.
//
// This Source Code Form is subject to the terms of
// the Mozilla Public License version 2.0 and additional exceptions.
// For more details, see the LICENSE, LICENSE.additional, and CONTRIBUTING files.

use object::Endianness;
use object::elf::{
    ELFOSABI_NONE, EM_X86_64, ET_EXEC, PF_R, PF_W, PF_X, PT_LOAD, SHF_ALLOC, SHF_EXECINSTR,
    SHN_UNDEF, SHT_NOBITS, SHT_PROGBITS, STB_GLOBAL, STB_LOCAL, STT_NOTYPE, STV_DEFAULT,
};
use object::write::WritableBuffer;
use object::write::elf::{FileHeader, ProgramHeader, SectionHeader, Sym, Writer};

const LOAD_ADDR: u64 = 0x400000; // typical base address for x86_64 executables
const PAGE_SIZE: u64 = 0x1000; // segments must be page-aligned in memory
const DATA_ALIGN: u64 = 8; // .rodata, .data and .bss sections are 8-byte aligned

// Generates a minimal statically linked x86_64 ELF executable that writes
// "Hello" to stdout via a syscall. The string and its length are stored in
// the `.rodata` section; the code lives in `.text`.
//
// ## File content (example)
//
// ```text
// .text
// 0000000000401000 <_start>:
//   401000:    bf 01 00 00 00           mov    edi,0x1
//   401005:    48 8d 35 f4 0f 00 00     lea    rsi,[rip+0xff4]        # 402000 <msg>
//   40100c:    48 8b 15 f4 0f 00 00     mov    rdx,QWORD PTR [rip+0xff4]        # 402007 <len>
//   401013:    b8 01 00 00 00           mov    eax,0x1
//   401018:    0f 05                    syscall
//   40101a:    48 31 ff                 xor    rdi,rdi
//   40101d:    b8 3c 00 00 00           mov    eax,0x3c
//   401022:    0f 05                    syscall
//
// .rodata
//   402000 48656c6c 6f0a0006 00000000 000000    Hello..........
// ```
//
// ## File layout
//
// | Size | Content         |
// |------|-----------------|
// | 64   | ELF header      |
// | Mx56 | program headers |
// | ...  | sections        |
// | Nx64 | section headers |
//
// ## Sections (in file order):
//
// | Name                       | Type         | Description                 | Alignment |
// |----------------------------|--------------|-----------------------------|-----------|
// | NULL                       | SHT_NULL     | Null section header         | 0         |
// | `.init`, `.text`, `.finit` | SHT_PROGBITS | Executable code             | 16        |
// | `.rodata`                  | SHT_PROGBITS | Read-only data (strings)    | 4/8       |
// | `.data`                    | SHT_PROGBITS | Initialized data            | 4/8       |
// | `.bss`                     | SHT_NOBITS   | Uninitialized data (zeroed) | 4/8       |
// | `.symtab`                  | SHT_SYMTAB   | Symbol table                | 8         |
// | `.strtab`                  | SHT_STRTAB   | Strings for symbol names    | 1         |
// | `.shstrtab`                | SHT_STRTAB   | Strings for section names   | 1         |
//
// ## Program headers
//
// | Segment | Sections                      | Type | Flags | Alignment |
// |---------|-------------------------------|------|-------|-----------|
// | 00      | File header + program headers | Load | R     | 0x1000    |
// | 01      | `.init`, `.text`, `.finit`    | Load | R E   | 0x1000    |
// | 02      | .rodata                       | Load | R     | 0x1000    |
// | 03      | .data, .bss                   | Load | R W   | 0x1000    |
//
// ## Symbols (example)
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
#[allow(dead_code)]
pub fn write_elf_x86_64(buffer: &mut dyn WritableBuffer) {
    // -------------------------------------------------------------------------
    // Construct .text and .rodata contents
    // -------------------------------------------------------------------------

    // The code before linking:
    //
    // ```text
    // 0000000000000000 <_start>:
    //    0:    bf 01 00 00 00           mov    edi,0x1
    //    5:    48 8d 35 00 00 00 00     lea    rsi,[rip+0x0]        # c <_start+0xc>
    //             8: R_X86_64_PC32    .rodata-0x4
    //    c:    48 8b 15 00 00 00 00     mov    rdx,QWORD PTR [rip+0x0]        # 13 <_start+0x13>
    //             f: R_X86_64_PC32    .rodata+0x3
    //   13:    b8 01 00 00 00           mov    eax,0x1
    //   18:    0f 05                    syscall
    //   1a:    48 31 ff                 xor    rdi,rdi
    //   1d:    b8 3c 00 00 00           mov    eax,0x3c
    //   22:    0f 05                    syscall
    // ```

    let mut text_data: Vec<u8> = Vec::new();
    let symbol_start_offset = text_data.len(); // offset of _start symbol in .text

    text_data.extend_from_slice(&[0xbf, 0x01, 0x00, 0x00, 0x00]); // mov edi, 1
    text_data.extend_from_slice(&[0x48, 0x8d, 0x35, 0x00, 0x00, 0x00, 0x00]); // lea rsi, [rip+0] (reloc to .rodata)
    let relo_offset_msg = text_data.len() - 4; // offset of the 4-byte relocation in .text

    text_data.extend_from_slice(&[0x48, 0x8b, 0x15, 0x00, 0x00, 0x00, 0x00]); // mov rdx, [rip+0] (reloc to .rodata)
    let relo_offset_len = text_data.len() - 4; // offset of the 4-byte relocation in .text

    text_data.extend_from_slice(&[0xb8, 0x01, 0x00, 0x00, 0x00]); // mov eax, 1
    text_data.extend_from_slice(&[0x0f, 0x05]); // syscall
    text_data.extend_from_slice(&[0x48, 0x31, 0xff]); // xor rdi, rdi
    text_data.extend_from_slice(&[0xb8, 0x3c, 0x00, 0x00, 0x00]); // mov eax, 60
    text_data.extend_from_slice(&[0x0f, 0x05]); // syscall

    // .rodata contents: "Hello" followed by its length as a little-endian u64
    let msg_data: &[u8] = b"Hello\n";
    let msg_len: u64 = msg_data.len() as u64;
    let len_data: [u8; 8] = msg_len.to_le_bytes();

    let symbol_msg_offset = 0;
    let symbol_len_offset = align_up(symbol_msg_offset + msg_len as usize, DATA_ALIGN as usize);
    let rodata_size = symbol_len_offset + len_data.len();

    let mut rodata_data = vec![0; rodata_size];
    rodata_data[symbol_msg_offset..symbol_msg_offset + msg_data.len()].copy_from_slice(msg_data);
    rodata_data[symbol_len_offset..symbol_len_offset + len_data.len()].copy_from_slice(&len_data);

    // -------------------------------------------------------------------------
    // Pre-calculate lengths and offsets
    // -------------------------------------------------------------------------
    let elf_header_size: usize = 64; // ELF64 header size is fixed at 64 bytes
    let program_header_entry_size: usize = 56; // ELF64
    let number_of_program_headers: u32 = 4; // 00-NULL, 01-.text, 02-.rodata, 03-.data

    // let section_header_entry_size: usize = 64; // ELF64
    // let number_of_section_headers: u16 = 8; // NULL, .text, .rodata, .data, .bss, .symtab, .strtab, .shstrtab
    // let section_header_string_table_index: u16 = 7; // index of .shstrtab in section header table

    // --------
    // Sections
    // --------

    let headers_size =
        elf_header_size + program_header_entry_size * (number_of_program_headers as usize);

    let section_text_offset = align_up(headers_size, PAGE_SIZE as usize);
    let section_text_size: usize = text_data.len();

    let section_rodata_offset =
        align_up(section_text_offset + section_text_size, PAGE_SIZE as usize);
    let section_rodata_size: usize = rodata_data.len();

    let section_data_offset = align_up(
        section_rodata_offset + section_rodata_size,
        PAGE_SIZE as usize,
    );
    let section_data_size: usize = 0;

    // .bss section starts immediately after .data section
    let section_bss_offset = align_up(section_data_offset + section_data_size, DATA_ALIGN as usize);
    let section_bss_memory_size: usize = 0; // there is no data in .bss section, but it still occupies memory

    // --------
    // Program headers
    // --------

    let segment_null_offset = 0_usize;
    let segment_null_size =
        elf_header_size + program_header_entry_size * (number_of_program_headers as usize);
    let segment_null_virtual_address = LOAD_ADDR;

    let segment_text_offset = section_text_offset;
    let segment_text_size = section_text_size;
    let segment_text_virtual_address = LOAD_ADDR + segment_text_offset as u64;

    let segment_rodata_offset = section_rodata_offset;
    let segment_rodata_size = section_rodata_size;
    let segment_rodata_virtual_address = LOAD_ADDR + segment_rodata_offset as u64;

    // segment for .data and .bss sections.
    let segment_data_offset = section_data_offset;
    let segment_data_size =
        align_up(section_data_size, DATA_ALIGN as usize) + section_bss_memory_size;
    let segment_data_virtual_address = LOAD_ADDR + segment_data_offset as u64;

    // --------
    // Relocations and symbol addresses
    // --------

    // Calculate the relocation values for the .text section.
    let dist_rodata = segment_rodata_virtual_address - segment_text_virtual_address;
    let relo_msg_value = dist_rodata + symbol_msg_offset as u64 - (relo_offset_msg as u64 + 4); // `-4` is addend.
    let relo_len_value = dist_rodata + symbol_len_offset as u64 - (relo_offset_len as u64 + 4); // `-4` is addend.

    // Patch the relocation values into the .text data. These will be used to create R_X86_64_PC32 relocations later.
    text_data[relo_offset_msg..relo_offset_msg + 4]
        .copy_from_slice(&(relo_msg_value as u32).to_le_bytes());
    text_data[relo_offset_len..relo_offset_len + 4]
        .copy_from_slice(&(relo_len_value as u32).to_le_bytes());

    // --------
    // Symbol addresses
    // --------

    let symbol_start_virtual_address = segment_text_virtual_address + symbol_start_offset as u64;
    let symbol_msg_virtual_address = segment_rodata_virtual_address + symbol_msg_offset as u64;
    let symbol_len_virtual_address = segment_rodata_virtual_address + symbol_len_offset as u64;
    let symbol_edata_virtual_address = segment_data_virtual_address + section_data_size as u64; // _edata is at the end of the .data section
    let symbol_bss_start_virtual_address =
        align_up(symbol_edata_virtual_address as usize, DATA_ALIGN as usize) as u64; // __bss_start is at the end of the .data section
    let symbol_end_virtual_address =
        symbol_bss_start_virtual_address + section_bss_memory_size as u64; // _end is at the end of the .bss section

    // The entry point is the virtual address of the `_start` symbol, which is at the beginning of the .text section.
    let entry_point = symbol_start_virtual_address;

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

    // Both `Vec<u8>` and `StreamingBuffer` implement `WritableBuffer`.
    // `Vec<u8>` is simpler for this example, but `StreamingBuffer` can be used to
    // write directly to a file without buffering the entire contents in memory.
    let mut writer = Writer::new(Endianness::Little, true, buffer);

    // -------------------------------------------------------------------------
    // Phase 1: build string tables
    // -------------------------------------------------------------------------

    let string_id_msg = writer.add_string(b"msg");
    let string_id_len = writer.add_string(b"len");
    let string_id_start = writer.add_string(b"_start");
    let string_id_edata = writer.add_string(b"_edata");
    let string_id_bss_start = writer.add_string(b"__bss_start");
    let string_id_end = writer.add_string(b"_end");

    let section_text_name = writer.add_section_name(b".text");
    let section_rodata_name = writer.add_section_name(b".rodata");
    let section_data_name = writer.add_section_name(b".data");
    let section_bss_name = writer.add_section_name(b".bss");
    writer.add_section_name(b".symtab");
    writer.add_section_name(b".strtab");
    writer.add_section_name(b".shstrtab");

    // -------------------------------------------------------------------------
    // Phase 2: reserve section and symbol indices
    // -------------------------------------------------------------------------

    writer.reserve_null_section_index(); // null section
    let section_text_idx = writer.reserve_section_index();
    let section_rodata_idx = writer.reserve_section_index();
    let section_data_idx = writer.reserve_section_index();
    let section_bss_idx = writer.reserve_section_index();
    writer.reserve_symtab_section_index();
    writer.reserve_strtab_section_index();
    writer.reserve_shstrtab_section_index();

    writer.reserve_null_symbol_index(); // null symbol
    writer.reserve_symbol_index(None); // msg symbol
    writer.reserve_symbol_index(None); // len symbol
    writer.reserve_symbol_index(None); // _start symbol
    writer.reserve_symbol_index(None); // _edata symbol
    writer.reserve_symbol_index(None); // __bss_start symbol
    writer.reserve_symbol_index(None); // _end symbol

    // -------------------------------------------------------------------------
    // Phase 3: reserve file ranges for headers and sections
    // -------------------------------------------------------------------------

    writer.reserve_file_header();
    writer.reserve_program_headers(number_of_program_headers);

    // Reserve space for section `.text`
    let actual_text_offset = writer.reserve(text_data.len(), PAGE_SIZE as usize);
    debug_assert_eq!(
        actual_text_offset, section_text_offset,
        ".text offset mismatch"
    );

    // Reserve space for section `.rodata`
    let actual_rodata_offset = writer.reserve(rodata_data.len(), PAGE_SIZE as usize);
    debug_assert_eq!(
        actual_rodata_offset, section_rodata_offset,
        ".rodata offset mismatch"
    );

    // Reserve space for section `.data`
    let actual_data_offset = writer.reserve(section_data_size, PAGE_SIZE as usize);
    debug_assert_eq!(
        actual_data_offset, section_data_offset,
        ".data offset mismatch"
    );

    writer.reserve_symtab();
    writer.reserve_strtab();
    writer.reserve_shstrtab();
    writer.reserve_section_headers();

    // -------------------------------------------------------------------------
    // Phase 4: write headers, sections, and symbols in the reserved order
    // -------------------------------------------------------------------------

    // Write ELF header
    writer
        .write_file_header(&FileHeader {
            os_abi: ELFOSABI_NONE,
            abi_version: 0,
            e_type: ET_EXEC,
            e_machine: EM_X86_64,
            e_entry: entry_point,
            e_flags: 0,
        })
        .expect("failed to write ELF file header");

    writer.write_align_program_headers();

    // Write 00-NULL segment header
    writer.write_program_header(&ProgramHeader {
        p_type: PT_LOAD,
        p_flags: PF_R,
        p_offset: segment_null_offset as u64,
        p_vaddr: segment_null_virtual_address,
        p_paddr: segment_null_virtual_address,
        p_filesz: segment_null_size as u64,
        p_memsz: segment_null_size as u64,
        p_align: PAGE_SIZE,
    });

    // Write 01-.text segment header
    writer.write_program_header(&ProgramHeader {
        p_type: PT_LOAD,
        p_flags: PF_R | PF_X,
        p_offset: segment_text_offset as u64,
        p_vaddr: segment_text_virtual_address,
        p_paddr: segment_text_virtual_address,
        p_filesz: segment_text_size as u64,
        p_memsz: segment_text_size as u64,
        p_align: PAGE_SIZE,
    });

    // Write 02-.rodata segment header
    writer.write_program_header(&ProgramHeader {
        p_type: PT_LOAD,
        p_flags: PF_R,
        p_offset: segment_rodata_offset as u64,
        p_vaddr: segment_rodata_virtual_address,
        p_paddr: segment_rodata_virtual_address,
        p_filesz: segment_rodata_size as u64,
        p_memsz: segment_rodata_size as u64,
        p_align: PAGE_SIZE,
    });

    // Write 03-.data segment header
    writer.write_program_header(&ProgramHeader {
        p_type: PT_LOAD,
        p_flags: PF_R | PF_W,
        p_offset: segment_data_offset as u64,
        p_vaddr: segment_data_virtual_address,
        p_paddr: segment_data_virtual_address,
        p_filesz: segment_data_size as u64,
        p_memsz: segment_data_size as u64,
        p_align: PAGE_SIZE,
    });

    // Write .text section data
    writer.write_align(PAGE_SIZE as usize);
    writer.write(&text_data);

    // Write .rodata section data
    writer.write_align(PAGE_SIZE as usize);
    writer.write(&rodata_data);

    // Write .data section data
    writer.write_align(PAGE_SIZE as usize);

    // Write symbol `NULL`
    writer.write_symbol(&Sym {
        name: None,

        // Section `.symtab_shndx` index.
        // When the section index is beyond 0xffff, the actual section index is stored in
        // the `.symtab_shndx` section and the `st_shndx` field here is set to SHN_XINDEX (0xffff).
        section: None,

        // high 4 bits is the binding (e.g. STB_GLOBAL, STB_LOCAL, and STB_WEAK),
        // low 4 bits is the type (e.g. STT_FUNC, STT_OBJECT, STT_SECTION, STT_FILE, and STT_COMMON)
        st_info: STB_LOCAL << 4 | STT_NOTYPE,

        // Symbol visibility.
        // Possible values are STV_DEFAULT, STV_HIDDEN, and STV_PROTECTED etc,.
        st_other: STV_DEFAULT,

        // section index of the symbol, e.g. 1 for .text, 2 for .rodata,
        // and special indices like SHN_UNDEF for undefined symbols and SHN_ABS for absolute symbols
        st_shndx: SHN_UNDEF,

        // virtual address of the symbol in memory (for defined symbols) or 0 (for undefined symbols).
        st_value: 0,

        // usually 0
        st_size: 0,
    });

    // Write symbol `msg`
    writer.write_symbol(&Sym {
        name: Some(string_id_msg),
        section: None,
        st_info: STB_LOCAL << 4 | STT_NOTYPE,
        st_other: STV_DEFAULT,
        st_shndx: section_rodata_idx.0 as u16,
        st_value: symbol_msg_virtual_address,
        st_size: 0,
    });

    // Write symbol `len`
    writer.write_symbol(&Sym {
        name: Some(string_id_len),
        section: None,
        st_info: STB_LOCAL << 4 | STT_NOTYPE,
        st_other: STV_DEFAULT,
        st_shndx: section_rodata_idx.0 as u16,
        st_value: symbol_len_virtual_address,
        st_size: 0,
    });

    // Write symbol `_start`
    writer.write_symbol(&Sym {
        name: Some(string_id_start),
        section: None,
        st_info: (STB_GLOBAL << 4) | STT_NOTYPE,
        st_other: STV_DEFAULT,
        st_shndx: section_text_idx.0 as u16,
        st_value: symbol_start_virtual_address,
        st_size: 0,
    });

    // Write symbol `_edata`
    writer.write_symbol(&Sym {
        name: Some(string_id_edata),
        section: None,
        st_info: (STB_GLOBAL << 4) | STT_NOTYPE,
        st_other: STV_DEFAULT,
        st_shndx: section_data_idx.0 as u16,
        st_value: symbol_edata_virtual_address,
        st_size: 0,
    });

    // Write symbol `__bss_start`
    writer.write_symbol(&Sym {
        name: Some(string_id_bss_start),
        section: None,
        st_info: (STB_GLOBAL << 4) | STT_NOTYPE,
        st_other: STV_DEFAULT,
        st_shndx: section_bss_idx.0 as u16,
        st_value: symbol_bss_start_virtual_address,
        st_size: 0,
    });

    // Write symbol `_end`
    writer.write_symbol(&Sym {
        name: Some(string_id_end),
        section: None,
        st_info: (STB_GLOBAL << 4) | STT_NOTYPE,
        st_other: STV_DEFAULT,
        st_shndx: section_bss_idx.0 as u16,
        st_value: symbol_end_virtual_address,
        st_size: 0,
    });

    // Write .strtab section data
    writer.write_strtab();

    // Write .shstrtab section data
    writer.write_shstrtab();

    // Write section header: null
    writer.write_null_section_header();

    // Write section header: .text
    writer.write_section_header(&SectionHeader {
        name: Some(section_text_name),
        sh_type: SHT_PROGBITS,
        sh_flags: (SHF_ALLOC | SHF_EXECINSTR) as u64,
        sh_addr: segment_text_virtual_address,
        sh_offset: section_text_offset as u64,
        sh_size: section_text_size as u64,

        // depends on the section type, for SHT_PROGBITS it is usually 0
        // for section `.rela.text`, it is the index of the section `.symtab` that holds the symbols.
        // for section `.symtab`, it is the index of the associated string table section (`.strtab`),
        sh_link: 0,

        // depends on the section type, for SHT_PROGBITS it is usually 0
        // for section `.rela.text`, it is the index of the section to which the relocations apply (e.g. `.text`)
        // for section `.symtab`, it is the index of the first non-local symbol (i.e. the number of local symbols)
        sh_info: 0,
        sh_addralign: 16, // code sections are usually aligned to 16 bytes
        sh_entsize: 0,
    });

    // Write section header: .rodata
    writer.write_section_header(&SectionHeader {
        name: Some(section_rodata_name),
        sh_type: SHT_PROGBITS,
        sh_flags: SHF_ALLOC as u64,
        sh_addr: segment_rodata_virtual_address,
        sh_offset: section_rodata_offset as u64,
        sh_size: section_rodata_size as u64,
        sh_link: 0,
        sh_info: 0,
        sh_addralign: DATA_ALIGN, // read-only data sections are usually aligned to 8 or 4 bytes
        sh_entsize: 0,
    });

    // Write section header: .data
    writer.write_section_header(&SectionHeader {
        name: Some(section_data_name),
        sh_type: SHT_PROGBITS,
        sh_flags: (SHF_ALLOC | PF_W) as u64,
        sh_addr: segment_data_virtual_address,
        sh_offset: section_data_offset as u64,
        sh_size: section_data_size as u64,
        sh_link: 0,
        sh_info: 0,
        sh_addralign: DATA_ALIGN, // data sections are usually aligned to 8 or 4 bytes
        sh_entsize: 0,
    });

    // Write section header: .bss
    writer.write_section_header(&SectionHeader {
        name: Some(section_bss_name),
        sh_type: SHT_NOBITS,
        sh_flags: (SHF_ALLOC | PF_W) as u64,
        sh_addr: segment_data_virtual_address + section_data_size as u64,
        sh_offset: section_bss_offset as u64,
        sh_size: 0, // .bss has no data in the file
        sh_link: 0,
        sh_info: 0,
        sh_addralign: DATA_ALIGN, // .bss sections are usually aligned to 8 or 4 bytes
        sh_entsize: 0,
    });

    // Write section header: .symtab
    let num_local = 3; // `null`, `msg`, and `len` are local symbols
    writer.write_symtab_section_header(num_local);

    // Write section header: .strtab
    writer.write_strtab_section_header();

    // Write section header: .shstrtab
    writer.write_shstrtab_section_header();
}

fn align_up(val: usize, align: usize) -> usize {
    (val + align - 1) & !(align - 1)
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use crate::write_elf_x86_64;

    #[test]
    fn test_write_elf_x86_64() {
        let mut buffer: Vec<u8> = Vec::new();
        write_elf_x86_64(&mut buffer);

        let tmp_dir = std::env::temp_dir();
        let path = tmp_dir.join("hello.elf");
        std::fs::write(&path, &buffer).expect("failed to write ELF file");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("failed to set permissions");
    }
}
