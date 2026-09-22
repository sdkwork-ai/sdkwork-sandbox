/* demos 能力探针 —— C 实现。协议见 demos/SPEC.md。
 *
 * 模式（照抄 SPEC §4，跨语言必须同名）：
 *   默认              跑全部检查
 *   --echo-child      往 stdout 写 child-ok、往 stderr 写 child-err，exit 0
 *   --read-stdin      读一行 stdin，内容为 ping 时回 pong
 *   --argv-probe A B  回显 argv（argv-probe:2:A:B）
 *
 * 设计说明（两处与脚本语言不同的取舍）：
 *  1) 子进程用 `system()` + **shell 重定向**完成，而不是 fork/exec+管道。
 *     C 标准库没有进程 API；POSIX 的 fork/exec 与 Windows 的 CreateProcess
 *     是两套完全不同的写法。用「引号包住自身路径 + `> out 2> err < in`」
 *     可以在两个宿主上共用同一段代码，且照样拿到退出码和分离的 stdout/stderr。
 *  2) A15 的 map 需要自己写：C 标准库没有哈希表（这不是"环境缺失"，
 *     而是"用 C 就得自己写容器"，所以这里实现一个 20 行的开放寻址表）。
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <errno.h>
#include <sys/stat.h>

#ifdef _WIN32
#  include <winsock2.h>
#  include <ws2tcpip.h>
#  include <windows.h>
#  include <io.h>
#  include <direct.h>
#  include <fcntl.h>
#  include <process.h>
#  include <pthread.h>
typedef SOCKET sock_t;
#  define CLOSE_SOCK closesocket
#else
#  include <unistd.h>
#  include <dirent.h>
#  include <fcntl.h>
#  include <sys/wait.h>
#  include <sys/socket.h>
#  include <netinet/in.h>
#  include <arpa/inet.h>
#  include <pthread.h>
typedef int sock_t;
#  define CLOSE_SOCK close
#endif

#if defined(__has_include)
#  if __has_include(<regex.h>)
#    include <regex.h>
#    define HAVE_POSIX_REGEX 1
#  endif
#endif

#define UNICODE_S "中文-日本語-한국어-🚀"
#define UNICODE_CODEPOINTS 12
#define UNICODE_BYTES 31
#define LARGE_BYTES 262144
#define MAX_DEPTH 3

/* ------------------------------------------------------------------ 小工具 */
typedef struct {
    int kind; /* 0=PASS 1=SKIP 2=FAIL */
    char detail[256];
} Res;

typedef void (*CheckFn)(Res *r);

#define OK(r, ...)   do { (r)->kind = 0; snprintf((r)->detail, sizeof((r)->detail), __VA_ARGS__); } while (0)
#define SKIPX(r, ...) do { (r)->kind = 1; snprintf((r)->detail, sizeof((r)->detail), __VA_ARGS__); } while (0)
#define ERRX(r, ...) do { (r)->kind = 2; snprintf((r)->detail, sizeof((r)->detail), __VA_ARGS__); } while (0)

static const char *g_self = "probe";
static int g_depth = 0;

static void trim_inplace(char *s)
{
    size_t n;
    while (*s == ' ' || *s == '\t' || *s == '\r' || *s == '\n') {
        memmove(s, s + 1, strlen(s));
    }
    n = strlen(s);
    while (n > 0 && (s[n - 1] == ' ' || s[n - 1] == '\t' || s[n - 1] == '\r' || s[n - 1] == '\n')) {
        s[--n] = '\0';
    }
}

/* 读整个文件到 buf，返回字节数（-1 = 打不开）。*/
static long slurp(const char *path, char *buf, size_t cap)
{
    FILE *f = fopen(path, "rb");
    size_t n;
    if (!f) {
        return -1;
    }
    n = fread(buf, 1, cap - 1, f);
    fclose(f);
    buf[n] = '\0';
    return (long) n;
}

