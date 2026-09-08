// Copyright (c) 2026 Hemashushu <hippospark@gmail.com>, All rights reserved.
//
// This Source Code Form is subject to the terms of
// the Mozilla Public License version 2.0 and additional exceptions.
// For more details, see the LICENSE, LICENSE.additional, and CONTRIBUTING files.

use std::{
    collections::{HashMap, HashSet},
    fmt::Display,
};

use crate::{
    elf::module::{
        BASE_PROGRAM_HEADER_COUNT, ELF_HEADER_SIZE, Machine, PROGRAM_HEADER_ENTRY_SIZE,
        RelocatableModule, Relocation, RelocationType, SECTION_ALIGN_DATA, SECTION_NAME_BSS,
        SECTION_NAME_DATA, SECTION_NAME_RODATA, SECTION_NAME_TBSS, SECTION_NAME_TDATA,
        SECTION_NAME_TEXT, SECTION_NAME_TOC, Symbol, SymbolBind, SymbolType, get_load_address_base,
        get_section_align_text, get_segment_align_page_size,
    },
    error::LinkerError,
};

/// A merged module represents essential elements of an object file,
/// which are intended to be merged into a single executable file.
///
/// The `MergedModule` is part of an object file,
/// and it is not a complete representation of all the details of an object file.
/// It assumes that an object file contains only:
///
/// - At most one:
///   - code section `.text`
///   - read-only data section `.rodata`
///   - thread local data section `.tdata`
///   - thread local uninitialized section `.tbss`
///   - data section `.data`
///   - uninitialized data section `.bss`
///   - TOC section `.toc` (for PowerPC64)
///   - symbol table `.symtab`
///   - relocation table `.rela.text`
///   - relocation table `.rela.rodata`
///   - relocation table `.rela.data`
///   - relocation table `.rela.tdata`
///   - relocation table `.rela.toc`
///   - string table `.strtab` (for symbol names)
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
pub struct FragmentModule<'a> {
    /// The identifier or name of the module, typically derived from the input file name.
    pub name: String,

    /// The relevant sections of the module
    pub sections: HashMap<SectionName, FragmentSection<'a>>,

    /// The symbol table of the module, which contains the symbols defined in the module.
    pub symbols: Vec<MergedSymbol>,

    /// The relocation entries of the module, which contain the information about
    /// how to adjust the code and data when linking.
    pub relocation_sections: Vec<FragmentRelocationSection>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SectionName {
    Text,

    ROData,

    TData,

    #[allow(clippy::upper_case_acronyms)]
    TBSS,

    Data,

    #[allow(clippy::upper_case_acronyms)]
    BSS,

    #[allow(clippy::upper_case_acronyms)]
    TOC, // PowerPC64 TOC or synthetic static GOT section

    Other, // Other sections that are not relevant to the final executable
}

/// Section represents a section in the merged module
#[derive(Debug, PartialEq, Clone)]
pub struct FragmentSection<'a> {
    /// The size of the section.
    /// For the `.bss` and `.tbss` sections, this is the memory size of the section,
    /// which is not present in the file, but occupies space in memory.
    pub size: usize,

    /// The binary data of the section.
    ///
    /// Input `.text`, `.rodata`, `.tdata`, `.data`, and `.toc` sections contain
    /// binary data. Synthetic sections, such as the static GOT stored in `.toc`,
    /// may own generated binary data. `.bss` and `.tbss` do not contain binary
    /// data in the object file.
    pub binary: FragmentSectionBinary<'a>,

    // The offset of the fragment sections in the merged section in the runtime memory of the final executable file
    //
    // Note that the offset in the merged section is different from the offset in the merged file,
    // because the `.bss` and `.tbss` sections are NOBITS sections, which do not occupy space in the file,
    // but occupy space in memory, so the offset in the merged section is affected by the size of `.bss` and `.tbss`.
    pub offset_in_merged_section: usize,

    /// The section offset in the final executable
    pub offset_in_merged_file: usize,

    /// The virtual address of the section in the runtime memory of the final executable file.
    ///
    /// For most sections, `virtual address = load address + section offset`,
    /// but start from the `.data` section, the virtual address is also affected by the
    /// size of the previous section `.bss` (which is not present in the file, but occupies space in memory).
    pub virtual_address: usize,
}

