// demos 能力探针 —— C++ 实现。协议见 demos/SPEC.md。
//
// 模式（照抄 SPEC §4，跨语言必须同名）：
//   默认              跑全部检查
//   --echo-child      往 stdout 写 child-ok、往 stderr 写 child-err，exit 0
//   --read-stdin      读一行 stdin，内容为 ping 时回 pong
//   --argv-probe A B  回显 argv（argv-probe:2:A:B）
//
// 与 C 版的关系：子进程同样用 `system()` + shell 重定向（理由见 demos/c/probe.c），
// 但容器/字符串/线程/文件系统/正则全部走标准库（std::map、std::thread、<regex>、
// <filesystem>）—— 这正是 C++ 相对 C 的差别所在，也是本矩阵要测出来的东西。
#include <algorithm>
#include <chrono>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <map>
#include <mutex>
#include <random>
#include <regex>
#include <sstream>
#include <string>
#include <thread>
#include <vector>

#ifdef _WIN32
#  include <winsock2.h>
#  include <ws2tcpip.h>
#  include <windows.h>
#  include <io.h>
#  include <fcntl.h>
#else
#  include <unistd.h>
#  include <fcntl.h>
#  include <sys/wait.h>
#  include <sys/socket.h>
#  include <netinet/in.h>
#  include <arpa/inet.h>
#endif

namespace fs = std::filesystem;

static const char *UNICODE_S = "中文-日本語-한국어-🚀";
static const int UNICODE_CODEPOINTS = 12;
static const int UNICODE_BYTES = 31;
static const long LARGE_BYTES = 262144;
static const int MAX_DEPTH = 3;

static std::string g_self = "probe";
static int g_depth = 0;

// ------------------------------------------------------------------ 结果与工具
struct Res {
    int kind;  // 0=PASS 1=SKIP 2=FAIL
    std::string detail;
};

static void ok(Res &r, const std::string &d)
{
    r.kind = 0;
    r.detail = d;
}

static void skip(Res &r, const std::string &d)
{
    r.kind = 1;
    r.detail = d;
}

static void err(Res &r, const std::string &d)
{
    r.kind = 2;
    r.detail = d;
}

static std::string trim(const std::string &s)
{
    size_t a = s.find_first_not_of(" \t\r\n");
    if (a == std::string::npos) {
        return "";
    }
    size_t b = s.find_last_not_of(" \t\r\n");
    return s.substr(a, b - a + 1);
}

static bool slurp(const std::string &path, std::string &out)
{
    std::ifstream f(path, std::ios::binary);
    if (!f) {
        return false;
    }
    std::ostringstream ss;
    ss << f.rdbuf();
    out = ss.str();
    return true;
}

/** 启动本程序自身；空串表示不重定向。返回子进程退出码。 */
static int run_self(const std::string &args, const std::string &in_file,
                    const std::string &out_file, const std::string &err_file)
{
    std::ostringstream inner;
    inner << "\"" << g_self << "\" " << args;
    if (!in_file.empty()) {
        inner << " < \"" << in_file << "\"";
    }
    if (!out_file.empty()) {
        inner << " > \"" << out_file << "\"";
    }
    if (!err_file.empty()) {
        inner << " 2> \"" << err_file << "\"";
    }

    std::string cmd = inner.str();
#ifdef _WIN32
    // cmd.exe 会剥掉「以引号开头且只有一个引号对」的整行首尾引号，
    // 所以外面再套一对（详见 demos/c/probe.c 的注释）。
    cmd = "\"" + cmd + "\"";
#endif

    int raw = std::system(cmd.c_str());
#ifdef _WIN32
    return raw;
#else
    if (WIFEXITED(raw)) {
        return WEXITSTATUS(raw);
    }
    return -1;
#endif
}

static int utf8_count(const std::string &s)
{
    int n = 0;
    for (unsigned char c : s) {
        if ((c & 0xC0) != 0x80) {
            n++;
        }
    }
    return n;
}

static void sleep_ms(int ms)
{
    std::this_thread::sleep_for(std::chrono::milliseconds(ms));
}

