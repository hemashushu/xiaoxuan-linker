// Copyright (c) 2026 Hemashushu <hippospark@gmail.com>, All rights reserved.
//
// This Source Code Form is subject to the terms of
// the Mozilla Public License version 2.0 and additional exceptions.
// For more details, see the LICENSE, LICENSE.additional, and CONTRIBUTING files.

use std::fmt::Display;

#[derive(Debug)]
pub enum LinkerError {
    Message(String),
}

impl LinkerError {
    pub fn new(msg: &str) -> Self {
        LinkerError::Message(msg.to_string())
    }
}

impl Display for LinkerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LinkerError::Message(msg) => write!(f, "LinkerError: {}", msg),
        }
    }
}

impl std::error::Error for LinkerError {}