impl<'a> FragmentSection<'a> {
    pub fn new(
        size: usize,
        binary: &'a [u8],
        offset_in_merged_section: usize,
        offset_in_merged_file: usize,
        virtual_address: usize,
    ) -> Self {
        FragmentSection {
            size,
            binary: FragmentSectionBinary::Referenced(binary),
            offset_in_merged_section,
            offset_in_merged_file,
            virtual_address,
        }
    }

    pub fn new_bss(
        size: usize,
        offset_in_merged_section: usize,
        offset_in_merged_file: usize,
        virtual_address: usize,
    ) -> Self {
        FragmentSection {
            size,
            binary: FragmentSectionBinary::None,
            offset_in_merged_section,
            offset_in_merged_file,
            virtual_address,
        }
    }

    pub fn new_owned(
        binary: Vec<u8>,
        offset_in_merged_section: usize,
        offset_in_merged_file: usize,
        virtual_address: usize,
    ) -> Self {
        let size = binary.len();
        FragmentSection {
            size,
            binary: FragmentSectionBinary::Owned(binary),
            offset_in_merged_section,
            offset_in_merged_file,
            virtual_address,
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum FragmentSectionBinary<'a> {
    Referenced(&'a [u8]),
    Owned(Vec<u8>),
    None,
}

#[derive(Debug, PartialEq, Clone)]
pub enum MergedSymbol {
    Defined {
        /// The name of the symbol
        /// This name may be empty for symbols that represent sections
        /// (e.g. the symbol which represents a section).
        name: String,

        /// The binding of the symbol, which determines the linkage of the symbol.
        bind: SymbolBind,

        /// The section where the symbol is located in the final executable.
        section_name: SectionName,

        /// The offset of the symbol in the merged section in the final executable,
        offset_in_merged_section: usize,

        /// The virtual address of the symbol in the merged section in the final executable,
        virtual_address: usize,
    },
    Absolute {
        /// The name of the symbol
        name: String,

        /// The binding of the symbol, which determines the linkage of the symbol.
        bind: SymbolBind,

        /// The absolute value
        value: u64,
    },
    External(/* name */ String),

    /// Symbols that the linker does not care about.
    Other,
}

#[derive(Debug, PartialEq, Clone)]
pub struct GlobalSymbolMapEntry {
    pub value: GlobalSymbolValue,
    pub is_weak: bool,
}

#[derive(Debug, PartialEq, Clone)]
pub enum GlobalSymbolValue {
    Defined {
        section_name: SectionName,
        virtual_address: usize,
    },
    Absolute(u64),
}

impl GlobalSymbolMapEntry {
    pub fn new(value: GlobalSymbolValue, is_weak: bool) -> Self {
        GlobalSymbolMapEntry { value, is_weak }
    }
}

impl GlobalSymbolValue {
    pub fn from_defined(section_name: SectionName, virtual_address: usize) -> Self {
        GlobalSymbolValue::Defined {
            section_name,
            virtual_address,
        }
    }

    pub fn from_absolute(value: u64) -> Self {
        GlobalSymbolValue::Absolute(value)
    }
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
            SECTION_NAME_TOC => SectionName::TOC,
            _ => SectionName::Other,
        }
    }
}

impl Display for SectionName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            SectionName::Text => SECTION_NAME_TEXT,
            SectionName::ROData => SECTION_NAME_RODATA,
            SectionName::TData => SECTION_NAME_TDATA,
            SectionName::TBSS => SECTION_NAME_TBSS,
            SectionName::Data => SECTION_NAME_DATA,
            SectionName::BSS => SECTION_NAME_BSS,
            SectionName::TOC => SECTION_NAME_TOC,
            SectionName::Other => "other",
        };
        write!(f, "{}", name)
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct FragmentRelocationSection {
    /// The section which these relocations apply to.
    ///
    /// Currently, only the following sections are supported:
    /// - Text
    /// - RoData
    /// - Data
    /// - TData
    /// - TOC
    pub target_section_name: SectionName,

    /// The relocation entries in this section.
    pub relocations: Vec<Relocation>,
}

fn contains_read_only_data_section(modules: &[RelocatableModule]) -> bool {
    modules.iter().any(|module| {
        module
            .sections
            .iter()
            .any(|s| s.name == SECTION_NAME_RODATA && s.size > 0)
    })
}

