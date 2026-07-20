// Program summary:
// - Exit with status code 24.

// .rodata
const long foo = 11;
const long bar = 13;

// .data
long a = 17;
long b = 19;

// .bss
long x;
long y;

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
    a = foo - 1; // 11 - 1 = 10
    b = bar + 1; // 13 + 1 = 14
    x = a + b;   // 10 + 14 = 24
    y = x;       // 24

    exit(y);
}