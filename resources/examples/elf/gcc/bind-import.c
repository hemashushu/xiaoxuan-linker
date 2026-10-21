/**
 * Copyright (c) 2026 Hemashushu <hippospark@gmail.com>, All rights reserved.
 *
 * This Source Code Form is subject to the terms of
 * the Mozilla Public License version 2.0 and additional exceptions.
 * For more details, see the LICENSE, LICENSE.additional, and CONTRIBUTING files.
 */

static int private_var = 11;
static int private_fn()
{
    return 13;
}

int exported_var = 17;
int public_fn()
{
    return 19;
}

extern int exported_var;
extern int exported_fn();

int main()
{
    int p0 = private_var;
    int p1 = private_fn();
    int g0 = exported_var;
    int g1 = public_fn();
    int e0 = exported_var;
    int e1 = exported_fn();

    // Sum of all variables and function return values
    // 11 + 13 + 17 + 19 + 23 + 29 = 112 in decimal
    int sum = p0 + p1 + g0 + g1 + e0 + e1;
    return sum;
}