// ------------------------------------------------------------------ Tier A
static void a01(Res &r)
{
    std::string out;
    int code = run_self("--argv-probe alpha beta", "", "cpp01.out", "cpp01.err");
    if (code != 0) {
        err(r, "child exit=" + std::to_string(code));
        return;
    }
    if (!slurp("cpp01.out", out)) {
        err(r, "cannot read cpp01.out");
        return;
    }
    out = trim(out);
    if (out != "argv-probe:2:alpha:beta") {
        err(r, "argv 回显不符: " + out);
        return;
    }
    ok(r, "2 args round-tripped");
}

static void a02(Res &r)
{
    const char *v = std::getenv("DEMO_LANG_TAG");
    if (!v || std::strcmp(v, "demos-capability") != 0) {
        err(r, std::string("DEMO_LANG_TAG=") + (v ? v : "(unset)"));
        return;
    }
    ok(r, "env visible");
}

static void a03(Res &r)
{
    std::string o, e;
    run_self("--echo-child", "", "cpp03.out", "cpp03.err");
    if (!slurp("cpp03.out", o) || !slurp("cpp03.err", e)) {
        err(r, "cannot read child output files");
        return;
    }
    if (trim(o) != "child-ok") {
        err(r, "stdout=" + trim(o));
        return;
    }
    if (e.find("child-err") == std::string::npos) {
        err(r, "stderr missing child-err");
        return;
    }
    if (o.find("child-err") != std::string::npos) {
        err(r, "stderr leaked into stdout");
        return;
    }
    ok(r, "stdout/stderr separated");
}

static void a04(Res &r)
{
    {
        std::ofstream f("cpp04.in", std::ios::binary);
        if (!f) {
            err(r, "cannot create cpp04.in");
            return;
        }
        f << "ping\n";
    }
    run_self("--read-stdin", "cpp04.in", "cpp04.out", "cpp04.err");
    std::string out;
    if (!slurp("cpp04.out", out)) {
        err(r, "cannot read cpp04.out");
        return;
    }
    if (trim(out) != "pong") {
        err(r, "reply=" + trim(out));
        return;
    }
    ok(r, "ping->pong");
}

static void a05(Res &r)
{
    std::string out;
    {
        std::ofstream f("a05.txt", std::ios::binary);
        f << "hello-io";
    }
    if (!slurp("a05.txt", out) || out != "hello-io") {
        err(r, "read back " + out);
        return;
    }
    ok(r, "text round-trip ok");
}

static void a06(Res &r)
{
    {
        std::ofstream f("a06.txt", std::ios::binary);
        f << "a";
    }
    {
        std::ofstream f("a06.txt", std::ios::binary | std::ios::app);
        f << "b";
    }
    std::string out;
    if (!slurp("a06.txt", out) || out != "ab") {
        err(r, "append 结果 " + out);
        return;
    }
    ok(r, "append ok");
}

static void a07(Res &r)
{
    std::string payload;
    for (int i = 0; i < 256; i++) {
        payload.push_back(static_cast<char>(i));
    }
    {
        std::ofstream f("a07.bin", std::ios::binary);
        f.write(payload.data(), static_cast<std::streamsize>(payload.size()));
    }
    std::string back;
    if (!slurp("a07.bin", back) || back != payload) {
        err(r, "二进制往返不一致 len=" + std::to_string(back.size()));
        return;
    }
    ok(r, "256 bytes incl 0x00/0xFF");
}

static void a08(Res &r)
{
    {
        std::ofstream f("a08.txt", std::ios::binary);
        f << "statted";
    }
    if (!fs::exists("a08.txt")) {
        err(r, "file not created");
        return;
    }
    auto size = fs::file_size("a08.txt");
    if (size != 7) {
        err(r, "size=" + std::to_string(size));
        return;
    }
    fs::remove("a08.txt");
    if (fs::exists("a08.txt")) {
        err(r, "remove failed");
        return;
    }
    ok(r, "size=7 then removed");
}

static void a09(Res &r)
{
    std::error_code ec;
    fs::remove_all("a09dir", ec);
    fs::create_directory("a09dir", ec);
    if (ec) {
        err(r, "mkdir failed: " + ec.message());
        return;
    }
    {
        std::ofstream f("a09dir/inner.txt", std::ios::binary);
        f << "x";
    }
    bool found = false;
    for (const auto &e : fs::directory_iterator("a09dir")) {
        if (e.path().filename() == "inner.txt") {
            found = true;
        }
    }
    if (!found) {
        err(r, "listing missing inner.txt");
        return;
    }
    fs::remove("a09dir/inner.txt", ec);
    fs::remove("a09dir", ec);
    if (fs::exists("a09dir")) {
        err(r, "rmdir failed");
        return;
    }
    ok(r, "mkdir/list/rmdir ok (std::filesystem)");
}

