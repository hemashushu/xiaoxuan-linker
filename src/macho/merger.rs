// Copyright (c) 2026 Hemashushu <hippospark@gmail.com>, All rights reserved.
//
// This Source Code Form is subject to the terms of
// the Mozilla Public License version 2.0 and additional exceptions.
// For more details, see the LICENSE, LICENSE.additional, and CONTRIBUTING files.

use std::{collections::HashMap, fmt::Display};

use crate::{
    error::LinkerError,
    macho::module::{
        CpuType, RelocatableModule, Relocation, RelocationType, SECTION_NAME_BSS,
        SECTION_NAME_COMMON, SECTION_NAME_CONST, SECTION_NAME_DATA, SECTION_NAME_TEXT,
        SEGMENT_NAME_DATA, SEGMENT_NAME_DATA_CONST, SEGMENT_NAME_TEXT, Symbol, SymbolBind,
        SymbolType, get_load_address_base, get_segment_align_page_size,
    },
};

#[derive(Debug, PartialEq)]
pub struct FragmentModule<'a> {
    pub name: String,
    pub sections: HashMap<SectionName, FragmentSection<'a>>,
    pub symbols: Vec<MergedSymbol>,
    pub relocation_sections: Vec<FragmentRelocationSection>,
    pub got_offsets: HashMap<usize, u64>,
    pub section_ordinals: HashMap<usize, SectionName>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SectionName {
    Text,
    Const,
    DataConst,
    Data,
    BSS,
    Other,
}

impl SectionName {
    pub fn from_macho_names(segment: &str, section: &str) -> Self {
        match (segment, section) {
            (SEGMENT_NAME_TEXT, SECTION_NAME_TEXT) => SectionName::Text,
            (SEGMENT_NAME_TEXT, SECTION_NAME_CONST) | (SEGMENT_NAME_TEXT, "__cstring") => {
                SectionName::Const
            }
            (SEGMENT_NAME_TEXT, _) => SectionName::Const,
            (SEGMENT_NAME_DATA_CONST, _) => SectionName::DataConst,
            (SEGMENT_NAME_DATA, SECTION_NAME_DATA) => SectionName::Data,
            (SEGMENT_NAME_DATA, SECTION_NAME_BSS) | (SEGMENT_NAME_DATA, SECTION_NAME_COMMON) => {
                SectionName::BSS
            }
            (SEGMENT_NAME_DATA, _) => SectionName::Data,
            _ => SectionName::Other,
        }
    }
}

impl Display for SectionName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SectionName::Text => write!(f, "__TEXT,__text"),
            SectionName::Const => write!(f, "__TEXT,__const"),
            SectionName::DataConst => write!(f, "__DATA_CONST,__const"),
            SectionName::Data => write!(f, "__DATA,__data"),
            SectionName::BSS => write!(f, "__DATA,__bss"),
            SectionName::Other => write!(f, "other"),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct FragmentSection<'a> {
    pub size: u64,
    pub binary: FragmentSectionBinary<'a>,
    pub offset_in_merged_section: u64,
    pub offset_in_merged_file: u64,
    pub virtual_address: u64,
}

