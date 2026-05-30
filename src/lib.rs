// Copyright (c) 2026 Hemashushu <hippospark@gmail.com>, All rights reserved.
//
// This Source Code Form is subject to the terms of
// the Mozilla Public License version 2.0 and additional exceptions.
// For more details, see the LICENSE, LICENSE.additional, and CONTRIBUTING files.

mod elf;

pub mod error;

pub use elf::executable_writer::write_executable;
pub use elf::linker::link;
pub use elf::relocatable_reader::read_relocatable;