static void a10(Res &r)
{
    std::string path;
    bool created = false;
    for (int i = 0; i < 32; i++) {
        std::ostringstream ss;
        ss << "demos-" << std::chrono::system_clock::now().time_since_epoch().count()
           << "-" << i << ".tmp";
        path = ss.str();
        if (fs::exists(path)) {
            continue;  // 名字撞了就换一个
        }
        std::ofstream f(path, std::ios::binary);
        if (!f) {
            continue;
        }
        f << "temp-content";
        f.close();
        created = true;
        break;
    }
    if (!created) {
        err(r, "cannot create unique temp file");
        return;
    }
    std::string out;
    bool read_ok = slurp(path, out);
    fs::remove(path);
    if (!read_ok) {
        err(r, "temp read failed");
        return;
    }
    if (out != "temp-content") {
        err(r, "temp read " + out);
        return;
    }
    ok(r, "unique temp file");
}

static void a11(Res &r)
{
    std::string s(UNICODE_S);
    if (utf8_count(s) != UNICODE_CODEPOINTS) {
        err(r, "codepoints=" + std::to_string(utf8_count(s)));
        return;
    }
    if (static_cast<int>(s.size()) != UNICODE_BYTES) {
        err(r, "bytes=" + std::to_string(s.size()));
        return;
    }
    {
        std::ofstream f("a11.txt", std::ios::binary);
        f.write(s.data(), static_cast<std::streamsize>(s.size()));
    }
    std::string back;
    if (!slurp("a11.txt", back) || back != s) {
        err(r, "round-trip mismatch");
        return;
    }
    ok(r, std::to_string(UNICODE_CODEPOINTS) + " codepoints / " +
              std::to_string(UNICODE_BYTES) + " bytes");
}

static void a12(Res &r)
{
    std::string payload;
    payload.reserve(LARGE_BYTES);
    while (static_cast<long>(payload.size()) < LARGE_BYTES) {
        payload += "abcdefgh";
    }
    {
        std::ofstream f("a12.bin", std::ios::binary);
        f.write(payload.data(), static_cast<std::streamsize>(payload.size()));
    }
    std::string back;
    if (!slurp("a12.bin", back) || back != payload) {
        err(r, "len=" + std::to_string(back.size()));
        return;
    }
    ok(r, std::to_string(LARGE_BYTES) + " bytes ok");
}

static void a13(Res &r)
{
    auto ms = std::chrono::duration_cast<std::chrono::milliseconds>(
                  std::chrono::system_clock::now().time_since_epoch())
                  .count();
    if (ms < 1577836800000LL) {
        err(r, "epoch=" + std::to_string(ms));
        return;
    }
    auto t0 = std::chrono::steady_clock::now();
    sleep_ms(50);
    double delta = std::chrono::duration<double, std::milli>(
                       std::chrono::steady_clock::now() - t0)
                       .count();
    if (delta < 40.0) {
        err(r, "sleep 只测到 " + std::to_string(delta) + " ms");
        return;
    }
    ok(r, "sleep " + std::to_string(static_cast<int>(delta)) + "ms (steady_clock)");
}

static void a14(Res &r)
{
    std::mt19937 gen(static_cast<unsigned>(
        std::chrono::system_clock::now().time_since_epoch().count()));
    std::uniform_int_distribution<int> dist(0, 999);
    int v = dist(gen);
    if (v < 0 || v >= 1000) {
        err(r, "out of range " + std::to_string(v));
        return;
    }
    ok(r, "in [0,1000) (std::mt19937)");
}

static void a15(Res &r)
{
    std::vector<int> data{5, 3, 9, 1, 7, 3};
    std::sort(data.begin(), data.end());
    if (data != std::vector<int>{1, 3, 3, 5, 7, 9}) {
        err(r, "sorted mismatch");
        return;
    }
    std::map<std::string, int> m;
    m["a"] = 1;
    m["b"] = 2;
    if (m["a"] != 1 || m["b"] != 2 || m.size() != 2) {
        err(r, "map broken");
        return;
    }
    ok(r, "std::sort + std::map ok");
}