impl<'a> FragmentSection<'a> {
    pub fn new(
        size: u64,
        binary: &'a [u8],
        offset_in_merged_section: u64,
        offset_in_merged_file: u64,
        virtual_address: u64,
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
        size: u64,
        offset_in_merged_section: u64,
        offset_in_merged_file: u64,
        virtual_address: u64,
    ) -> Self {
        FragmentSection {
            size,
            binary: FragmentSectionBinary::None,
            offset_in_merged_section,
            offset_in_merged_file,
            virtual_address,
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum FragmentSectionBinary<'a> {
    Owned(Vec<u8>),
    Referenced(&'a [u8]),
    None,
}

#[derive(Debug, PartialEq, Clone)]
pub enum MergedSymbol {
    Defined {
        name: String,
        bind: SymbolBind,
        symbol_type: SymbolType,
        section_name: SectionName,
        virtual_address: u64,
        value: u64,
    },
    Absolute {
        name: String,
        bind: SymbolBind,
        value: u64,
    },
    External(String),
    Other,
}

#[derive(Debug, PartialEq)]
pub struct FragmentRelocationSection {
    pub name: String,
    pub target_section_name: SectionName,
    pub relocations: Vec<Relocation>,
}

fn align_up(val: u64, align: u64) -> u64 {
    if align == 0 {
        return val;
    }
    (val + align - 1) & !(align - 1)
}

#[derive(Debug, PartialEq, Clone)]
pub struct GlobalSymbolMapEntry {
    pub value: GlobalSymbolValue,
    pub is_weak: bool,
}

impl GlobalSymbolMapEntry {
    pub fn new(value: GlobalSymbolValue, is_weak: bool) -> Self {
        GlobalSymbolMapEntry { value, is_weak }
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum GlobalSymbolValue {
    Defined {
        section_name: SectionName,
        virtual_address: u64,
    },
    Absolute(u64),
}

impl GlobalSymbolValue {
    pub fn from_defined(section_name: SectionName, virtual_address: u64) -> Self {
        GlobalSymbolValue::Defined {
            section_name,
            virtual_address,
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct MergedSectionInfo {
    pub offset_in_merged_file: u64,
    pub virtual_address: u64,
    pub size: u64,
}

impl MergedSectionInfo {
    pub fn new(offset_in_merged_file: u64, virtual_address: u64, size: u64) -> Self {
        MergedSectionInfo {
            offset_in_merged_file,
            virtual_address,
            size,
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct MergedFileLayout {
    pub header_and_cmds_size: u64,
    pub text_segment_vmaddr: u64,
    pub text_segment_vmsize: u64,
    pub text_segment_filesize: u64,
    pub data_segment_vmaddr: u64,
    pub data_segment_vmsize: u64,
    pub data_segment_fileoff: u64,
    pub data_segment_filesize: u64,
    pub linkedit_segment_vmaddr: u64,
    pub linkedit_segment_vmsize: u64,
    pub linkedit_segment_fileoff: u64,
    pub linkedit_segment_filesize: u64,
    pub contains_read_only_data: bool,
    pub contains_writable_data: bool,
    pub dylib_symbol_names: Vec<String>,
    pub merged_section_infos: HashMap<SectionName, MergedSectionInfo>,
}

impl MergedFileLayout {
    pub fn contains_non_empty_section(&self, section_name: SectionName) -> bool {
        if let Some(info) = self.merged_section_infos.get(&section_name) {
            info.size > 0
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

/// TODO::
/// 1. Add crt1.o? which is located in SDK path, e.g.,
///    "/Applications/Xcode.app/Contents/Developer/Platforms/MacOSX.platform/Developer/SDKs/MacOSX26.1.sdk"
///    Obtain the path to crt1.o with command: `xcrun --sdk macosx --show-sdk-path` and append `/usr/lib/crt1.o` to the path.
pub fn merge<'a>(
    modules: Vec<RelocatableModule<'a>>,
    cpu_type: CpuType,
) -> Result<MergedAsset<'a>, LinkerError> {
    let load_addr_base = get_load_address_base(cpu_type);
    let page_size = get_segment_align_page_size(cpu_type);

    // Collect external symbols that are not defined in any module
    let mut defined_symbol_names = std::collections::HashSet::new();
    for module in &modules {
        for sym in &module.symbols {
            if let Symbol::Defined { name, .. } | Symbol::Absolute { name, .. } = sym {
                defined_symbol_names.insert(name.as_str());
            }
        }
    }

    let mut dylib_symbol_names = Vec::new();
    for module in &modules {
        for sym in &module.symbols {
            if let Symbol::External(name) = sym {
                if !defined_symbol_names.contains(name.as_str())
                    && !dylib_symbol_names.contains(name)
                {
                    dylib_symbol_names.push(name.clone());
                }
            }
        }
    }

    // Calculate section sizes across all modules
    let mut text_size = 0u64;
    let mut const_size = 0u64;
    let mut data_const_size = 0u64;
    let mut data_size = 0u64;
    let mut bss_size = 0u64;

    // Track common symbol offsets per module and symbol name
    let mut common_symbol_offsets: HashMap<(usize, String), u64> = HashMap::new();

    // Track fragment offsets per module
    type ModuleFragmentMap<'a> = HashMap<SectionName, FragmentSection<'a>>;
    let mut module_fragments: Vec<ModuleFragmentMap<'a>> =
        (0..modules.len()).map(|_| HashMap::new()).collect();

    for (mod_idx, module) in modules.iter().enumerate() {
        for sect in &module.sections {
            let section_name =
                SectionName::from_macho_names(&sect.segment_name, &sect.section_name);
            let align = sect.align.max(1);

            match section_name {
                SectionName::Text => {
                    text_size = align_up(text_size, align);
                    let offset_in_section = text_size;
                    text_size += sect.size;
                    module_fragments[mod_idx].insert(
                        SectionName::Text,
                        FragmentSection::new(sect.size, sect.binary, offset_in_section, 0, 0),
                    );
                }
                SectionName::Const => {
                    const_size = align_up(const_size, align);
                    let offset_in_section = const_size;
                    const_size += sect.size;
                    module_fragments[mod_idx].insert(
                        SectionName::Const,
                        FragmentSection::new(sect.size, sect.binary, offset_in_section, 0, 0),
                    );
                }
                SectionName::DataConst => {
                    data_const_size = align_up(data_const_size, align);
                    let offset_in_section = data_const_size;
                    data_const_size += sect.size;
                    module_fragments[mod_idx].insert(
                        SectionName::DataConst,
                        FragmentSection::new(sect.size, sect.binary, offset_in_section, 0, 0),
                    );
                }
                SectionName::Data => {
                    data_size = align_up(data_size, align);
                    let offset_in_section = data_size;
                    data_size += sect.size;
                    module_fragments[mod_idx].insert(
                        SectionName::Data,
                        FragmentSection::new(sect.size, sect.binary, offset_in_section, 0, 0),
                    );
                }
                SectionName::BSS => {
                    bss_size = align_up(bss_size, align);
                    let offset_in_section = bss_size;
                    bss_size += sect.size;
                    module_fragments[mod_idx].insert(
                        SectionName::BSS,
                        FragmentSection::new_bss(sect.size, offset_in_section, 0, 0),
                    );
                }
                SectionName::Other => {}
            }
        }

        // Allocate common/tentative symbols (section_index == 0) in BSS
        for sym in &module.symbols {
            if let Symbol::Defined {
                name,
                section_index: 0,
                ..
            } = sym
            {
                bss_size = align_up(bss_size, 8);
                common_symbol_offsets.insert((mod_idx, name.clone()), bss_size);
                bss_size += 8;
            }
        }

        // Scan for GOT relocations and allocate GOT entries in DataConst
        for reloc_sect in &module.relocation_sections {
            for reloc in &reloc_sect.relocations {
                if matches!(
                    reloc.relocation_type,
                    RelocationType::ARM64_RELOC_GOT_LOAD_PAGE21
                        | RelocationType::ARM64_RELOC_GOT_LOAD_PAGEOFF12
                ) {
                    if !common_symbol_offsets.contains_key(&(mod_idx, format!("__got_{}", reloc.symbol_index))) {
                        data_const_size = align_up(data_const_size, 8);
                        common_symbol_offsets.insert((mod_idx, format!("__got_{}", reloc.symbol_index)), data_const_size);
                        data_const_size += 8;
                    }
                }
            }
        }
    }

    // Allocate GOT slots and stubs for dylib symbols AFTER module sections have been accumulated
    let mut dylib_stub_addrs = HashMap::new();

    for sym_name in &dylib_symbol_names {
        data_const_size = align_up(data_const_size, 8);
        let got_slot_offset = data_const_size;
        data_const_size += 8;

        text_size = align_up(text_size, 4);
        let stub_offset = text_size;
        text_size += 12;

        dylib_stub_addrs.insert(sym_name.clone(), (stub_offset, got_slot_offset));
    }

    // Estimate Mach-O Header and Load Commands size
    // 32 (header) + 72 (__PAGEZERO) + 232 (__TEXT) + 152 (__DATA) + 72 (__LINKEDIT) + 24 (LC_MAIN) + 32 (LC_LOAD_DYLINKER) + 56 (LC_LOAD_DYLIB) + 24 (LC_SYMTAB) + 80 (LC_DYSYMTAB) = ~800 bytes.
    let header_and_cmds_size = 1024u64;

    // Layout __TEXT segment
    let text_segment_vmaddr = load_addr_base;
    let text_section_offset = header_and_cmds_size;
    let text_section_vmaddr = text_segment_vmaddr + text_section_offset;

    let const_section_offset = text_section_offset + text_size;
    let const_section_vmaddr = text_segment_vmaddr + const_section_offset;

    let text_segment_raw_size = const_section_offset + const_size;
    let text_segment_vmsize = align_up(text_segment_raw_size, page_size);
    let text_segment_filesize = text_segment_vmsize;

    // Layout __DATA segment (starts at next page boundary)
    let data_segment_vmaddr = text_segment_vmaddr + text_segment_vmsize;
    let data_segment_fileoff = text_segment_filesize;

    let data_const_section_offset = data_segment_fileoff;
    let data_const_section_vmaddr = data_segment_vmaddr;

    let data_section_offset = data_const_section_offset + data_const_size;
    let data_section_vmaddr = data_segment_vmaddr + (data_section_offset - data_segment_fileoff);

    let bss_section_offset = data_section_offset + data_size;
    let bss_section_vmaddr = data_segment_vmaddr + (bss_section_offset - data_segment_fileoff);

    let data_segment_raw_filesize = (data_section_offset + data_size) - data_segment_fileoff;
    let data_segment_filesize = align_up(data_segment_raw_filesize, page_size);

    let data_segment_raw_vmsize = (bss_section_offset + bss_size) - data_segment_fileoff;
    let data_segment_vmsize = align_up(data_segment_raw_vmsize, page_size);

    // Layout __LINKEDIT segment
    let linkedit_segment_vmaddr = data_segment_vmaddr + data_segment_vmsize;
    let linkedit_segment_fileoff = data_segment_fileoff + data_segment_filesize;
    let linkedit_segment_filesize = page_size;
    let linkedit_segment_vmsize = page_size;

    let mut merged_section_infos = HashMap::new();
    merged_section_infos.insert(
        SectionName::Text,
        MergedSectionInfo::new(text_section_offset, text_section_vmaddr, text_size),
    );
    merged_section_infos.insert(
        SectionName::Const,
        MergedSectionInfo::new(const_section_offset, const_section_vmaddr, const_size),
    );
    merged_section_infos.insert(
        SectionName::DataConst,
        MergedSectionInfo::new(
            data_const_section_offset,
            data_const_section_vmaddr,
            data_const_size,
        ),
    );
    merged_section_infos.insert(
        SectionName::Data,
        MergedSectionInfo::new(data_section_offset, data_section_vmaddr, data_size),
    );
    merged_section_infos.insert(
        SectionName::BSS,
        MergedSectionInfo::new(bss_section_offset, bss_section_vmaddr, bss_size),
    );

    let merged_file_layout = MergedFileLayout {
        header_and_cmds_size,
        text_segment_vmaddr,
        text_segment_vmsize,
        text_segment_filesize,
        data_segment_vmaddr,
        data_segment_vmsize,
        data_segment_fileoff,
        data_segment_filesize,
        linkedit_segment_vmaddr,
        linkedit_segment_vmsize,
        linkedit_segment_fileoff,
        linkedit_segment_filesize,
        contains_read_only_data: const_size > 0,
        contains_writable_data: (data_size + bss_size + data_const_size) > 0,
        dylib_symbol_names,
        merged_section_infos,
    };

    // Update fragment sections and symbol virtual addresses
    let mut fragment_modules = Vec::new();

    for (mod_idx, module) in modules.into_iter().enumerate() {
        let mut sections = module_fragments[mod_idx].clone();

        for (sect_name, frag) in sections.iter_mut() {
            if let Some(info) = merged_file_layout.merged_section_infos.get(sect_name) {
                frag.offset_in_merged_file = info.offset_in_merged_file + frag.offset_in_merged_section;
                frag.virtual_address = info.virtual_address + frag.offset_in_merged_section;
            }
        }

        let mut section_ordinals = HashMap::new();
        for (sect_idx, sect) in module.sections.iter().enumerate() {
            let name_enum = SectionName::from_macho_names(&sect.segment_name, &sect.section_name);
            section_ordinals.insert(sect_idx + 1, name_enum);
        }

        let symbol_count = module.symbols.len();
        let mut symbols = Vec::new();
        for sym in module.symbols {
            match sym {
                Symbol::Defined {
                    name,
                    bind,
                    symbol_type,
                    section_index,
                    value,
                } => {
                    let (sect_name, virtual_address) = if section_index > 0
                        && section_index <= module.sections.len()
                    {
                        let sect_header = &module.sections[section_index - 1];
                        let name_enum = SectionName::from_macho_names(
                            &sect_header.segment_name,
                            &sect_header.section_name,
                        );
                        let frag_addr = sections
                            .get(&name_enum)
                            .map(|f| f.virtual_address)
                            .unwrap_or(0);
                        let section_offset = value.saturating_sub(sect_header.virtual_address);
                        (name_enum, frag_addr + section_offset)
                    } else if let Some(&common_offset) =
                        common_symbol_offsets.get(&(mod_idx, name.clone()))
                    {
                        (SectionName::BSS, bss_section_vmaddr + common_offset)
                    } else {
                        (SectionName::BSS, bss_section_vmaddr)
                    };

                    symbols.push(MergedSymbol::Defined {
                        name,
                        bind,
                        symbol_type,
                        section_name: sect_name,
                        virtual_address,
                        value,
                    });
                }
                Symbol::Absolute { name, bind, value } => {
                    symbols.push(MergedSymbol::Absolute { name, bind, value });
                }
                Symbol::External(name) => {
                    symbols.push(MergedSymbol::External(name));
                }
                _ => {
                    symbols.push(MergedSymbol::Other);
                }
            }
        }

        let mut relocation_sections = Vec::new();
        for reloc_sect in module.relocation_sections {
            let target_sect = module.sections.get(reloc_sect.target_section_index - 1);
            let target_sect_name = target_sect
                .map(|s| SectionName::from_macho_names(&s.segment_name, &s.section_name))
                .unwrap_or(SectionName::Other);

            relocation_sections.push(FragmentRelocationSection {
                name: reloc_sect.name,
                target_section_name: target_sect_name,
                relocations: reloc_sect.relocations,
            });
        }

        let mut got_offsets = HashMap::new();
        for sym_idx in 0..symbol_count + 10 {
            if let Some(&got_offset) =
                common_symbol_offsets.get(&(mod_idx, format!("__got_{}", sym_idx)))
            {
                let got_vaddr = data_const_section_vmaddr + got_offset;
                got_offsets.insert(sym_idx, got_vaddr);
            }
        }

        fragment_modules.push(FragmentModule {
            name: module.name,
            sections,
            symbols,
            relocation_sections,
            got_offsets,
            section_ordinals,
        });
    }

    let mut linker_generated_symbols = HashMap::new();
    linker_generated_symbols.insert(
        "__mh_execute_header".to_string(),
        GlobalSymbolMapEntry::new(
            GlobalSymbolValue::from_defined(SectionName::Text, text_segment_vmaddr),
            false,
        ),
    );

    for (sym_name, (stub_off, got_off)) in &dylib_stub_addrs {
        let stub_vaddr = text_section_vmaddr + stub_off;
        let got_vaddr = data_const_section_vmaddr + got_off;

        let site_vaddr = stub_vaddr;
        let target_page = got_vaddr & !0xfff;
        let site_page = site_vaddr & !0xfff;
        let page_delta = (target_page as i64 - site_page as i64) >> 12;

        let immlo = ((page_delta & 3) as u32) << 29;
        let immhi = (((page_delta >> 2) & 0x7ffff) as u32) << 5;
        let insn1 = 0x90000010u32 | immlo | immhi;

        let page_off = ((got_vaddr & 0xfff) / 8) as u32;
        let insn2 = 0xf9400210u32 | ((page_off & 0xfff) << 10);
        let insn3 = 0xd61f0200u32;

        if let Some(main_mod) = fragment_modules.get_mut(0) {
            if let Some(text_sect) = main_mod.sections.get_mut(&SectionName::Text) {
                let mut bin = match &text_sect.binary {
                    FragmentSectionBinary::Referenced(d) => d.to_vec(),
                    FragmentSectionBinary::Owned(d) => d.clone(),
                    FragmentSectionBinary::None => Vec::new(),
                };
                if bin.len() < *stub_off as usize {
                    bin.resize(*stub_off as usize, 0);
                }
                bin.extend_from_slice(&insn1.to_le_bytes());
                bin.extend_from_slice(&insn2.to_le_bytes());
                bin.extend_from_slice(&insn3.to_le_bytes());
                text_sect.binary = FragmentSectionBinary::Owned(bin);
            }
        }

        linker_generated_symbols.insert(
            sym_name.clone(),
            GlobalSymbolMapEntry::new(
                GlobalSymbolValue::from_defined(SectionName::Text, stub_vaddr),
                false,
            ),
        );
    }

    Ok(MergedAsset {
        fragment_modules,
        linker_generated_symbols,
        merged_file_layout,
    })
}