/* 单调毫秒。C 标准库没有单调时钟：POSIX 有 clock_gettime，Windows 有
 * QueryPerformanceCounter / gettimeofday。两条路都只为了拿到"经过了多少毫秒"。*/
static double mono_ms(void)
{
#ifdef _WIN32
    LARGE_INTEGER freq, now;
    QueryPerformanceFrequency(&freq);
    QueryPerformanceCounter(&now);
    return (double) now.QuadPart * 1000.0 / (double) freq.QuadPart;
#else
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (double) ts.tv_sec * 1000.0 + (double) ts.tv_nsec / 1e6;
#endif
}

static void sleep_ms(int ms)
{
#ifdef _WIN32
    Sleep((DWORD) ms);
#else
    struct timespec ts;
    ts.tv_sec = ms / 1000;
    ts.tv_nsec = (long) (ms % 1000) * 1000000L;
    nanosleep(&ts, NULL);
#endif
}

static int exit_code_of(int raw)
{
#ifdef _WIN32
    return raw;
#else
    if (WIFEXITED(raw)) {
        return WEXITSTATUS(raw);
    }
    return -1;
#endif
}

/* 启动本程序自身；in/out/err 为 NULL 表示不重定向。返回子进程退出码。 */
static int run_self(const char *args, const char *in_file, const char *out_file, const char *err_file)
{
    char inner[2048];
    char cmd[2100];
    size_t n = 0;
    int raw;

    n += (size_t) snprintf(inner + n, sizeof(inner) - n, "\"%s\" %s", g_self, args);
    if (in_file) {
        n += (size_t) snprintf(inner + n, sizeof(inner) - n, " < \"%s\"", in_file);
    }
    if (out_file) {
        n += (size_t) snprintf(inner + n, sizeof(inner) - n, " > \"%s\"", out_file);
    }
    if (err_file) {
        n += (size_t) snprintf(inner + n, sizeof(inner) - n, " 2> \"%s\"", err_file);
    }

#ifdef _WIN32
    /* cmd.exe 的怪规则：`cmd /c` 见到命令行以 `"` 开头、且只有一个完整引号对时，
     * 会**剥掉整行的首尾两个引号**，于是 `"prog" a > "o"` 被拆成
     * `prog" a > "o`（实测报"不是内部或外部命令"）。
     * 外面再套一对引号即可让它剥掉外层、留下可用的内层。 */
    snprintf(cmd, sizeof(cmd), "\"%s\"", inner);
#else
    snprintf(cmd, sizeof(cmd), "%s", inner);
#endif

    raw = system(cmd);
    return exit_code_of(raw);
}

static int utf8_count(const char *s)
{
    int n = 0;
    const unsigned char *p = (const unsigned char *) s;
    for (; *p; p++) {
        if ((*p & 0xC0) != 0x80) {
            n++;
        }
    }
    return n;
}

/* ------------------------------------------------------- A15 用的迷你哈希表 */
#define MAP_SLOTS 32
typedef struct {
    char key[32];
    int used;
    int value;
} MapEntry;

typedef struct {
    MapEntry e[MAP_SLOTS];
} MiniMap;

static void map_init(MiniMap *m)
{
    memset(m, 0, sizeof(*m));
}

static unsigned map_hash(const char *k)
{
    unsigned h = 2166136261u;
    for (; *k; k++) {
        h = (h ^ (unsigned char) *k) * 16777619u;
    }
    return h;
}

static void map_put(MiniMap *m, const char *k, int v)
{
    unsigned i = map_hash(k) % MAP_SLOTS;
    for (;;) {
        if (!m->e[i].used || strcmp(m->e[i].key, k) == 0) {
            m->e[i].used = 1;
            strncpy(m->e[i].key, k, sizeof(m->e[i].key) - 1);
            m->e[i].key[sizeof(m->e[i].key) - 1] = '\0';
            m->e[i].value = v;
            return;
        }
        i = (i + 1) % MAP_SLOTS;
    }
}