fn contains_writable_data_section(modules: &[RelocatableModule]) -> bool {
    modules.iter().any(|module| {
        module.sections.iter().any(|section| {
            matches!(
                section.name.as_str(),
                SECTION_NAME_DATA | SECTION_NAME_BSS | SECTION_NAME_TOC
            ) && section.size > 0
        })
    }) || contains_tls_data_section(modules)
}

fn contains_tls_data_section(modules: &[RelocatableModule]) -> bool {
    modules.iter().any(|module| {
        module.sections.iter().any(|section| {
            matches!(
                section.name.as_str(),
                SECTION_NAME_TDATA | SECTION_NAME_TBSS
            ) && section.size > 0
        })
    })
}

fn align_up(val: usize, align: usize) -> usize {
    (val + align - 1) & !(align - 1)
}

#[derive(Debug, PartialEq)]
pub struct MergedSectionInfo {
    /// The offset of the section in the final executable file.
    pub offset_in_merged_file: usize,

    /// The virtual address of the section in the runtime memory of the final executable file.
    pub virtual_address: usize,

    /// The size of the section in the final executable file.
    /// For the `.bss` and `.tbss` sections, this is the memory size of the section,
    /// which is not present in the file, but occupies space in memory.
    pub size: usize,
}

impl MergedSectionInfo {
    pub fn new(offset_in_merged_file: usize, virtual_address: usize, size: usize) -> Self {
        MergedSectionInfo {
            offset_in_merged_file,
            virtual_address,
            size,
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct MergedFileLayout {
    pub contains_read_only_data: bool,

    pub contains_writable_data: bool,

    /// Indicates whether the final executable contains TLS segments.
    pub contains_tls_data: bool,

    /// The number of program headers in the final executable.
    ///
    /// In general, there are `PHDR, metadata, code, read-only data, writable data` five program headers,
    /// and an additional TLS segment if there is TLS.
    pub program_header_count: usize,

    pub merged_section_infos: HashMap<SectionName, MergedSectionInfo>,
}

impl MergedFileLayout {
    /// Checks if the merged file layout contains a section with non-empty size for the given section name.
    pub fn contains_non_empty_section(&self, section_name: SectionName) -> bool {
        if let Some(section) = self.merged_section_infos.get(&section_name) {
            section.size > 0
        } else {
            false
        }
    }

    pub fn get_non_empty_section_info(
        &self,
        section_name: SectionName,
    ) -> Option<&MergedSectionInfo> {
        self.merged_section_infos
            .get(&section_name)
            .filter(|s| s.size > 0)
    }
}

#[derive(Debug, PartialEq)]
pub struct MergedAsset<'a> {
    pub fragment_modules: Vec<FragmentModule<'a>>,

    pub linker_generated_symbols: HashMap<String, GlobalSymbolMapEntry>,

    pub merged_file_layout: MergedFileLayout,
}

pub fn merge<'a>(
    modules: Vec<RelocatableModule<'a>>,
    arch: Machine,
) -> Result<MergedAsset<'a>, LinkerError> {
    // Load architecture-specific parameters
    #[allow(non_snake_case)]
    let LOAD_ADDR_BASE = get_load_address_base(arch);

    #[allow(non_snake_case)]
    let SEGMENT_ALIGN_PAGE_SIZE = get_segment_align_page_size(arch);

    #[allow(non_snake_case)]
    let SECTION_ALIGN_TEXT = get_section_align_text(arch);

    // Store the module names for later use
    let module_names: Vec<String> = modules.iter().map(|m| m.name.clone()).collect();

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

    // The offset of the fragment sections in the final executable file
    let mut offset_in_merged_file: usize;

    // The offset of the fragment sections in the merged section in the runtime memory of the final executable file
    //
    // Note that the offset in the merged section is different from the offset in the merged file,
    // because the `.bss` and `.tbss` sections are NOBITS sections, which do not occupy space in the file,
    // but occupy space in memory, so the offset in the merged section is affected by the size of `.bss` and `.tbss`.
    let mut offset_in_merged_section: usize;

    // The overall virtual address of the merged sections in the final executable file
    // By default, the virtual address is calculated as `load_address_base + file_offset`,
    // but for the `.bss` and `.tbss` sections, which are NOBITS sections and do not occupy space in the file,
    // but occupy space in memory,
    // we need to calculate the virtual address separately.
    let mut virtual_address: usize;

