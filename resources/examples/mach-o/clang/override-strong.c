// Ported from resources/examples/elf/gcc/override-strong.c (Linux) to macOS.
//
// Program summary:
// - Exit with status code 53.

extern int weak_keep(void);
extern int weak_override(void);

int weak_override(void)
{
    return 42;
}

int main(void)
{
    int i = weak_keep();
    int j = weak_override();
    int k = i + j;
    return k;
}
