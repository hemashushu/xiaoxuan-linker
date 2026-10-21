// Ported from resources/examples/elf/gcc/data.c (Linux) to macOS.
//
// Program summary:
// - Exit with status code 24.

// __TEXT,__const
const long const0 = 11;
const long const1 = 13;

// __DATA,__data
long init0 = 17;
long init1 = 19;

// __DATA,__bss
long uninit0;
long uninit1;

int main(void)
{
    init0 = const0 - 1;      // 11 - 1 = 10
    init1 = const1 + 1;      // 13 + 1 = 14
    uninit0 = init0 + init1; // 10 + 14 = 24
    uninit1 = uninit0;       // 24

    return (int)uninit1;
}