    let mut fragment_sectionss: Vec<HashMap<SectionName, FragmentSection<'a>>> =
        vec![HashMap::new(); modules.len()];

    let mut merged_section_infos: HashMap<SectionName, MergedSectionInfo> = HashMap::new();

    // merging code sections
    // ------------------------

    // `code` segment is page-aligned
    offset_in_merged_file = align_up(
        file_header_and_program_headers_size,
        SEGMENT_ALIGN_PAGE_SIZE,
    );

    // reset the offset in the merged section to 0, since we are starting a new merged section for `.text`
    offset_in_merged_section = 0;

    // merge `.text` sections
    let merged_section_offset_text = offset_in_merged_file;
    for ((section_name_map, module), fragment_sections) in section_name_maps
        .iter()
        .zip(modules.iter())
        .zip(fragment_sectionss.iter_mut())
    {
        if let Some(section_idx) = section_name_map
            .iter()
            .position(|&name| name == SectionName::Text)
        {
            let section = &module.sections[section_idx];
            offset_in_merged_file = align_up(offset_in_merged_file, SECTION_ALIGN_TEXT);

            let fragment_section = FragmentSection::new(
                section.size,
                section.binary,
                offset_in_merged_section,
                offset_in_merged_file,
                LOAD_ADDR_BASE + offset_in_merged_file,
            );
            fragment_sections.insert(SectionName::Text, fragment_section);

            offset_in_merged_file += section.size;
            offset_in_merged_section += section.size;
        }
    }

    let merged_section_size_text = offset_in_merged_file - merged_section_offset_text;
    merged_section_infos.insert(
        SectionName::Text,
        MergedSectionInfo::new(
            merged_section_offset_text,
            LOAD_ADDR_BASE + merged_section_offset_text,
            merged_section_size_text,
        ),
    );

    // merging read-only data sections
    // -------------------------------

    // `read-only data` segment is page-aligned
    offset_in_merged_file = align_up(offset_in_merged_file, SEGMENT_ALIGN_PAGE_SIZE);
    offset_in_merged_section = 0; // reset

    // merge `.rodata` sections
    let merged_section_offset_rodata = offset_in_merged_file;
    for ((section_name_map, module), fragment_sections) in section_name_maps
        .iter()
        .zip(modules.iter())
        .zip(fragment_sectionss.iter_mut())
    {
        if let Some(section_idx) = section_name_map
            .iter()
            .position(|&name| name == SectionName::ROData)
        {
            let section = &module.sections[section_idx];
            offset_in_merged_file = align_up(offset_in_merged_file, SECTION_ALIGN_DATA);

            let fragment_section = FragmentSection::new(
                section.size,
                section.binary,
                offset_in_merged_section,
                offset_in_merged_file,
                LOAD_ADDR_BASE + offset_in_merged_file,
            );
            fragment_sections.insert(SectionName::ROData, fragment_section);

            offset_in_merged_file += section.size;
            offset_in_merged_section += section.size;
        }
    }

    let merged_section_size_rodata = offset_in_merged_file - merged_section_offset_rodata;
    merged_section_infos.insert(
        SectionName::ROData,
        MergedSectionInfo::new(
            merged_section_offset_rodata,
            LOAD_ADDR_BASE + merged_section_offset_rodata,
            merged_section_size_rodata,
        ),
    );

    // merging all writable data sections
    // ----------------------------------

    // Note that the `.tdata`, `.tbss`, `.data`, and `.bss` sections will be merged into
    // one `writable data` segment, so we need to calculate their offsets and virtual addresses together.

    // `writable` segment is page-aligned
    offset_in_merged_file = align_up(offset_in_merged_file, SEGMENT_ALIGN_PAGE_SIZE);
    offset_in_merged_section = 0; // reset

    // merging `.tdata` sections
    let merged_section_offset_tdata = offset_in_merged_file;
    for ((section_name_map, module), fragment_sections) in section_name_maps
        .iter()
        .zip(modules.iter())
        .zip(fragment_sectionss.iter_mut())
    {
        if let Some(section_idx) = section_name_map
            .iter()
            .position(|&name| name == SectionName::TData)
        {
            let section = &module.sections[section_idx];
            offset_in_merged_file = align_up(offset_in_merged_file, SECTION_ALIGN_DATA);

            let fragment_section = FragmentSection::new(
                section.size,
                section.binary,
                offset_in_merged_section,
                offset_in_merged_file,
                LOAD_ADDR_BASE + offset_in_merged_file,
            );
            fragment_sections.insert(SectionName::TData, fragment_section);

            offset_in_merged_file += section.size;
            offset_in_merged_section += section.size;
        }
    }

