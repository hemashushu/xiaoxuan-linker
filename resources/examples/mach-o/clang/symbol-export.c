/**
 * Copyright (c) 2026 Hemashushu <hippospark@gmail.com>, All rights reserved.
 *
 * This Source Code Form is subject to the terms of
 * the Mozilla Public License version 2.0 and additional exceptions.
 * For more details, see the LICENSE, LICENSE.additional, and CONTRIBUTING files.
 */

// Ported from resources/examples/elf/gcc/symbol-export.c (Linux) to macOS.
// No source changes were needed; only the build flags/output format differ
// (see build.sh).

const long const0 = 11;
const long const1 = 13;

long init0 = 17;
long init1 = 19;

long uninit0;
long uninit1;

long dec(long n)
{
    return n - 1;
}

long inc(long n)
{
    return n + 1;
}
