// Ported from resources/examples/elf/gcc/override-strong.c (Linux) to macOS.
//
// Program summary:
// - Exit with status code 53.

extern int foo(void);
extern int bar(void);

int bar(void)
{
    return 42;
}

int main(void)
{
    int i = foo();
    int j = bar();
    int k = i + j;
    return k;
}