static int map_get(const MiniMap *m, const char *k, int *out)
{
    unsigned i = map_hash(k) % MAP_SLOTS;
    while (m->e[i].used) {
        if (strcmp(m->e[i].key, k) == 0) {
            *out = m->e[i].value;
            return 1;
        }
        i = (i + 1) % MAP_SLOTS;
    }
    return 0;
}

static int map_len(const MiniMap *m)
{
    int i, n = 0;
    for (i = 0; i < MAP_SLOTS; i++) {
        if (m->e[i].used) {
            n++;
        }
    }
    return n;
}

/* ------------------------------------------------------------------ Tier A */
static void a01(Res *r)
{
    char buf[256];
    int code = run_self("--argv-probe alpha beta", NULL, "c01.out", "c01.err");
    if (code != 0) {
        ERRX(r, "child exit=%d", code);
        return;
    }
    if (slurp("c01.out", buf, sizeof(buf)) < 0) {
        ERRX(r, "cannot read c01.out");
        return;
    }
    trim_inplace(buf);
    if (strcmp(buf, "argv-probe:2:alpha:beta") != 0) {
        ERRX(r, "argv 回显不符: %s", buf);
        return;
    }
    OK(r, "2 args round-tripped");
}

static void a02(Res *r)
{
    const char *v = getenv("DEMO_LANG_TAG");
    if (!v || strcmp(v, "demos-capability") != 0) {
        ERRX(r, "DEMO_LANG_TAG=%s", v ? v : "(unset)");
        return;
    }
    OK(r, "env visible");
}

static void a03(Res *r)
{
    char o[256], e[256];
    run_self("--echo-child", NULL, "c03.out", "c03.err");
    if (slurp("c03.out", o, sizeof(o)) < 0 || slurp("c03.err", e, sizeof(e)) < 0) {
        ERRX(r, "cannot read child output files");
        return;
    }
    trim_inplace(o);
    if (strcmp(o, "child-ok") != 0) {
        ERRX(r, "stdout=%s", o);
        return;
    }
    if (!strstr(e, "child-err")) {
        ERRX(r, "stderr missing child-err");
        return;
    }
    if (strstr(o, "child-err")) {
        ERRX(r, "stderr leaked into stdout");
        return;
    }
    OK(r, "stdout/stderr separated");
}

static void a04(Res *r)
{
    char buf[64];
    FILE *f = fopen("c04.in", "wb");
    if (!f) {
        ERRX(r, "cannot create c04.in");
        return;
    }
    fwrite("ping\n", 1, 5, f);
    fclose(f);

    run_self("--read-stdin", "c04.in", "c04.out", "c04.err");
    if (slurp("c04.out", buf, sizeof(buf)) < 0) {
        ERRX(r, "cannot read c04.out");
        return;
    }
    trim_inplace(buf);
    if (strcmp(buf, "pong") != 0) {
        ERRX(r, "reply=%s", buf);
        return;
    }
    OK(r, "ping->pong");
}

static void a05(Res *r)
{
    char buf[64];
    FILE *f = fopen("a05.txt", "wb");
    if (!f) {
        ERRX(r, "open for write failed");
        return;
    }
    fwrite("hello-io", 1, 8, f);
    fclose(f);
    if (slurp("a05.txt", buf, sizeof(buf)) < 0) {
        ERRX(r, "read back failed");
        return;
    }
    if (strcmp(buf, "hello-io") != 0) {
        ERRX(r, "read back %s", buf);
        return;
    }
    OK(r, "text round-trip ok");
}

static void a06(Res *r)
{
    char buf[64];
    FILE *f = fopen("a06.txt", "wb");
    if (!f) {
        ERRX(r, "open failed");
        return;
    }
    fwrite("a", 1, 1, f);
    fclose(f);
    f = fopen("a06.txt", "ab");
    if (!f) {
        ERRX(r, "append open failed");
        return;
    }
    fwrite("b", 1, 1, f);
    fclose(f);
    if (slurp("a06.txt", buf, sizeof(buf)) < 0) {
        ERRX(r, "read back failed");
        return;
    }
    if (strcmp(buf, "ab") != 0) {
        ERRX(r, "append 结果 %s", buf);
        return;
    }
    OK(r, "append ok");
}

