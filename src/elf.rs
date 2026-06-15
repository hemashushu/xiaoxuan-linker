// Copyright (c) 2026 Hemashushu <hippospark@gmail.com>, All rights reserved.
//
// This Source Code Form is subject to the terms of
// the Mozilla Public License version 2.0 and additional exceptions.
// For more details, see the LICENSE, LICENSE.additional, and CONTRIBUTING files.

mod consts;
mod writer;
mod linker;
mod module;
mod reader;

pub use writer::write_executable;
pub use linker::link;
pub use reader::read_relocatable;
