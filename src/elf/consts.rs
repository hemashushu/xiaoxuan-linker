// Copyright (c) 2026 Hemashushu <hippospark@gmail.com>, All rights reserved.
//
// This Source Code Form is subject to the terms of
// the Mozilla Public License version 2.0 and additional exceptions.
// For more details, see the LICENSE, LICENSE.additional, and CONTRIBUTING files.

// ELF64 header size is fixed at 64 bytes
pub const ELF_HEADER_SIZE: usize = 64;

// ELF64 program header entry size is fixed at 56 bytes
pub const PROGRAM_HEADER_ENTRY_SIZE: usize = 56;

// typical base address for x86_64 executables (ET_EXEC),
// by a contrast, PIE/DSO (ET_DYN) usually has a base address of 0.
pub const LOAD_ADDR_BASE: usize = 0x400000;

// Some load segments must be page-aligned in memory:
// - code segment (includes .text and .init/.finit)
// - read-only data segment (includes .rodata)
// - writable data segment (includes .tdata, .tbss, .data and .bss)
pub const PAGE_SIZE: usize = 0x1000;

pub const PHDR_SEGMENT_ALIGN: usize = 0x8;
pub const TLS_SEGMENT_ALIGN: usize = 0x8;

// code sections are usually 16-byte aligned, it is also used for
// merging .text sections from different modules
pub const TEXT_ALIGN: usize = 16;

// .rodata, .data and .bss sections are 8-byte aligned. This is used for
// merging data sections from different modules
pub const DATA_ALIGN: usize = 8;

// The symbol table section is 8-byte aligned
pub const SYMTAB_ALIGN: usize = 8;