static void a16(Res &r)
{
    std::ifstream f("definitely-missing-file-xyz", std::ios::binary);
    if (f.good()) {
        err(r, "no error raised for missing file");
        return;
    }
    ok(r, "ifstream failbit set for missing file");
}

static void a17(Res &r)
{
    std::string out;
    int code = run_self("--echo-child", "", "cpp17.out", "cpp17.err");
    if (code != 0) {
        err(r, "child exit=" + std::to_string(code));
        return;
    }
    if (!slurp("cpp17.out", out) || trim(out) != "child-ok") {
        err(r, "child stdout=" + trim(out));
        return;
    }
    ok(r, "child exit=0, stdout captured");
}

static void a18(Res &r)
{
    int counter = 0;
    std::mutex lock;
    auto worker = [&counter, &lock]() {
        for (int i = 0; i < 1000; i++) {
            std::lock_guard<std::mutex> g(lock);
            counter++;
        }
    };
    std::thread t1(worker);
    std::thread t2(worker);
    t1.join();
    t2.join();
    if (counter != 2000) {
        err(r, "total=" + std::to_string(counter));
        return;
    }
    ok(r, "2 threads -> 2000");
}

static void a19(Res &r)
{
#ifdef _WIN32
    WSADATA wsa;
    if (WSAStartup(MAKEWORD(2, 2), &wsa) != 0) {
        err(r, "WSAStartup failed");
        return;
    }
    using sock_t = SOCKET;
    auto closer = [](sock_t s) { closesocket(s); };
    const sock_t INVALID = INVALID_SOCKET;
#else
    using sock_t = int;
    auto closer = [](sock_t s) { close(s); };
    const sock_t INVALID = -1;
#endif

    sock_t srv = socket(AF_INET, SOCK_STREAM, 0);
    if (srv == INVALID) {
        err(r, "socket() failed");
        return;
    }
    struct sockaddr_in addr;
    std::memset(&addr, 0, sizeof(addr));
    addr.sin_family = AF_INET;
    addr.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
    addr.sin_port = 0;
    if (bind(srv, reinterpret_cast<struct sockaddr *>(&addr), sizeof(addr)) != 0) {
        closer(srv);
        err(r, "bind failed");
        return;
    }
    int alen = sizeof(addr);
    if (getsockname(srv, reinterpret_cast<struct sockaddr *>(&addr),
#ifdef _WIN32
                    &alen
#else
                    reinterpret_cast<socklen_t *>(&alen)
#endif
                    ) != 0) {
        closer(srv);
        err(r, "getsockname failed");
        return;
    }
    if (listen(srv, 4) != 0) {
        closer(srv);
        err(r, "listen failed");
        return;
    }

    // listen 之后 connect 会在内核完成握手（backlog），不必先 accept。
    sock_t cli = socket(AF_INET, SOCK_STREAM, 0);
    if (cli == INVALID || connect(cli, reinterpret_cast<struct sockaddr *>(&addr), sizeof(addr)) != 0) {
        closer(srv);
        err(r, "connect failed");
        return;
    }
    if (send(cli, "tcp-ping", 8, 0) != 8) {
        closer(cli);
        closer(srv);
        err(r, "send failed");
        return;
    }

    sock_t conn = accept(srv, nullptr, nullptr);
    if (conn == INVALID) {
        closer(cli);
        closer(srv);
        err(r, "accept failed");
        return;
    }
    char buf[64];
    int n = recv(conn, buf, sizeof(buf) - 1, 0);
    std::string recv_s = (n > 0) ? std::string(buf, static_cast<size_t>(n)) : "";
    if (recv_s != "tcp-ping") {
        closer(conn);
        closer(cli);
        closer(srv);
        err(r, "server recv=" + recv_s);
        return;
    }
    if (send(conn, "tcp-pong", 8, 0) != 8) {
        closer(conn);
        closer(cli);
        closer(srv);
        err(r, "server send failed");
        return;
    }
    n = recv(cli, buf, sizeof(buf) - 1, 0);
    std::string reply = (n > 0) ? std::string(buf, static_cast<size_t>(n)) : "";
    closer(conn);
    closer(cli);
    closer(srv);
    if (reply != "tcp-pong") {
        err(r, "client recv=" + reply);
        return;
    }
    ok(r, "loopback send/recv ok");
}

