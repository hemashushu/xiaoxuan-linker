// Copyright (c) 2026 Hemashushu <hippospark@gmail.com>, All rights reserved.
//
// This Source Code Form is subject to the terms of
// the Mozilla Public License version 2.0 and additional exceptions.
// For more details, see the LICENSE, LICENSE.additional, and CONTRIBUTING files.


const LOAD_ADDR: u64 = 0x400000; // typical base address for x86_64 executables
const PAGE_SIZE: u64 = 0x1000; // segments must be page-aligned in memory
const DATA_ALIGN: u64 = 8; // .rodata, .data and .bss sections are 8-byte aligned

