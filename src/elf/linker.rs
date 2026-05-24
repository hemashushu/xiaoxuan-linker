// Copyright (c) 2026 Hemashushu <hippospark@gmail.com>, All rights reserved.
//
// This Source Code Form is subject to the terms of
// the Mozilla Public License version 2.0 and additional exceptions.
// For more details, see the LICENSE, LICENSE.additional, and CONTRIBUTING files.

use crate::{elf::module::Module, error::LinkerError};

// typical base address for x86_64 executables
const LOAD_ADDR: u64 = 0x400000;

// segments must be page-aligned in memory
const PAGE_SIZE: u64 = 0x1000;

// .text section is 16-byte aligned, this is
// used for merging .text sections from different modules
const CODE_ALIGN: u64 = 16;

// .rodata, .tdata, .tbss, .data and .bss sections are 8-byte aligned,
// this is used for merging data sections from different modules
const DATA_ALIGN: u64 = 8;

pub fn link(modules: &mut [Module]) -> Result<(), LinkerError> {
    // let mut merged_code = Vec::new();
    // let mut merged_rodata = Vec::new();
    // let mut merged_tdata = Vec::new();
    // let mut merged_tbss = Vec::new();
    // let mut merged_data = Vec::new();
    // let mut merged_bss_size = 0;

    todo!()
}