// ------------------------------------------------------------------ Tier B
static void b01(Res &r)
{
    skip(r, "C++ 标准库无 SHA-256（需 OpenSSL/OpenSSL++ 或自写实现）");
}

static void b02(Res &r)
{
    skip(r, "C++ 标准库无 Base64（需第三方库或自写编码表）");
}

static void b03(Res &r)
{
    skip(r, "C++ 标准库无 JSON（需 nlohmann/json 等第三方库）");
}

static void b04(Res &r)
{
    try {
        std::regex re("^[a-z]+-[0-9]{3}$");
        if (!std::regex_match(std::string("abc-123"), re)) {
            err(r, "positive match failed");
            return;
        }
        if (std::regex_match(std::string("ABC-123"), re)) {
            err(r, "negative match unexpected");
            return;
        }
    } catch (const std::regex_error &e) {
        err(r, std::string("regex_error: ") + e.what());
        return;
    }
    ok(r, "std::regex match+reject ok");
}

static void b05(Res &r)
{
    skip(r, "C++ 标准库无 gzip/deflate（需 zlib）");
}

// -------------------------------------------------------------------- 主流程
int main(int argc, char **argv)
{
    using CheckFn = void (*)(Res &);
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

#ifdef _WIN32
    // 关掉 CRT 文本模式：否则重定向到文件时 `\n` 会变成 `\r\n`（同 demos/c/probe.c）。
    _setmode(_fileno(stdout), _O_BINARY);
    _setmode(_fileno(stderr), _O_BINARY);
    _setmode(_fileno(stdin), _O_BINARY);
#endif

    const char *self = std::getenv("DEMO_SELF");
    g_self = (self && *self) ? self : argv[0];

    const char *env_depth = std::getenv("DEMO_DEPTH");
    g_depth = env_depth ? std::atoi(env_depth) : 0;
    if (g_depth > MAX_DEPTH) {
        std::cerr << "FATAL: probe recursion depth " << g_depth << " exceeded\n";
        return 3;
    }

    // ⚠️ 模式分派必须先于任何检查执行：探针会自我调用，一旦派错就会指数级派生进程。
    if (argc >= 2) {
        std::string m = argv[1];
        if (m == "--echo-child") {
            std::cout << "child-ok\n";
            std::cerr << "child-err\n";
            std::cout.flush();
            std::cerr.flush();
            return 0;
        }
        if (m == "--read-stdin") {
            std::string line;
            std::getline(std::cin, line);
            line = trim(line);
            if (line == "ping") {
                std::cout << "pong\n";
            } else {
                std::cout << "unexpected:" << line << "\n";
            }
            std::cout.flush();
            return 0;
        }
        if (m == "--argv-probe") {
            std::cout << "argv-probe:" << (argc - 2) << ":";
            for (int i = 2; i < argc; i++) {
                std::cout << argv[i] << (i + 1 < argc ? ":" : "");
            }
            std::cout << "\n";
            std::cout.flush();
            return 0;
        }
    }

    int pass = 0, fail = 0, sk = 0;
    for (int i = 0; i < 24; i++) {
        Res res;
        res.kind = 2;
        CHECKS[i](res);
        std::string clean;
        for (char c : res.detail) {
            clean.push_back((c == '\t' || c == '\r' || c == '\n') ? ' ' : c);
        }
        if (clean.size() > 160) {
            clean = clean.substr(0, 160);
        }
        const char *st = res.kind == 0 ? "PASS" : (res.kind == 1 ? "SKIP" : "FAIL");
        if (res.kind == 0) {
            pass++;
        } else if (res.kind == 1) {
            sk++;
        } else {
            fail++;
        }
        std::cout << "CAP " << IDS[i] << " " << st << " " << clean << "\n";
        std::cout.flush();
    }

    std::cout << "SUMMARY cpp " << pass << " " << fail << " " << sk << "\n";
    std::cout.flush();
    return fail > 0 ? 1 : 0;
}
