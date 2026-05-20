// Copyright (c) 2026 Hemashushu <hippospark@gmail.com>, All rights reserved.
//
// This Source Code Form is subject to the terms of
// the Mozilla Public License version 2.0 and additional exceptions.
// For more details, see the LICENSE, LICENSE.additional, and CONTRIBUTING files.

use std::fs;

use object::{Object, ObjectSection, ObjectSegment, ObjectSymbol};

fn get_file_binary(file_name: &str) -> Vec<u8> {
    let file_path = std::env::current_dir()
        .unwrap()
        .join("resources/examples/x86_64-linux")
        .join(file_name);

    let binary_vec = fs::read(file_path).unwrap();
    binary_vec
}

#[test]
fn test_read_file_header() {
    let binary_vec = get_file_binary("single.o");
    let binary = binary_vec.as_slice();

    let file = object::File::parse(binary).unwrap();

    /* Print file header, only architecture and kind are necessary for a linker */

    // ET_*, expect `ET_REL`
    println!("Kind: {:?}", file.kind());

    // EM_*, expect `EM_X86_64`, it determines the relocation types (e.g. R_X86_64_PC32, R_X86_64_PLT32)
    println!("Architecture: {:?}", file.architecture());
}

#[test]
fn test_read_sections() {
    let binary_vec = get_file_binary("single.o");
    let binary = binary_vec.as_slice();

    let file = object::File::parse(binary).unwrap();

    println!("Section Headers:");

    for section in file.sections() {
        println!(
            "index: {}, name: {}, kind: {:?}, virtual address: 0x{:x}, offset (in file): 0x{:x}, size: 0x{:x}, flags: {:?}, alignment: {}",
            section.index().0,
            section.name().unwrap(),
            section.kind(),
            section.address(),
            section.file_range().unwrap().0,
            section.file_range().unwrap().1,
            section.flags(),
            section.align()
        );
    }
}

#[test]
fn test_read_symbols() {
    let binary_vec = get_file_binary("single.o");
    let binary = binary_vec.as_slice();

    let file = object::File::parse(binary).unwrap();

    println!("Symbols:");

    for symbol in file.symbols() {
        println!(
            "index: {}, name: {}, value (offset in section): 0x{:x}, size: 0x{:x}, section index: {:?}, kind: {:?}, flags: {:?}",
            symbol.index().0,
            symbol.name().unwrap(),
            symbol.address(),
            symbol.size(),
            symbol.section_index(),
            symbol.kind(),  // File/Section ... comes from st_info low 4 bits.
            symbol.flags()  // Elf{st_info, st_other}, st_info = bind (scope) + type
        );
    }
}

#[test]
fn test_read_relocations() {
    let binary_vec = get_file_binary("single.o");
    let binary = binary_vec.as_slice();

    let file = object::File::parse(binary).unwrap();

    println!("Relocations:");

    for section in file.sections() {
        println!(
            "Section index: {}, name: {}",
            section.index().0,
            section.name().unwrap()
        );
        for (offset, relocation) in section.relocations() {
            println!(
                "Placeholder offset: 0x{:x}, kind: {:?}, size: {}, symbol index: {:?}, addend: {}",
                offset,
                relocation.kind(), // Absolute/Relative/PltRelative, etc. comes from r_info low 32 bits (R_X86_64_PC32, R_X86_64_PLT32).
                relocation.size(), // 32/64 bits, comes from r_info low 32 bits (R_X86_64_PC32, R_X86_64_PLT32).
                relocation.target(), // The symbol index (if any)
                relocation.addend()
            );
        }
    }
}

#[test]
fn test_read_program_headers() {
    let binary_vec = get_file_binary("single.elf");
    let binary = binary_vec.as_slice();

    let file = object::File::parse(binary).unwrap();

    println!("Program Headers:");

    for segment in file.segments() {
        println!(
            "offset (in file): 0x{:x}, virtual address: 0x{:x}, size in file: 0x{:x}, size in memory: 0x{:x}, flags: {:?}, alignment: {}",
            segment.file_range().0,
            segment.address(),
            segment.file_range().1,
            segment.size(),
            segment.flags(), // PF_X, PF_W, PF_R, etc. it is equal to `segment.permissions()`
            segment.align()
        );
    }
}
