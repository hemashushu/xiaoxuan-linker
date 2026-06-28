// Copyright (c) 2026 Hemashushu <hippospark@gmail.com>, All rights reserved.
//
// This Source Code Form is subject to the terms of
// the Mozilla Public License version 2.0 and additional exceptions.
// For more details, see the LICENSE, LICENSE.additional, and CONTRIBUTING files.

mod linker;
mod relocatable;
mod writer;

pub mod module;
pub mod reader;

pub use linker::link;
pub use relocatable::read_relocatable;
pub use writer::write_executable;
