// Program summary:
// - Exit with status code 53.

extern int foo();
extern int bar();

int bar()
{
    return 42;
}

#define SYS_exit 60

static inline long __syscall1(long nr, long arg1)
{
    long ret;
    __asm__ volatile(
        "syscall"
        : "=a"(ret)
        : "a"(nr),
          "D"(arg1)
        : "rcx", "r11", "memory");
    return ret;
}

[[noreturn]]
static inline void exit(int status)
{
    (void)__syscall1(SYS_exit, (long)status);

    for (;;)
    {
    }
}

[[noreturn]]
void _start(void)
{
    int i = foo();
    int j = bar();
    int k = i + j;
    exit(k);
}