static void a07(Res *r)
{
    unsigned char payload[256], back[256];
    size_t i;
    FILE *f;
    for (i = 0; i < 256; i++) {
        payload[i] = (unsigned char) i;
    }
    f = fopen("a07.bin", "wb");
    if (!f) {
        ERRX(r, "open failed");
        return;
    }
    fwrite(payload, 1, 256, f);
    fclose(f);
    f = fopen("a07.bin", "rb");
    if (!f) {
        ERRX(r, "reopen failed");
        return;
    }
    if (fread(back, 1, 256, f) != 256) {
        fclose(f);
        ERRX(r, "short read");
        return;
    }
    fclose(f);
    if (memcmp(payload, back, 256) != 0) {
        ERRX(r, "二进制往返不一致");
        return;
    }
    OK(r, "256 bytes incl 0x00/0xFF");
}

static void a08(Res *r)
{
    FILE *f = fopen("a08.txt", "wb");
    struct stat st;
    if (!f) {
        ERRX(r, "open failed");
        return;
    }
    fwrite("statted", 1, 7, f);
    fclose(f);
    if (stat("a08.txt", &st) != 0) {
        ERRX(r, "stat failed");
        return;
    }
    if (st.st_size != 7) {
        ERRX(r, "size=%ld", (long) st.st_size);
        return;
    }
    if (remove("a08.txt") != 0) {
        ERRX(r, "remove failed");
        return;
    }
    if (stat("a08.txt", &st) == 0) {
        ERRX(r, "still exists after remove");
        return;
    }
    OK(r, "size=7 then removed");
}

