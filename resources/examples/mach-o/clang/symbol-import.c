// Ported from resources/examples/elf/gcc/symbol-import.c (Linux) to macOS.
//
// Program summary:
// - Exit with status code 24.

extern long const0;
extern long const1;
extern long init0;
extern long init1;
extern long uninit0;
extern long uninit1;
extern long dec(long n);
extern long inc(long n);

int main(void)
{
    init0 = dec(const0);     // init0 = 11 - 1 = 10
    init1 = inc(const1);     // init1 = 13 + 1 = 14
    uninit0 = init0 + init1; // uninit0 = 10 + 14 = 24
    uninit1 = uninit0;       // uninit1 = 24
    return (int)uninit1;
}
