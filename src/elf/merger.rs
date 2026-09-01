// Copyright (c) 2026 Hemashushu <hippospark@gmail.com>, All rights reserved.
//
// This Source Code Form is subject to the terms of
// the Mozilla Public License version 2.0 and additional exceptions.
// For more details, see the LICENSE, LICENSE.additional, and CONTRIBUTING files.

use std::collections::HashMap;

use crate::{
    elf::module::{Machine, RelocatableModule, Relocation, Symbol, SymbolBind},
    error::LinkerError,
};

/// ELF file layout
/// ===============
///
/// Overall
/// -------
///
/// | Size   | Content         |
/// |--------|-----------------|
/// | 64     | ELF header      |
/// | m * 56 | program headers |
/// | ...    | section data    |
/// | n * 64 | section headers |
///
/// Sections (in file order)
/// ------------------------
///
/// | Name           | Type         | Description                     | Align | Opt? |
/// |----------------|--------------|---------------------------------|-------|------|
/// | 00 NULL        | SHT_NULL     | Null section header             | 0     |      |
/// | 01 `.text`     | SHT_PROGBITS | Executable code                 | 16    |      |
/// | 02 `.rodata`   | SHT_PROGBITS | Read-only data (strings)        | 4/8   | Opt  |
/// | 03 `.tdata`    | SHT_PROGBITS | Initialized thread-local data   | 4/8   | Opt  |
/// | 04 `.tbss`     | SHT_NOBITS   | Uninitialized thread-local data | 4/8   | Opt  |
/// | 05 `.data`     | SHT_PROGBITS | Initialized data                | 4/8   | Opt  |
/// | 06 `.bss`      | SHT_NOBITS   | Uninitialized data              | 4/8   | Opt  |
/// | 07 `.symtab`   | SHT_SYMTAB   | Symbol table                    | 8     |      |
/// | 08 `.strtab`   | SHT_STRTAB   | Strings for symbol names        | 1     |      |
/// | 09 `.shstrtab` | SHT_STRTAB   | Strings for section names       | 1     |      |
///
/// Note that sections such as `.rela.*` are consumed by the linker and would not appear in the final executable.
///
/// Program headers
/// ---------------
///
/// | Segment           | Sections                        | Type    | Flags | Alignment | Opt? |
/// |-------------------|---------------------------------|---------|-------|-----------|------|
/// | 00 phdr           | program headers                 | PT_PHDR | R     | 0x8       |      |
/// | 01 meta           | file header and program headers | PT_LOAD | R     | 0x1000    |      |
/// | 02 text           | .text                           | PT_LOAD | R E   | 0x1000    |      |
/// | 03 read-only data | .rodata                         | PT_LOAD | R     | 0x1000    | Opt  |
/// | 04 writable data  | .tdata, .tbss, .data, .bss      | PT_LOAD | R W   | 0x1000    | Opt  |
/// | 05 tls            | .tdata, .tbss                   | PT_TLS  | R     | 0x8       | Opt  |

// The names of the supported sections
pub const SECTION_NAME_TEXT: &str = ".text";
pub const SECTION_NAME_RODATA: &str = ".rodata";
pub const SECTION_NAME_TDATA: &str = ".tdata";
pub const SECTION_NAME_TBSS: &str = ".tbss";
pub const SECTION_NAME_DATA: &str = ".data";
pub const SECTION_NAME_BSS: &str = ".bss";

// The names of the supported relocation sections
pub const SECTION_NAME_RELA_TEXT: &str = ".rela.text";
pub const SECTION_NAME_RELA_RODATA: &str = ".rela.rodata";
pub const SECTION_NAME_RELA_DATA: &str = ".rela.data";
pub const SECTION_NAME_RELA_TDATA: &str = ".rela.tdata";

// ELF64 header size is fixed at 64 bytes
pub const ELF_HEADER_SIZE: usize = 64;

// ELF64 program header entry size is fixed at 56 bytes
pub const PROGRAM_HEADER_ENTRY_SIZE: usize = 56;

// All executable file contains `PHDR`, `meta`, and `code` segements,
// and the following are optional:
// - `read-only data`: .rodata
// - `writable data`: .tdata, .tbss, .data, .bss
// - `TLS data`: .tdata, .tbss
pub const BASE_PROGRAM_HEADER_COUNT: usize = 3;

