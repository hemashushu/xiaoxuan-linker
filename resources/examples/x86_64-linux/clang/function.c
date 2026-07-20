// Program summary:
// - Prints "Hello, world!\n" to stdout.
// - Exit with status code 0.

#define SYS_write 1
#define SYS_exit 60

const char hello[] = "Hello";
const long hello_len = sizeof(hello) - 1; // String literals include a null terminator
const char world[] = ", world!\n";
const long world_len = sizeof(world) - 1;

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

static inline long __syscall3(
    long nr,
    long arg1,
    long arg2,
    long arg3)
{
    long ret;
    __asm__ volatile(
        "syscall"
        : "=a"(ret)
        : "a"(nr),
          "D"(arg1),
          "S"(arg2),
          "d"(arg3)
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

static inline long write(
    int fd,
    const void *buffer,
    long size)
{
    return __syscall3(
        SYS_write,
        (long)fd,
        (long)buffer,
        size);
}

void print_hello()
{
    (void)write(1, hello, hello_len);
}

void print_world()
{
    (void)write(1, world, world_len);
}

[[noreturn]]
void _start(void)
{
    print_hello();
    print_world();
    exit(0);
}