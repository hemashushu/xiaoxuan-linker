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

int public_var = 17;
int public_fn()
{
    return 19;
}

extern int exported_var;
extern int exported_fn();

int main()
{
    int pri0 = private_var;
    int pri1 = private_fn();
    int pub0 = public_var;
    int pub1 = public_fn();
    int exp0 = exported_var;
    int exp1 = exported_fn();

    // Sum of all variables and function return values
    // 11 + 13 + 17 + 19 + 23 + 29 = 112 in decimal
    int sum = pri0 + pri1 + pub0 + pub1 + exp0 + exp1;
    return sum;
}