static void a09(Res *r)
{
    char path[128];
    FILE *f;
    int found = 0;
#ifdef _WIN32
    struct _finddata_t fd;
    intptr_t h;
#else
    DIR *dp;
    struct dirent *de;
#endif
    remove("a09dir/inner.txt");
#ifdef _WIN32
    /* Windows 的 remove() 删不了目录（见下方 rmdir 处的同款说明）。 */
    _rmdir("a09dir");
#else
    remove("a09dir");
#endif
#ifdef _WIN32
    if (_mkdir("a09dir") != 0) {
        ERRX(r, "mkdir failed");
        return;
    }
#else
    if (mkdir("a09dir", 0755) != 0) {
        ERRX(r, "mkdir failed");
        return;
    }
#endif
    snprintf(path, sizeof(path), "a09dir/inner.txt");
    f = fopen(path, "wb");
    if (!f) {
        ERRX(r, "cannot create inner file");
        return;
    }
    fwrite("x", 1, 1, f);
    fclose(f);

#ifdef _WIN32
    h = _findfirst("a09dir/*", &fd);
    if (h != -1) {
        do {
            if (strcmp(fd.name, "inner.txt") == 0) {
                found = 1;
            }
        } while (_findnext(h, &fd) == 0);
        _findclose(h);
    }
#else
    dp = opendir("a09dir");
    if (dp) {
        while ((de = readdir(dp)) != NULL) {
            if (strcmp(de->d_name, "inner.txt") == 0) {
                found = 1;
            }
        }
        closedir(dp);
    }
#endif
    if (!found) {
        ERRX(r, "listing missing inner.txt");
        return;
    }
    remove(path);
#ifdef _WIN32
    /* Windows 的 remove() 只删文件，删不了目录，必须用 _rmdir。 */
    if (_rmdir("a09dir") != 0) {
#else
    if (remove("a09dir") != 0) {
#endif
        ERRX(r, "rmdir failed");
        return;
    }
    OK(r, "mkdir/list/rmdir ok");
}

static void a10(Res *r)
{
    char path[128], buf[64];
    FILE *f = NULL;
    int i;
    for (i = 0; i < 32; i++) {
        snprintf(path, sizeof(path), "demos-%ld-%d-%d.tmp", (long) time(NULL), (int) getpid(), i);
        f = fopen(path, "wbx"); /* C11 的 'x'：已存在则失败，保证唯一且不覆盖 */
        if (f) {
            break;
        }
    }
    if (!f) {
        ERRX(r, "cannot create unique temp file");
        return;
    }
    fwrite("temp-content", 1, 12, f);
    fclose(f);
    if (slurp(path, buf, sizeof(buf)) < 0) {
        remove(path);
        ERRX(r, "temp read failed");
        return;
    }
    remove(path);
    if (strcmp(buf, "temp-content") != 0) {
        ERRX(r, "temp read %s", buf);
        return;
    }
    OK(r, "unique temp file (fopen wbx)");
}

static void a11(Res *r)
{
    char buf[128];
    FILE *f;
    if (utf8_count(UNICODE_S) != UNICODE_CODEPOINTS) {
        ERRX(r, "codepoints=%d", utf8_count(UNICODE_S));
        return;
    }
    if ((int) strlen(UNICODE_S) != UNICODE_BYTES) {
        ERRX(r, "bytes=%d", (int) strlen(UNICODE_S));
        return;
    }
    f = fopen("a11.txt", "wb");
    if (!f) {
        ERRX(r, "open failed");
        return;
    }
    fwrite(UNICODE_S, 1, UNICODE_BYTES, f);
    fclose(f);
    if (slurp("a11.txt", buf, sizeof(buf)) < 0) {
        ERRX(r, "read back failed");
        return;
    }
    if (strcmp(buf, UNICODE_S) != 0) {
        ERRX(r, "round-trip mismatch");
        return;
    }
    OK(r, "%d codepoints / %d bytes", UNICODE_CODEPOINTS, UNICODE_BYTES);
}

static void a12(Res *r)
{
    char *payload = (char *) malloc(LARGE_BYTES);
    char *back = (char *) malloc(LARGE_BYTES);
    FILE *f;
    size_t i;
    if (!payload || !back) {
        free(payload);
        free(back);
        ERRX(r, "malloc failed");
        return;
    }
    for (i = 0; i < LARGE_BYTES; i++) {
        payload[i] = "abcdefgh"[i % 8];
    }
    f = fopen("a12.bin", "wb");
    if (!f) {
        free(payload);
        free(back);
        ERRX(r, "open failed");
        return;
    }
    fwrite(payload, 1, LARGE_BYTES, f);
    fclose(f);
    f = fopen("a12.bin", "rb");
    if (!f) {
        free(payload);
        free(back);
        ERRX(r, "reopen failed");
        return;
    }
    if (fread(back, 1, LARGE_BYTES, f) != LARGE_BYTES) {
        fclose(f);
        free(payload);
        free(back);
        ERRX(r, "short read");
        return;
    }
    fclose(f);
    if (memcmp(payload, back, LARGE_BYTES) != 0) {
        free(payload);
        free(back);
        ERRX(r, "content mismatch");
        return;
    }
    free(payload);
    free(back);
    OK(r, "%d bytes ok", LARGE_BYTES);
}

static void a13(Res *r)
{
    /* 注意用 long long：Windows 的 long 只有 32 位，装不下 epoch 毫秒。 */
    long long ms = (long long) time(NULL) * 1000LL;
    double t0, delta;
    if (ms < 1577836800000LL) {
        ERRX(r, "epoch=%lld", ms);
        return;
    }
    t0 = mono_ms();
    sleep_ms(50);
    delta = mono_ms() - t0;
    if (delta < 40.0) {
        ERRX(r, "sleep 只测到 %.1f ms", delta);
        return;
    }
    OK(r, "sleep %.0fms", delta);
}

static void a14(Res *r)
{
    int v;
    srand((unsigned) time(NULL) ^ (unsigned) getpid());
    v = rand() % 1000;
    if (v < 0 || v >= 1000) {
        ERRX(r, "out of range %d", v);
        return;
    }
    OK(r, "in [0,1000)");
}

static int cmp_int(const void *a, const void *b)
{
    int x = *(const int *) a, y = *(const int *) b;
    return (x > y) - (x < y);
}

static void a15(Res *r)
{
    int data[6];
    int want[6];
    MiniMap m;
    int v = 0, i;
    data[0] = 5; data[1] = 3; data[2] = 9; data[3] = 1; data[4] = 7; data[5] = 3;
    want[0] = 1; want[1] = 3; want[2] = 3; want[3] = 5; want[4] = 7; want[5] = 9;
    qsort(data, 6, sizeof(int), cmp_int);
    for (i = 0; i < 6; i++) {
        if (data[i] != want[i]) {
            ERRX(r, "sorted[%d]=%d", i, data[i]);
            return;
        }
    }
    map_init(&m);
    map_put(&m, "a", 1);
    map_put(&m, "b", 2);
    if (!map_get(&m, "a", &v) || v != 1) {
        ERRX(r, "map['a'] broken");
        return;
    }
    if (!map_get(&m, "b", &v) || v != 2) {
        ERRX(r, "map['b'] broken");
        return;
    }
    if (map_len(&m) != 2) {
        ERRX(r, "map len=%d", map_len(&m));
        return;
    }
    OK(r, "qsort + own hash map ok (C 标准库无 map)");
}

static void a16(Res *r)
{
    FILE *f = fopen("definitely-missing-file-xyz", "rb");
    if (f) {
        fclose(f);
        ERRX(r, "no error raised for missing file");
        return;
    }
    OK(r, "fopen returns NULL (errno=%d) caught", errno);
}

static void a17(Res *r)
{
    char o[256];
    int code = run_self("--echo-child", NULL, "c17.out", "c17.err");
    if (code != 0) {
        ERRX(r, "child exit=%d", code);
        return;
    }
    if (slurp("c17.out", o, sizeof(o)) < 0) {
        ERRX(r, "cannot read child stdout");
        return;
    }
    trim_inplace(o);
    if (strcmp(o, "child-ok") != 0) {
        ERRX(r, "child stdout=%s", o);
        return;
    }
    OK(r, "child exit=0, stdout captured");
}

typedef struct {
    int *counter;
    pthread_mutex_t *lock;
} ThreadArg;

static void *counter_worker(void *p)
{
    ThreadArg *a = (ThreadArg *) p;
    int i;
    for (i = 0; i < 1000; i++) {
        pthread_mutex_lock(a->lock);
        (*a->counter)++;
        pthread_mutex_unlock(a->lock);
    }
    return NULL;
}

static void a18(Res *r)
{
    pthread_t t1, t2;
    pthread_mutex_t lock = PTHREAD_MUTEX_INITIALIZER;
    ThreadArg a1, a2;
    int counter = 0;

    a1.counter = &counter; a1.lock = &lock;
    a2.counter = &counter; a2.lock = &lock;

    if (pthread_create(&t1, NULL, counter_worker, &a1) != 0) {
        ERRX(r, "pthread_create 1 failed");
        return;
    }
    if (pthread_create(&t2, NULL, counter_worker, &a2) != 0) {
        ERRX(r, "pthread_create 2 failed");
        return;
    }
    pthread_join(t1, NULL);
    pthread_join(t2, NULL);
    if (counter != 2000) {
        ERRX(r, "total=%d", counter);
        return;
    }
    OK(r, "2 pthreads -> 2000");
}

static void a19(Res *r)
{
    sock_t srv, cli, conn;
    struct sockaddr_in addr;
    socklen_t alen = sizeof(addr);
    char buf[64];
    int n;

#ifdef _WIN32
    {
        WSADATA wsa;
        if (WSAStartup(MAKEWORD(2, 2), &wsa) != 0) {
            ERRX(r, "WSAStartup failed");
            return;
        }
    }
#endif

    srv = socket(AF_INET, SOCK_STREAM, 0);
    if (srv == (sock_t) -1) {
        ERRX(r, "socket() failed");
        return;
    }
    memset(&addr, 0, sizeof(addr));
    addr.sin_family = AF_INET;
    addr.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
    addr.sin_port = 0; /* 让内核挑端口 */
    if (bind(srv, (struct sockaddr *) &addr, sizeof(addr)) != 0) {
        CLOSE_SOCK(srv);
        ERRX(r, "bind failed");
        return;
    }
    if (getsockname(srv, (struct sockaddr *) &addr, &alen) != 0) {
        CLOSE_SOCK(srv);
        ERRX(r, "getsockname failed");
        return;
    }
    if (listen(srv, 4) != 0) {
        CLOSE_SOCK(srv);
        ERRX(r, "listen failed");
        return;
    }

    /* listen 之后 connect 会在内核完成三次握手（backlog），不必先 accept。 */
    cli = socket(AF_INET, SOCK_STREAM, 0);
    if (cli == (sock_t) -1) {
        CLOSE_SOCK(srv);
        ERRX(r, "client socket failed");
        return;
    }
    if (connect(cli, (struct sockaddr *) &addr, sizeof(addr)) != 0) {
        CLOSE_SOCK(cli);
        CLOSE_SOCK(srv);
        ERRX(r, "connect failed");
        return;
    }
    if (send(cli, "tcp-ping", 8, 0) != 8) {
        CLOSE_SOCK(cli);
        CLOSE_SOCK(srv);
        ERRX(r, "send failed");
        return;
    }

    conn = accept(srv, NULL, NULL);
    if (conn == (sock_t) -1) {
        CLOSE_SOCK(cli);
        CLOSE_SOCK(srv);
        ERRX(r, "accept failed");
        return;
    }
    n = recv(conn, buf, sizeof(buf) - 1, 0);
    buf[n > 0 ? n : 0] = '\0';
    if (n != 8 || strcmp(buf, "tcp-ping") != 0) {
        CLOSE_SOCK(conn);
        CLOSE_SOCK(cli);
        CLOSE_SOCK(srv);
        ERRX(r, "server recv=%s", buf);
        return;
    }
    if (send(conn, "tcp-pong", 8, 0) != 8) {
        CLOSE_SOCK(conn);
        CLOSE_SOCK(cli);
        CLOSE_SOCK(srv);
        ERRX(r, "server send failed");
        return;
    }
    n = recv(cli, buf, sizeof(buf) - 1, 0);
    buf[n > 0 ? n : 0] = '\0';
    if (n != 8 || strcmp(buf, "tcp-pong") != 0) {
        CLOSE_SOCK(conn);
        CLOSE_SOCK(cli);
        CLOSE_SOCK(srv);
        ERRX(r, "client recv=%s", buf);
        return;
    }
    CLOSE_SOCK(conn);
    CLOSE_SOCK(cli);
    CLOSE_SOCK(srv);
    OK(r, "loopback send/recv ok");
}

/* ------------------------------------------------------------------ Tier B */
static void b01(Res *r)
{
    SKIPX(r, "C 标准库无 SHA-256（需 OpenSSL/libcrypto 等外部库）");
}

static void b02(Res *r)
{
    SKIPX(r, "C 标准库无 Base64（需 OpenSSL 或自写编码表）");
}

static void b03(Res *r)
{
    SKIPX(r, "C 标准库无 JSON（需 cJSON/jansson 等外部库）");
}

static void b04(Res *r)
{
#ifdef HAVE_POSIX_REGEX
    {
        regex_t re;
        if (regcomp(&re, "^[a-z]+-[0-9]{3}$", REG_EXTENDED) != 0) {
            ERRX(r, "regcomp failed");
            return;
        }
        if (regexec(&re, "abc-123", 0, NULL, 0) != 0) {
            regfree(&re);
            ERRX(r, "positive match failed");
            return;
        }
        if (regexec(&re, "ABC-123", 0, NULL, 0) == 0) {
            regfree(&re);
            ERRX(r, "negative match unexpected");
            return;
        }
        regfree(&re);
    }
    OK(r, "POSIX regex match+reject ok");
#else
    SKIPX(r, "本机无 <regex.h>（POSIX regex 非 ISO C，MinGW 不提供）");
#endif
}

static void b05(Res *r)
{
    SKIPX(r, "C 标准库无 gzip/deflate（需 zlib）");
}

/* -------------------------------------------------------------------- 主流程 */
static const char *IDS[24] = {
    "A01_argv", "A02_env", "A03_streams", "A04_stdin", "A05_file_rw", "A06_file_append",
    "A07_file_binary", "A08_file_stat", "A09_dir_ops", "A10_temp_file", "A11_unicode",
    "A12_large_io", "A13_time", "A14_random", "A15_container", "A16_error",
    "A17_subprocess", "A18_concurrency", "A19_tcp_loopback",
    "B01_sha256", "B02_base64", "B03_json", "B04_regex", "B05_gzip"
};

static CheckFn CHECKS[24] = {
    a01, a02, a03, a04, a05, a06, a07, a08, a09, a10, a11, a12, a13, a14, a15, a16, a17, a18, a19,
    b01, b02, b03, b04, b05
};

int main(int argc, char **argv)
{
    int pass = 0, fail = 0, skip = 0, i;
    const char *env_depth;
    const char *self;

#ifdef _WIN32
    /* 关掉 CRT 的文本模式：否则重定向到文件时 `\n` 会被翻译成 `\r\n`，
     * A03 的逐字符比对就会失败（这也是两个宿主读数不一致的经典来源）。 */
    _setmode(_fileno(stdout), _O_BINARY);
    _setmode(_fileno(stderr), _O_BINARY);
    _setmode(_fileno(stdin), _O_BINARY);
#endif

    self = getenv("DEMO_SELF");
    g_self = (self && *self) ? self : argv[0];

    env_depth = getenv("DEMO_DEPTH");
    g_depth = env_depth ? atoi(env_depth) : 0;
    if (g_depth > MAX_DEPTH) {
        fprintf(stderr, "FATAL: probe recursion depth %d exceeded\n", g_depth);
        return 3;
    }

    /* ⚠️ 模式分派必须先于任何检查执行：探针会自我调用，一旦派错就会指数级派生进程。 */
    if (argc >= 2) {
        if (strcmp(argv[1], "--echo-child") == 0) {
            printf("child-ok\n");
            fprintf(stderr, "child-err\n");
            return 0;
        }
        if (strcmp(argv[1], "--read-stdin") == 0) {
            char line[64];
            if (fgets(line, sizeof(line), stdin)) {
                trim_inplace(line);
                if (strcmp(line, "ping") == 0) {
                    printf("pong\n");
                } else {
                    printf("unexpected:%s\n", line);
                }
            } else {
                printf("unexpected:EOF\n");
            }
            return 0;
        }
        if (strcmp(argv[1], "--argv-probe") == 0) {
            printf("argv-probe:%d:", argc - 2);
            for (i = 2; i < argc; i++) {
                printf("%s%s", argv[i], (i + 1 < argc) ? ":" : "");
            }
            printf("\n");
            return 0;
        }
    }

    for (i = 0; i < 24; i++) {
        Res res;
        res.kind = 2;
        res.detail[0] = '\0';
        CHECKS[i](&res);
        if (res.kind == 0) {
            pass++;
            printf("CAP %s PASS %s\n", IDS[i], res.detail);
        } else if (res.kind == 1) {
            skip++;
            printf("CAP %s SKIP %s\n", IDS[i], res.detail);
        } else {
            fail++;
            printf("CAP %s FAIL %s\n", IDS[i], res.detail);
        }
        fflush(stdout);
    }

    printf("SUMMARY c %d %d %d\n", pass, fail, skip);
    fflush(stdout);
    return fail > 0 ? 1 : 0;
}