    let merged_section_size_tdata = offset_in_merged_file - merged_section_offset_tdata;
    merged_section_infos.insert(
        SectionName::TData,
        MergedSectionInfo::new(
            merged_section_offset_tdata,
            LOAD_ADDR_BASE + merged_section_offset_tdata,
            merged_section_size_tdata,
        ),
    );

    // data alignment
    offset_in_merged_file = align_up(offset_in_merged_file, SECTION_ALIGN_DATA);
    offset_in_merged_section = 0; // reset
    virtual_address = LOAD_ADDR_BASE + offset_in_merged_file;

    // merging `.tbss` sections
    let merged_section_offset_tbss = offset_in_merged_file;
    let merged_section_virtual_address_tbss = virtual_address;

    for ((section_name_map, module), fragment_sections) in section_name_maps
        .iter()
        .zip(modules.iter())
        .zip(fragment_sectionss.iter_mut())
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

            let fragment_section = FragmentSection::new_bss(
                section.size,
                offset_in_merged_section,
                offset_in_merged_file,
                virtual_address,
            );
            fragment_sections.insert(SectionName::TBSS, fragment_section);

            virtual_address += section.size;
            offset_in_merged_section += section.size;
        }
    }

    let merged_section_size_tbss = virtual_address - merged_section_virtual_address_tbss;
    merged_section_infos.insert(
        SectionName::TBSS,
        MergedSectionInfo::new(
            merged_section_offset_tbss,
            merged_section_virtual_address_tbss,
            merged_section_size_tbss,
        ),
    );

    // data alignment
    offset_in_merged_file = align_up(offset_in_merged_file, SECTION_ALIGN_DATA);
    offset_in_merged_section = 0; // reset
    virtual_address = align_up(virtual_address, SECTION_ALIGN_DATA);

    // merging `.data` sections
    let merged_section_offset_data = offset_in_merged_file;
    let merged_section_virtual_address_data = virtual_address;

    for ((section_name_map, module), fragment_sections) in section_name_maps
        .iter()
        .zip(modules.iter())
        .zip(fragment_sectionss.iter_mut())
    {
        if let Some(section_idx) = section_name_map
            .iter()
            .position(|&name| name == SectionName::Data)
        {
            let section = &module.sections[section_idx];

            // Both `file_offset` and `virtual_address` need to be accumulated.
            offset_in_merged_file = align_up(offset_in_merged_file, SECTION_ALIGN_DATA);
            virtual_address = align_up(virtual_address, SECTION_ALIGN_DATA);

            let fragment_section = FragmentSection::new(
                section.size,
                section.binary,
                offset_in_merged_section,
                offset_in_merged_file,
                virtual_address,
            );
            fragment_sections.insert(SectionName::Data, fragment_section);

            // Both `file_offset` and `virtual_address` need to be accumulated.
            offset_in_merged_file += section.size;
            offset_in_merged_section += section.size;
            virtual_address += section.size;
        }
    }

    let merged_section_size_data = virtual_address - merged_section_virtual_address_data;
    merged_section_infos.insert(
        SectionName::Data,
        MergedSectionInfo::new(
            merged_section_offset_data,
            merged_section_virtual_address_data,
            merged_section_size_data,
        ),
    );

    // The linker-generated symbol `_edata` points to the end of the initialized data.
    let symbol_edata_virtual_address = virtual_address;

    // data alignment
    offset_in_merged_file = align_up(offset_in_merged_file, SECTION_ALIGN_DATA);
    offset_in_merged_section = 0; // reset
    virtual_address = align_up(virtual_address, SECTION_ALIGN_DATA);

    // Merge PowerPC64 TOC sections after initialized data and before BSS.
    let merged_section_offset_toc = offset_in_merged_file;
    let merged_section_virtual_address_toc = virtual_address;

    for ((section_name_map, module), fragment_sections) in section_name_maps
        .iter()
        .zip(modules.iter())
        .zip(fragment_sectionss.iter_mut())
    {
        if let Some(section_idx) = section_name_map
            .iter()
            .position(|&name| name == SectionName::TOC)
        {
            let section = &module.sections[section_idx];

            // Both `file_offset` and `virtual_address` need to be accumulated.
            offset_in_merged_file = align_up(offset_in_merged_file, SECTION_ALIGN_DATA);
            virtual_address = align_up(virtual_address, SECTION_ALIGN_DATA);

            let fragment_section = FragmentSection::new(
                section.size,
                section.binary,
                offset_in_merged_section,
                offset_in_merged_file,
                virtual_address,
            );
            fragment_sections.insert(SectionName::TOC, fragment_section);

            // Both `file_offset` and `virtual_address` need to be accumulated.
            offset_in_merged_file += section.size;
            offset_in_merged_section += section.size;
            virtual_address += section.size;
        } else if arch == Machine::LoongArch {
            let mut got_symbols = HashSet::new();
            for relocation_section in &module.relocation_sections {
                for relocation in &relocation_section.relocations {
                    if matches!(
                        relocation.relocation_type,
                        RelocationType::R_LARCH_GOT_PC_HI20 | RelocationType::R_LARCH_GOT_PC_LO12
                    ) {
                        got_symbols.insert(relocation.symbol_index);
                    }
                }
            }

            if !got_symbols.is_empty() {
                let binary = vec![0; got_symbols.len() * 8];
                offset_in_merged_file = align_up(offset_in_merged_file, SECTION_ALIGN_DATA);
                virtual_address = align_up(virtual_address, SECTION_ALIGN_DATA);
                let fragment_section = FragmentSection::new_owned(
                    binary,
                    offset_in_merged_section,
                    offset_in_merged_file,
                    virtual_address,
                );
                fragment_sections.insert(SectionName::TOC, fragment_section);
                offset_in_merged_file += got_symbols.len() * 8;
                offset_in_merged_section += got_symbols.len() * 8;
                virtual_address += got_symbols.len() * 8;
            }
        }
    }

    let merged_section_size_toc = virtual_address - merged_section_virtual_address_toc;
    merged_section_infos.insert(
        SectionName::TOC,
        MergedSectionInfo::new(
            merged_section_offset_toc,
            merged_section_virtual_address_toc,
            merged_section_size_toc,
        ),
    );

    // data alignment
    offset_in_merged_file = align_up(offset_in_merged_file, SECTION_ALIGN_DATA);
    offset_in_merged_section = 0; // reset
    virtual_address = align_up(virtual_address, SECTION_ALIGN_DATA);

    // The linker-generated symbol `__bss_start` points to the start of the uninitialized data.
    let symbol_bss_start_virtual_address = virtual_address;

    // merging `.bss`
    let merged_section_offset_bss = offset_in_merged_file;
    let merged_section_virtual_address_bss = virtual_address;

    for ((section_name_map, module), fragment_sections) in section_name_maps
        .iter()
        .zip(modules.iter())
        .zip(fragment_sectionss.iter_mut())
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

            let fragment_section = FragmentSection::new_bss(
                section.size,
                offset_in_merged_section,
                offset_in_merged_file,
                virtual_address,
            );
            fragment_sections.insert(SectionName::BSS, fragment_section);

            virtual_address += section.size;
            offset_in_merged_section += section.size;
        }
    }

    let merged_section_size_bss = virtual_address - merged_section_virtual_address_bss;
    merged_section_infos.insert(
        SectionName::BSS,
        MergedSectionInfo::new(
            merged_section_offset_bss,
            merged_section_virtual_address_bss,
            merged_section_size_bss,
        ),
    );

    // The linker-generated symbol `_end` points to the end of the uninitialized data.
    let symbol_end_virtual_address = virtual_address;

    // Create global symbol map for all modules
    let mut linker_generated_symbols: HashMap<String, GlobalSymbolMapEntry> = HashMap::new();

    // Add linker-generated symbols to the global symbol map
    linker_generated_symbols.insert(
        "_edata".to_string(),
        GlobalSymbolMapEntry::new(
            GlobalSymbolValue::from_defined(SectionName::Data, symbol_edata_virtual_address),
            false,
        ),
    );
    linker_generated_symbols.insert(
        "__bss_start".to_string(),
        GlobalSymbolMapEntry::new(
            GlobalSymbolValue::from_defined(SectionName::BSS, symbol_bss_start_virtual_address),
            false,
        ),
    );
    linker_generated_symbols.insert(
        "_end".to_string(),
        GlobalSymbolMapEntry::new(
            GlobalSymbolValue::from_defined(SectionName::BSS, symbol_end_virtual_address),
            false,
        ),
    );

    // Resolve symbols file offset and virtual address
    // -----------------------------------------------

    let mut merged_symbolss: Vec<Vec<MergedSymbol>> = vec![Vec::new(); modules.len()];

    for (((module, section_name_map), fragment_sections), merged_symbols) in modules
        .iter()
        .zip(section_name_maps.iter())
        .zip(fragment_sectionss.iter())
        .zip(merged_symbolss.iter_mut())
    {
        for symbol in &module.symbols {
            match symbol {
                Symbol::Defined {
                    name,
                    bind,
                    symbol_type,
                    section_index,
                    value,
                    ..
                } => {
                    let section_name = section_name_map[*section_index];
                    if section_name == SectionName::Other {
                        if symbol_type == &SymbolType::Section {
                            // The symbol represents a section, which is not relevant to the final executable
                            merged_symbols.push(MergedSymbol::Other);
                        } else {
                            // The symbol is not in the relevant sections
                            return Err(LinkerError::Message(format!(
                                "Symbol \"{}\" is defined in an unsupported section in module {}",
                                name, module.name
                            )));
                        }
                    } else if let Some(fragment_section) = fragment_sections.get(&section_name) {
                        let original_value = *value as usize;
                        let merged_symbol = MergedSymbol::Defined {
                            name: name.clone(),
                            bind: *bind,
                            section_name,
                            offset_in_merged_section: original_value
                                + fragment_section.offset_in_merged_section,
                            virtual_address: original_value + fragment_section.virtual_address,
                        };
                        merged_symbols.push(merged_symbol);
                    } else {
                        return Err(LinkerError::Message(format!(
                            "Section \"{}\" not found in module \"{}\" for symbol \"{}\"",
                            section_name, module.name, name
                        )));
                    }
                }
                Symbol::Absolute {
                    name, bind, value, ..
                } => {
                    let merged_symbol = MergedSymbol::Absolute {
                        name: name.clone(),
                        bind: *bind,
                        value: *value,
                    };
                    merged_symbols.push(merged_symbol);
                }
                Symbol::External(name) => {
                    merged_symbols.push(MergedSymbol::External(name.clone()));
                }
                _ => {
                    merged_symbols.push(MergedSymbol::Other);
                }
            }
        }
    }

    // Move relocation entries FragmentRelocationSection
    let mut fragment_relocation_sectionss: Vec<Vec<FragmentRelocationSection>> =
        vec![vec![]; modules.len()];

    for (((module, section_name_map), fragment_sections), fragment_relocation_sections) in modules
        .into_iter()
        .zip(section_name_maps.iter())
        .zip(fragment_sectionss.iter())
        .zip(fragment_relocation_sectionss.iter_mut())
    {
        for relocation_section in module.relocation_sections {
            let target_section_name = section_name_map[relocation_section.target_section_index];
            if target_section_name == SectionName::Other {
                return Err(LinkerError::Message(format!(
                    "Relocation section \"{}\" in module \"{}\" targets an unsupported section",
                    relocation_section.target_section_index, module.name
                )));
            }

            if fragment_sections.get(&target_section_name).is_some() {
                let fragment_relocation_section = FragmentRelocationSection {
                    target_section_name,
                    relocations: relocation_section.relocations,
                };
                fragment_relocation_sections.push(fragment_relocation_section);
            } else {
                return Err(LinkerError::Message(format!(
                    "Section \"{}\" not found in module \"{}\" for relocation section",
                    target_section_name, module.name
                )));
            }
        }
    }

    // Refactor the merged sections, symbols, and relocation sections into a single MergedModule(s)
    let fragment_modules: Vec<FragmentModule> = module_names
        .into_iter()
        .zip(fragment_sectionss)
        .zip(merged_symbolss)
        .zip(fragment_relocation_sectionss)
        .map(
            |(((name, sections), symbols), relocation_sections)| FragmentModule {
                name,
                sections,
                symbols,
                relocation_sections,
            },
        )
        .collect();

    let merged_file_layout = MergedFileLayout {
        contains_read_only_data,
        contains_writable_data,
        contains_tls_data,
        program_header_count,
        merged_section_infos,
    };

    let merged_asset = MergedAsset {
        fragment_modules,
        linker_generated_symbols,
        merged_file_layout,
    };

    Ok(merged_asset)
}