/// A merged module represents essential elements of an object file,
/// which are intended to be merged into a single executable file.
///
/// The `MergedModule` is part of an object file,
/// and it is not a complete representation of all the details of an object file.
/// It assumes that an object file contains only:
///
/// - At most one code section `.text`
/// - At most one read-only data section `.rodata`
/// - At most one thread local data section `.tdata`
/// - At most one thread local uninitialized section `.tbss`
/// - At most one data section `.data`
/// - At most one uninitialized data section `.bss`
/// - At most one symbol table `.symtab`
/// - At most one relocation table `.rela.text`
/// - At most one relocation table `.rela.rodata`
/// - At most one relocation table `.rela.data`
/// - At most one relocation table `.rela.tdata`
/// - At most one string table `.strtab` (for symbol names)
/// - One section header string table `.shstrtab` (for section names)
///
/// Other sections and details of the object file are ignored without notice.
///
/// Note:
/// GCC in modern Linux distributions (e.g., Ubuntu 22.04) generates PIE (Position Independent Executable) by default,
/// which means that the `.data.rel.local` section is generated instead of the `.data` section,
/// and the `.rela.data.rel.local` section is generated instead of the `.rela.data` section.
/// As well as `.data.rel.ro.local` and `.rela.data.rel.ro.local` sections are generated
/// instead of the `.rodata` and `.rela.rodata` sections.
/// However, the current implementation of the linker does not support PIE, so we need to use a non-PIE object file for testing.
#[derive(Debug, PartialEq)]
pub struct MergedModule {
    /// The relevant sections of the module
    pub sections: HashMap<SectionName, MergedSection>,

    /// The symbol table of the module, which contains the symbols defined in the module.
    pub symbols: Vec<MergedSymbol>,

    /// The relocation entries of the module, which contain the information about
    /// how to adjust the code and data when linking.
    pub relocation_sections: Vec<MergedRelocationSection>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SectionName {
    Text,
    ROData,
    TData,
    TBSS,
    Data,
    BSS,

    Other, // Other sections that are not relevant to the final executable
}

/// Section represents a section in the merged module
#[derive(Debug, PartialEq, Clone)]
pub struct MergedSection {
    /// The size of the section.
    /// For the `.bss` and `.tbss` sections, this is the memory size of the section,
    /// which is not present in the file, but occupies space in memory.
    pub size: usize,

    /// The binary data of the section.
    ///
    /// Note: only `.text`, `.rodata`, `.tdata`, and `.data` sections
    /// contain binary data in the object file,
    /// while `.bss` and `.tbss` sections do not contain binary data in the object file.
    pub binary: Option<Vec<u8>>,

    /// The section offset in the final executable, which are calculated during the linking process.
    pub offset_in_file: usize,

    /// The virtual addresses of the sections in the final executable,
    /// which are calculated during the linking process based on the section offsets and the load address.
    ///
    /// For most sections, `virtual address = load address + section offset`,
    /// but start from the `.data` section, the virtual address is also affected by the
    /// size of the previous section `.bss` (which is not present in the file, but occupies space in memory).
    pub virtual_address: usize,
}

impl MergedSection {
    pub fn new(size: usize, binary: &[u8], offset_in_file: usize, virtual_address: usize) -> Self {
        MergedSection {
            size,
            binary: Some(binary.to_vec()),
            offset_in_file,
            virtual_address,
        }
    }

    pub fn new_bss(size: usize, offset_in_file: usize, virtual_address: usize) -> Self {
        MergedSection {
            size,
            binary: None,
            offset_in_file,
            virtual_address,
        }
    }
}

/// Symbol represents a symbol in the merged module
#[derive(Debug, PartialEq)]
pub enum MergedSymbol {
    Effective {
        // /// The offset of the symbol in the merged section in the final executable,
        // offset_in_section: usize,
        /// The virtual address of the symbol in the merged section in the final executable,
        virtual_address: usize,
    },

