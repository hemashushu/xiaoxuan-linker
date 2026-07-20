// Program summary:
// - Exit with status code 24.

extern long foo;
extern long bar;
extern long a;
extern long b;
extern long x;
extern long y;
extern long dec(long n);
extern long inc(long n);

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
    a = dec(foo); // a = 11 - 1 = 10
    b = inc(bar); // b = 13 + 1 = 14
    x = a + b;    // x = 10 + 14 = 24
    y = x;        // y = 24
    exit(y);
}