#[cfg(test)]
mod tests {

    use pretty_assertions::assert_eq;
    use std::{fmt::Display, vec};

    use crate::elf::{
        merger::{SectionName, merge},
        module::{Machine, RelocatableModule},
        reader::read_relocatable_module,
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

    #[test]
    fn test_merge_single_module() {
        for arch in IMPLEMENTED_ARCHS {
            let file_binary = get_example_file_binary(SourceType::Assembly, arch, "minimal.o");
            let module = get_example_file_module("minimal.o", &file_binary);
            let modules = vec![module];

            let merged_asset_result = merge(modules, arch);
            assert!(merged_asset_result.is_ok());

            let merged_asset = merged_asset_result.unwrap();

            // Check the merged modules
            let merged_modules = &merged_asset.fragment_modules;
            assert_eq!(merged_modules.len(), 1);

            // Check the linker-generated symbols
            let linker_generated_symbols = &merged_asset.linker_generated_symbols;
            let keys = linker_generated_symbols
                .keys()
                .map(|s| s.as_str())
                .collect::<Vec<_>>();
            assert_contains_all(&keys, &["_edata", "__bss_start", "_end"]);

            // Check the merged file layout
            let merged_file_layout = &merged_asset.merged_file_layout;
            assert_eq!(merged_file_layout.contains_read_only_data, false);
            assert_eq!(merged_file_layout.contains_writable_data, false);
            assert_eq!(merged_file_layout.contains_tls_data, false);
            assert_eq!(merged_file_layout.program_header_count, 3); // PHDR, metadata, code

            assert!(
                merged_file_layout
                    .merged_section_infos
                    .contains_key(&SectionName::Text)
            );

            let file_section_text = merged_file_layout
                .merged_section_infos
                .get(&SectionName::Text)
                .unwrap();
            assert!(file_section_text.size > 0);
        }
    }

    #[test]
    fn test_merge_symbol_import_and_export() {
        for arch in IMPLEMENTED_ARCHS {
            let file_binaries = get_example_file_binaries(
                SourceType::Assembly,
                arch,
                &["symbol-import.o", "symbol-export.o"],
            );

            let file_binaries_ref: Vec<&[u8]> =
                file_binaries.iter().map(|b| b.as_slice()).collect();
            let modules = get_example_file_modules(
                &["symbol-import.o", "symbol-export.o"],
                &file_binaries_ref,
            );

            let merged_asset_result = merge(modules, arch);
            assert!(merged_asset_result.is_ok());

            let merged_asset = merged_asset_result.unwrap();

            // Check the merged modules
            let merged_modules = &merged_asset.fragment_modules;
            assert_eq!(merged_modules.len(), 2);

            // Check the linker-generated symbols
            let linker_generated_symbols = &merged_asset.linker_generated_symbols;
            let keys = linker_generated_symbols
                .keys()
                .map(|s| s.as_str())
                .collect::<Vec<_>>();
            assert_contains_all(&keys, &["_edata", "__bss_start", "_end"]);

            // Check the merged file layout
            let merged_file_layout = merged_asset.merged_file_layout;
            assert_eq!(merged_file_layout.contains_read_only_data, true);
            assert_eq!(merged_file_layout.contains_writable_data, true);
            assert_eq!(merged_file_layout.contains_tls_data, false);

            assert_eq!(merged_file_layout.program_header_count, 5); // PHDR, metadata, code, read-only data, writable data

            let file_section_text = merged_file_layout
                .merged_section_infos
                .get(&SectionName::Text)
                .unwrap();
            assert!(file_section_text.size > 0);

            let file_section_rodata = merged_file_layout
                .merged_section_infos
                .get(&SectionName::ROData)
                .unwrap();
            assert!(file_section_rodata.size > 0);

            let file_section_data = merged_file_layout
                .merged_section_infos
                .get(&SectionName::Data)
                .unwrap();
            assert!(file_section_data.size > 0);

            let file_section_bss = merged_file_layout
                .merged_section_infos
                .get(&SectionName::BSS)
                .unwrap();
            assert!(file_section_bss.size > 0);
        }
    }
}