    /// Symbols that the linker does not care about.
    Other,
}

impl From<&str> for SectionName {
    fn from(value: &str) -> Self {
        match value {
            SECTION_NAME_TEXT => SectionName::Text,
            SECTION_NAME_RODATA => SectionName::ROData,
            SECTION_NAME_TDATA => SectionName::TData,
            SECTION_NAME_TBSS => SectionName::TBSS,
            SECTION_NAME_DATA => SectionName::Data,
            SECTION_NAME_BSS => SectionName::BSS,
            _ => SectionName::Other,
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct MergedRelocationSection {
    /// The section which these relocations apply to.
    ///
    /// Currently, only the following sections are supported:
    /// - Text
    /// - RoData
    /// - Data
    /// - TData
    pub target_section_name: SectionName,

    /// The relocation entries in this section.
    pub relocations: Vec<Relocation>,
}

#[derive(Debug, PartialEq)]
pub struct GlobalSymbolMapEntry {
    // /// The file offset of the symbol in the final executable,
    // pub offset_in_section: usize,
    /// The virtual address of the symbol in the final executable,
    pub virtual_address: usize,

    /// Whether the symbol is weakly bound
    pub is_weak: bool,
}

impl GlobalSymbolMapEntry {
    pub fn new(/* offset_in_section: usize, */ virtual_address: usize, is_weak: bool) -> Self {
        GlobalSymbolMapEntry {
            // offset_in_section,
            virtual_address,
            is_weak,
        }
    }
}

fn get_load_address_base(arch: &Machine) -> usize {
    match arch {
        // typical base address for x86_64 executables (ET_EXEC),
        // by a contrast, PIE/DSO (ET_DYN) usually has a base address of 0.
        Machine::X86_64 => 0x400000,
        Machine::AArch64 => 0x400000,
        Machine::RiscV => 0x10000,
        Machine::LoongArch => 0x120000000,
        Machine::PowerPC64 => 0x10000000,
        Machine::S390 => 0x1000000,
        _ => unimplemented!("Unsupported architecture: {:?}", arch),
    }
}

pub const SEGMENT_ALIGN_PHDR: usize = 0x8;
pub const SEGMENT_ALIGN_TLS: usize = 0x8;

fn get_segment_align_page_size(arch: &Machine) -> usize {
    match arch {
        Machine::X86_64 => 0x1000,
        Machine::AArch64 => 0x10000,
        Machine::RiscV => 0x1000,
        Machine::LoongArch => 0x10000,
        Machine::PowerPC64 => 0x10000,
        Machine::S390 => 0x1000,
        _ => unimplemented!("Unsupported architecture: {:?}", arch),
    }
}

// .rodata, .data and .tdata sections are 8-byte aligned. This is used for
// merging data sections from different modules
pub const SECTION_ALIGN_DATA: usize = 8;

// The symbol table section is 8-byte aligned
pub const SECTION_ALIGN_SYMTAB: usize = 8;

fn get_section_align_text(arch: &Machine) -> usize {
    match arch {
        Machine::X86_64 => 16,
        Machine::AArch64 => 64,
        Machine::RiscV => 4,
        Machine::LoongArch => 32,
        Machine::PowerPC64 => 32,
        Machine::S390 => 8,
        _ => unimplemented!("Unsupported architecture: {:?}", arch),
    }
}

fn contains_read_only_data_section(modules: &[crate::elf::module::RelocatableModule]) -> bool {
    modules.iter().any(|module| {
        module
            .sections
            .iter()
            .any(|s| s.name == SECTION_NAME_RODATA)
    })
}

fn contains_writable_data_section(modules: &[crate::elf::module::RelocatableModule]) -> bool {
    modules.iter().any(|module| {
        let existing_data = matches!(module.sections.iter().find(|s| s.name == SECTION_NAME_DATA),
        Some(section) if section.size > 0);

        let existing_bss = matches!(module.sections.iter().find(|s| s.name == SECTION_NAME_BSS),
        Some(section) if section.size > 0);

        let existing_tls = contains_tls_data_section(modules);

        existing_data || existing_bss || existing_tls
    })
}

fn contains_tls_data_section(modules: &[crate::elf::module::RelocatableModule]) -> bool {
    modules.iter().any(|module| {
        let existing_tdata = matches!(module.sections.iter().find(|s| s.name == SECTION_NAME_TDATA),
        Some(section) if section.size > 0);

        let existing_tbss = matches!(module.sections.iter().find(|s| s.name == SECTION_NAME_TBSS),
        Some(section) if section.size > 0);

        existing_tdata || existing_tbss
    })
}

fn align_up(val: usize, align: usize) -> usize {
    (val + align - 1) & !(align - 1)
}

/// Filter out the modules that do not contain any relevant symbols
/// for the final executable.
///
/// The first module is the main module, which contains the entry point of the executable.
/// This function traverses from the main module to find all the modules that are reachable
/// through the imported and exported symbols.
pub fn filter<'a>(
    modules: Vec<RelocatableModule<'a>>,
) -> Result<Vec<RelocatableModule<'a>>, LinkerError> {
    // build the exported symbol map for all modules
    let mut exported_symbol_map: HashMap<String, (/* module_index */ usize, /* is_weak */ bool)> =
        HashMap::new();

    for (module_index, module) in modules.iter().enumerate() {
        for symbol in &module.symbols {
            if let Symbol::Defined { name, bind, .. } = symbol {
                match bind {
                    SymbolBind::Global => {
                        let existing_entry = exported_symbol_map.get(name);
                        if let Some((existing_module_index, existing_is_weak)) = existing_entry {
                            if !*existing_is_weak {
                                // Duplicate strong symbol, which is an error
                                return Err(LinkerError::Message(format!(
                                    "Duplicate strong symbol: {} defined in module {} and module {}",
                                    name, existing_module_index, module_index
                                )));
                            }
                        }
                        exported_symbol_map.insert(name.clone(), (module_index, false));
                    }
                    SymbolBind::Weak => {
                        if !exported_symbol_map.contains_key(name) {
                            // we should ignore the weak symbol no matter whether the existing symbol is weak or strong
                            exported_symbol_map.insert(name.clone(), (module_index, true));
                        }
                    }
                    _ => {
                        // Local symbols are not exported
                    }
                }
            }
        }
    }

    let mut reachable_module_indices: Vec<usize> = vec![];
    reachable_module_indices.push(0); // The first module is the main module

    let mut pending_module_indices: Vec<usize> = vec![];
    pending_module_indices.push(0); // The first module is the main module

    while let Some(module_index) = pending_module_indices.pop() {
        let module = &modules[module_index];
        for symbol in &module.symbols {
            if let Symbol::External(name) = symbol {
                if let Some((target_module_index, _)) = exported_symbol_map.get(name) {
                    if !reachable_module_indices.contains(target_module_index) {
                        reachable_module_indices.push(*target_module_index);
                        pending_module_indices.push(*target_module_index);
                    }
                } else {
                    return Err(LinkerError::Message(format!(
                        "Unresolved external symbol: {} in module {}",
                        name, module_index
                    )));
                }
            }
        }
    }

    let reachable_modules = modules
        .into_iter()
        .enumerate()
        .filter_map(|(index, module)| {
            if reachable_module_indices.contains(&index) {
                Some(module)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    Ok(reachable_modules)
}

#[derive(Debug, PartialEq)]
pub struct MergeResult {
    pub contains_read_only_data: bool,

    pub contains_writable_data: bool,

    /// Indicates whether the final executable contains TLS segments.
    pub contains_tls_data: bool,

    /// The number of program headers in the final executable.
    ///
    /// In general, there are `PHDR, metadata, code, read-only data, writable data` five program headers,
    /// and an additional TLS segment if there is TLS.
    pub program_header_count: usize,

    /// The virtual address of the entry point (the `_start` symbol) in the final executable.
    pub entry_point: usize,

    pub merged_modules: Vec<MergedModule>,

    pub global_symbol_map: HashMap<String, GlobalSymbolMapEntry>,
}

pub fn merge(modules: Vec<RelocatableModule>, arch: &Machine) -> Result<MergeResult, LinkerError> {
    // Create section name map for each module
    // This map is used to quickly find the section index of a given section name,
    // or to get the section name of a given section index.
    let mut section_name_maps = vec![];
    for module in &modules {
        let mut section_name_map = Vec::new();
        for section in &module.sections {
            let section_name = SectionName::from(section.name.as_str());
            section_name_map.push(section_name);
        }
        section_name_maps.push(section_name_map);
    }

    // Calculate the first section offset in the final executable file,
    // which is after the file header and program headers.
    let mut program_header_count = BASE_PROGRAM_HEADER_COUNT;

    let contains_read_only_data = contains_read_only_data_section(&modules);
    let contains_writable_data = contains_writable_data_section(&modules);
    let contains_tls_data = contains_tls_data_section(&modules);

    if contains_read_only_data {
        program_header_count += 1;
    }

    if contains_writable_data {
        program_header_count += 1;
    }

    if contains_tls_data {
        program_header_count += 1;
    }

    let file_header_and_program_headers_size =
        ELF_HEADER_SIZE + program_header_count * PROGRAM_HEADER_ENTRY_SIZE;

    // Load architecture-specific parameters
    let load_address_base = get_load_address_base(arch);
    let segment_align_page_size = get_segment_align_page_size(arch);
    let section_align_text = get_section_align_text(arch);

    // The overall offset of the merged sections in the final executable file
    let mut file_offset: usize;

    // The overall virtual address of the merged sections in the final executable file
    // By default, the virtual address is calculated as `load_address_base + file_offset`,
    // but for the `.bss` and `.tbss` sections, which are NOBITS sections and do not occupy space in the file,
    // but occupy space in memory,
    // we need to calculate the virtual address separately.
    let mut virtual_address: usize;

    let mut merged_sectionss: Vec<HashMap<SectionName, MergedSection>> =
        vec![HashMap::new(); modules.len()];

    // merging code sections
    // ------------------------

    // `code` segment is page-aligned
    file_offset = align_up(
        file_header_and_program_headers_size,
        segment_align_page_size,
    );

    for ((section_name_map, module), merged_sections) in section_name_maps
        .iter()
        .zip(modules.iter())
        .zip(merged_sectionss.iter_mut())
    {
        if let Some(section_idx) = section_name_map
            .iter()
            .position(|&name| name == SectionName::Text)
        {
            let section = &module.sections[section_idx];
            file_offset = align_up(file_offset, section_align_text);

            let merged_section = MergedSection::new(
                section.size,
                &section.binary,
                file_offset,
                load_address_base + file_offset,
            );
            merged_sections.insert(SectionName::Text, merged_section);

            file_offset += section.size;
        }
    }

    // merge `.text` sections
    // let merged_section_offset_text = file_offset;
    for ((section_name_map, module), merged_sections) in section_name_maps
        .iter()
        .zip(modules.iter())
        .zip(merged_sectionss.iter_mut())
    {
        if let Some(section_idx) = section_name_map
            .iter()
            .position(|&name| name == SectionName::Text)
        {
            let section = &module.sections[section_idx];
            file_offset = align_up(file_offset, section_align_text);

            let merged_section = MergedSection::new(
                section.size,
                &section.binary,
                file_offset,
                load_address_base + file_offset,
            );
            merged_sections.insert(SectionName::Text, merged_section);

            file_offset += section.size;
        }
    }
    // let merged_section_size_text = file_offset - merged_section_offset_text;

    // merging read-only data sections
    // -------------------------------

    // `read-only data` segment is page-aligned
    file_offset = align_up(file_offset, segment_align_page_size);

    // merge `.rodata` sections
    // let merged_section_offset_rodata = file_offset;
    for ((section_name_map, module), merged_sections) in section_name_maps
        .iter()
        .zip(modules.iter())
        .zip(merged_sectionss.iter_mut())
    {
        if let Some(section_idx) = section_name_map
            .iter()
            .position(|&name| name == SectionName::ROData)
        {
            let section = &module.sections[section_idx];
            file_offset = align_up(file_offset, SECTION_ALIGN_DATA);

            let merged_section = MergedSection::new(
                section.size,
                &section.binary,
                file_offset,
                load_address_base + file_offset,
            );
            merged_sections.insert(SectionName::ROData, merged_section);

            file_offset += section.size;
        }
    }
    // let merged_section_size_rodata = file_offset - merged_section_offset_rodata;

    // merging all writable data sections
    // ----------------------------------

    // Note that the `.tdata`, `.tbss`, `.data`, and `.bss` sections will be merged into
    // one `writable data` segment, so we need to calculate their offsets and virtual addresses together.

    // `writable` segment is page-aligned
    file_offset = align_up(file_offset, segment_align_page_size);

    // merging `.tdata` sections
    // let merged_section_offset_tdata = file_offset;
    for ((section_name_map, module), merged_sections) in section_name_maps
        .iter()
        .zip(modules.iter())
        .zip(merged_sectionss.iter_mut())
    {
        if let Some(section_idx) = section_name_map
            .iter()
            .position(|&name| name == SectionName::TData)
        {
            let section = &module.sections[section_idx];
            file_offset = align_up(file_offset, SECTION_ALIGN_DATA);

            let merged_section = MergedSection::new(
                section.size,
                &section.binary,
                file_offset,
                load_address_base + file_offset,
            );
            merged_sections.insert(SectionName::TData, merged_section);

            file_offset += section.size;
        }
    }
    // let merged_section_size_tdata = file_offset - merged_section_offset_tdata;

    // data alignment
    file_offset = align_up(file_offset, SECTION_ALIGN_DATA);
    virtual_address = load_address_base + file_offset;

    // merging `.tbss` sections
    // let merged_section_virtual_address_tbss = virtual_address;
    for ((section_name_map, module), merged_sections) in section_name_maps
        .iter()
        .zip(modules.iter())
        .zip(merged_sectionss.iter_mut())
    {
        if let Some(section_idx) = section_name_map
            .iter()
            .position(|&name| name == SectionName::TBSS)
        {
            let section = &module.sections[section_idx];

            // Since `.tbss` is a NOBITS section, so it does not occupy space in the file,
            // but it does occupy space in memory.
            // Therefore, we need to increase the `virtual_address` by
            // the size of the `.tbss` section, but we do not increase the `file_offset`.
            virtual_address = align_up(virtual_address, SECTION_ALIGN_DATA);

            let merged_section = MergedSection::new_bss(section.size, file_offset, virtual_address);
            merged_sections.insert(SectionName::TBSS, merged_section);

            virtual_address += section.size;
        }
    }
    // let merged_section_size_tbss = virtual_address - merged_section_virtual_address_tbss;

    // data alignment
    file_offset = align_up(file_offset, SECTION_ALIGN_DATA);
    virtual_address = align_up(virtual_address, SECTION_ALIGN_DATA);

    // merging `.data` sections
    // let merged_section_virtual_address_data = virtual_address;
    for ((section_name_map, module), merged_sections) in section_name_maps
        .iter()
        .zip(modules.iter())
        .zip(merged_sectionss.iter_mut())
    {
        if let Some(section_idx) = section_name_map
            .iter()
            .position(|&name| name == SectionName::Data)
        {
            let section = &module.sections[section_idx];

            // Both `file_offset` and `virtual_address` need to be accumulated.
            file_offset = align_up(file_offset, SECTION_ALIGN_DATA);
            virtual_address = align_up(virtual_address, SECTION_ALIGN_DATA);

            let merged_section =
                MergedSection::new(section.size, &section.binary, file_offset, virtual_address);
            merged_sections.insert(SectionName::Data, merged_section);

            // Both `file_offset` and `virtual_address` need to be accumulated.
            file_offset += section.size;
            virtual_address += section.size;
        }
    }
    // let merged_section_size_data = virtual_address - merged_section_virtual_address_data;

    // The linker-generated symbol `_edata` points to the end of the initialized data.
    let symbol_edata_offset = file_offset;
    let symbol_edata_virtual_address = virtual_address;

    // data alignment
    file_offset = align_up(file_offset, SECTION_ALIGN_DATA);
    virtual_address = align_up(virtual_address, SECTION_ALIGN_DATA);

    // The linker-generated symbol `__bss_start` points to the start of the uninitialized data.
    let symbol_bss_start_offset = file_offset;
    let symbol_bss_start_virtual_address = virtual_address;

    // merging `.bss`
    // let merged_section_virtual_address_bss = virtual_address;
    for ((section_name_map, module), merged_sections) in section_name_maps
        .iter()
        .zip(modules.iter())
        .zip(merged_sectionss.iter_mut())
    {
        if let Some(section_idx) = section_name_map
            .iter()
            .position(|&name| name == SectionName::BSS)
        {
            let section = &module.sections[section_idx];

            // Similar to `.tbss`, since `.bss` is a NOBITS section, so it does not occupy space in the file,
            // but it does occupy space in memory.
            // Therefore, we need to increase the `virtual_address` by
            // the size of the `.bss` section, but we do not increase the `file_offset`.
            virtual_address = align_up(virtual_address, SECTION_ALIGN_DATA);

            let merged_section = MergedSection::new_bss(section.size, file_offset, virtual_address);
            merged_sections.insert(SectionName::BSS, merged_section);

            virtual_address += section.size;
        }
    }
    // let merged_section_size_bss = virtual_address - merged_section_virtual_address_bss;

    // The linker-generated symbol `_end` points to the end of the uninitialized data.
    let symbol_end_offset = file_offset;
    let symbol_end_virtual_address = virtual_address;

    // Create global symbol map for all modules
    let mut global_symbol_map: HashMap<String, GlobalSymbolMapEntry> = HashMap::new();

    // Add linker-generated symbols to the global symbol map
    global_symbol_map.insert(
        "_edata".to_string(),
        GlobalSymbolMapEntry::new(
            /* symbol_edata_offset, */ symbol_edata_virtual_address,
            false,
        ),
    );
    global_symbol_map.insert(
        "__bss_start".to_string(),
        GlobalSymbolMapEntry::new(
            /* symbol_bss_start_offset, */
            symbol_bss_start_virtual_address,
            false,
        ),
    );
    global_symbol_map.insert(
        "_end".to_string(),
        GlobalSymbolMapEntry::new(
            /* symbol_end_offset, */ symbol_end_virtual_address,
            false,
        ),
    );

    // Resolve symbols file offset and virtual address
    // -----------------------------------------------

    enum ResolvedSymbol {
        Defined {
            /// The name of the symbol
            /// This name may be empty for symbols that represent sections
            /// (e.g. the symbol which represents a section).
            name: String,

            /// The binding of the symbol, which determines the linkage of the symbol.
            bind: SymbolBind,

            /// The section that the symbol belongs to.
            section_name: SectionName,

            // /// The offset of the symbol in the merged section in the final executable,
            // offset: usize,
            /// The virtual address of the symbol in the merged section in the final executable,
            virtual_address: usize,
        },
        External(/* name */ String),
        Other,
    }

    let mut resolved_symbolss: Vec<Vec<ResolvedSymbol>> = vec![];

    for (((module, section_name_map), merged_sections), resolved_symbols) in modules
        .iter()
        .zip(section_name_maps.iter())
        .zip(merged_sectionss.iter())
        .zip(resolved_symbolss.iter_mut())
    {
        for symbol in &module.symbols {
            match symbol {
                Symbol::Defined {
                    name,
                    bind,
                    section_index,
                    offset,
                    ..
                } => {
                    let section_name = section_name_map[*section_index];
                    if section_name == SectionName::Other {
                        // The symbol is not in the relevant sections
                        return Err(LinkerError::Message(format!(
                            "Symbol {} is defined in an unsupported section in module {}",
                            name, module.name
                        )));
                    }

                    let merged_section = merged_sections.get(&section_name);
                    match merged_section {
                        Some(merged_section) => {
                            let resolved_symbol = ResolvedSymbol::Defined {
                                name: name.clone(),
                                bind: *bind,
                                section_name,
                                // offset: *offset + merged_section.offset_in_file,
                                virtual_address: *offset + merged_section.virtual_address,
                            };
                            resolved_symbols.push(resolved_symbol);
                        }
                        None => {
                            return Err(LinkerError::Message(format!(
                                "Section {:?} not found in module {} for symbol {}",
                                section_name, module.name, name
                            )));
                        }
                    }
                }
                Symbol::External(name) => {
                    resolved_symbols.push(ResolvedSymbol::External(name.clone()));
                }
                Symbol::Other => {
                    resolved_symbols.push(ResolvedSymbol::Other);
                }
            }
        }
    }

    // Extract global symbols from all modules
    for (resolved_symbols, module) in resolved_symbolss.iter().zip(modules.iter()) {
        for resolved_symbol in resolved_symbols {
            if let ResolvedSymbol::Defined {
                name,
                bind,
                // offset,
                virtual_address,
                ..
            } = resolved_symbol
            {
                match bind {
                    SymbolBind::Global => {
                        if global_symbol_map.contains_key(name) {
                            return Err(LinkerError::Message(format!(
                                "Duplicate global symbol: {} defined in module {}",
                                name, module.name
                            )));
                        }
                        global_symbol_map.insert(
                            name.clone(),
                            GlobalSymbolMapEntry::new(/* *offset, */ *virtual_address, false),
                        );
                    }
                    SymbolBind::Weak => {
                        if !global_symbol_map.contains_key(name) {
                            global_symbol_map.insert(
                                name.clone(),
                                GlobalSymbolMapEntry::new(
                                    /* *offset, */ *virtual_address,
                                    true,
                                ),
                            );
                        }
                    }
                    _ => {
                        // Local symbols are not added to the global symbol map
                    }
                }
            }
        }
    }

    // Translate ResolvedSymbol to MergedSymbol
    let mut merged_symbolss: Vec<Vec<MergedSymbol>> = vec![];
    for (resolved_symbols, module) in resolved_symbolss.iter().zip(modules.iter()) {
        let mut merged_symbols = Vec::new();
        for resolved_symbol in resolved_symbols {
            match resolved_symbol {
                ResolvedSymbol::Defined {
                    // offset,
                    virtual_address,
                    ..
                } => {
                    let merged_symbol = MergedSymbol::Effective {
                        // offset_in_section: *offset,
                        virtual_address: *virtual_address,
                    };
                    merged_symbols.push(merged_symbol);
                }
                ResolvedSymbol::External(name) => {
                    // Look up the symbol in the global symbol map
                    if let Some(global_symbol) = global_symbol_map.get(name) {
                        let merged_symbol = MergedSymbol::Effective {
                            // offset_in_section: global_symbol.offset_in_section,
                            virtual_address: global_symbol.virtual_address,
                        };
                        merged_symbols.push(merged_symbol);
                    } else {
                        return Err(LinkerError::Message(format!(
                            "Unresolved external symbol: {} in module {}",
                            name, module.name
                        )));
                    }
                }
                ResolvedSymbol::Other => {
                    merged_symbols.push(MergedSymbol::Other);
                }
            }
        }
        merged_symbolss.push(merged_symbols);
    }

    // Translate relocation entries MergedRelocationSection
    let mut merged_relocation_sectionss: Vec<Vec<MergedRelocationSection>> = vec![];
    for ((module, section_name_map), merged_sections) in modules
        .into_iter()
        .zip(section_name_maps.iter())
        .zip(merged_sectionss.iter())
    {
        let mut merged_relocation_sections = Vec::new();
        for relocation_section in module.relocation_sections {
            let target_section_name = section_name_map[relocation_section.target_section_index];
            if target_section_name == SectionName::Other {
                return Err(LinkerError::Message(format!(
                    "Relocation section {:?} in module {} targets an unsupported section",
                    relocation_section.target_section_index, module.name
                )));
            }

            let merged_section = merged_sections.get(&target_section_name);
            match merged_section {
                Some(_) => {
                    let merged_relocation_section = MergedRelocationSection {
                        target_section_name,
                        relocations: relocation_section.relocations,
                    };
                    merged_relocation_sections.push(merged_relocation_section);
                }
                None => {
                    return Err(LinkerError::Message(format!(
                        "Section {:?} not found in module {} for relocation section",
                        target_section_name, module.name
                    )));
                }
            }
        }
        merged_relocation_sectionss.push(merged_relocation_sections);
    }

    // Refactor the merged sections, symbols, and relocation sections into a single MergedModule(s)
    let merged_modules: Vec<MergedModule> = merged_sectionss
        .into_iter()
        .zip(merged_symbolss.into_iter())
        .zip(merged_relocation_sectionss.into_iter())
        .map(|((sections, symbols), relocation_sections)| MergedModule {
            sections,
            symbols,
            relocation_sections,
        })
        .collect();

    // Find the entry point symbol `_start` and get its virtual address.
    let entry_point = if let Some(entry_symbol) = global_symbol_map.get("_start") {
        entry_symbol.virtual_address
    } else {
        return Err(LinkerError::Message(
            "Entry point symbol `_start` not found in the global symbols".to_string(),
        ));
    };

    let merge_result = MergeResult {
        contains_read_only_data,
        contains_writable_data,
        contains_tls_data,
        program_header_count,
        entry_point,
        merged_modules,
        global_symbol_map,
    };

    Ok(merge_result)
}
