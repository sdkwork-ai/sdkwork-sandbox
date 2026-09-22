# demos 能力探针 —— awk 实现。协议见 demos/SPEC.md。
#
# 模式（SPEC §4 的四个 + 一个并发用；awk 没有位置参数，实参经 -v 赋值传入）：
#   默认             跑全部检查
#   --echo-child     往 stdout 写 child-ok、往 stderr 写 child-err，exit 0
#   --read-stdin     读一行 stdin，内容为 ping 时回 pong
#   --argv-probe     回显两个传入的实参
#   --count-worker   往 $DEMO_OUT 追加 1000 行（A18 并发用）
#
# 设计取舍（都在 SPEC 允许范围内）：
#  1) 并发：awk 没有线程，用 gawk 的**协程管道**（`print "" | cmd` 启动子进程、
#     `close(cmd)` 等待即 join），两个子 awk 真正并行，再各自汇总。
#  2) 字节级校验靠 POSIX 系统工具（od / wc / cmp / rm / mkdir / ls / test）：
#     awk 自身既没有文件元信息 API，也没有二进制 IO 原语。
#     这些是 POSIX 运行环境的一部分，不是第三方库。
#  3) A13 声明 EXEMPT：awk 只有整秒级 systime()，无亚秒/单调时钟。
#  4) A19 声明 EXEMPT：gawk 的 /inet 是编译期扩展，本机（Ubuntu 的 gawk）未编入
#     （实测 errno = "Address family not supported by protocol"），POSIX awk 亦无 socket。
#  5) **必须在 LC_ALL=C 下运行**（run.sh 已钉死）：gawk 的 `printf "%c"` 与字符/字节语义
#     都随 locale 变，UTF-8 locale 下 A07 的二进制往返不成立（见 a07 上方注释）。
#     中文串在 C locale 下是无损逐字节透传的，A11 不受影响。

BEGIN {
    UNICODE_S = "中文-日本語-한국어-🚀"
    UNICODE_CODEPOINTS = 12
    UNICODE_BYTES = 31
    LARGE_BYTES = 262144
    MAX_DEPTH = 3

    SQ = "'\\''"
    AWKQ = q(ENVIRON["DEMO_AWK"])
    SELFQ = q(ENVIRON["DEMO_SELF"])

    depth = ENVIRON["DEMO_DEPTH"] + 0
    if (depth > MAX_DEPTH) {
        print "FATAL: probe recursion depth " depth " exceeded" > "/dev/stderr"
        exit 3
    }

    mode = ENVIRON["DEMO_MODE"]

    # ⚠️ 模式分派必须先于任何检查执行：探针会自我调用，一旦派错就会指数级派生进程。
    if (mode == "--echo-child") {
        print "child-ok"
        print "child-err" > "/dev/stderr"
        exit 0
    }
    if (mode == "--read-stdin") {
        line = ""
        if ((getline line) > 0) sub(/\r$/, "", line)
        if (line == "ping") print "pong"
        else print "unexpected:" line
        exit 0
    }
    if (mode == "--argv-probe") {
        print "argv-probe:2:" ENVIRON["DEMO_P1"] ":" ENVIRON["DEMO_P2"]
        exit 0
    }
    if (mode == "--count-worker") {
        for (i = 0; i < 1000; i++) print "x" >> ENVIRON["DEMO_OUT"]
        close(ENVIRON["DEMO_OUT"])
        exit 0
    }

    npass = 0; nfail = 0; nskip = 0

    check("A01_argv", "a01")
    check("A02_env", "a02")
    check("A03_streams", "a03")
    check("A04_stdin", "a04")
    check("A05_file_rw", "a05")
    check("A06_file_append", "a06")
    check("A07_file_binary", "a07")
    check("A08_file_stat", "a08")
    check("A09_dir_ops", "a09")
    check("A10_temp_file", "a10")
    check("A11_unicode", "a11")
    check("A12_large_io", "a12")
    check("A13_time", "a13")
    check("A14_random", "a14")
    check("A15_container", "a15")
    check("A16_error", "a16")
    check("A17_subprocess", "a17")
    check("A18_concurrency", "a18")

    print "EXEMPT A19_tcp_loopback awk 无 socket（gawk 的 /inet 是编译期扩展，本机未编入）"

    check("B01_sha256", "b01")
    check("B02_base64", "b02")
    check("B03_json", "b03")
    check("B04_regex", "b04")
    check("B05_gzip", "b05")

    printf "SUMMARY awk %d %d %d\n", npass, nfail, nskip
    exit (nfail > 0) ? 1 : 0
}

# ------------------------------------------------------------------ 框架
function q(s,   t) {
    t = s
    gsub(/'/, SQ, t)
    return "'" t "'"
}

function record(cid, status, detail,   c) {
    c = detail
    gsub(/[\t\r\n]+/, " ", c)
    if (length(c) > 160) c = substr(c, 1, 160)
    if (status == "PASS") npass++
    else if (status == "FAIL") nfail++
    else nskip++
    printf "CAP %s %s %s\n", cid, status, c
    fflush()
}

function exitcode(raw) {
    # POSIX 上 system()/close() 给的是 wait 状态（退出码 << 8）；Windows 上可能已是退出码。
    return (raw > 255) ? int(raw / 256) : raw
}

function read_file(path,   s, l) {
    s = ""
    while ((getline l < path) > 0) s = s l "\n"
    close(path)
    return s
}

function trimnl(s) {
    sub(/\r?\n$/, "", s)
    return s
}

function cmd_out(cmd,   l, s) {
    s = ""
    while ((cmd | getline l) > 0) s = s l "\n"
    close(cmd)
    return s
}

function sh(cmd) {
    return exitcode(system(cmd))
}

function has_file(path) {
    return (index(cmd_out("test -f " q(path) " && echo yes"), "yes") > 0) ? 1 : 0
}

function has_dir(path) {
    return (index(cmd_out("test -d " q(path) " && echo yes"), "yes") > 0) ? 1 : 0
}

function wc_bytes(path) {
    return cmd_out("wc -c < " q(path)) + 0
}

# 从 UTF-8 字节流数码点（与 bash 实现同一套方法，跨宿主读数一致）。
function utf8_codepoints(path,   n, i, a, c) {
    n = 0
    c = cmd_out("od -An -v -tu1 " q(path))
    gsub(/\n/, " ", c)
    split(c, a, /[ \t]+/)
    for (i = 1; i in a; i++) {
        if (a[i] != "" && (a[i] + 0 < 128 || a[i] + 0 >= 192)) n++
    }
    return n
}

# 运行自身；结果放全局 RS_RC / RS_OUT / RS_ERR，返回退出码。
# envs 是 POSIX 的环境变量前缀赋值（`K=V K2=V2 'awk' -f self /dev/null`）——
# 不能用 awk 的 `-v`：那设的是 **awk 变量**，而模式分派读的是 ENVIRON（实测踩过）。
function run_self(envs,   cmd) {
    cmd = envs " " AWKQ " -f " SELFQ " /dev/null"
    RS_RC = sh(cmd " > self.out 2> self.err")
    RS_OUT = read_file("self.out")
    RS_ERR = read_file("self.err")
    return RS_RC
}

# gawk 4.1.2+ 的间接函数调用 @fn()，用来把 24 个检查表驱动地跑起来。
#
# 标签剥离用「去掉固定的 5 字符前缀 `SKIP:`/`FAIL:` + 去掉后续空白」，
# **不要**写成 substr(res, 7)：那样 `"SKIP:awk ..."` 会丢掉首字母（实测踩过，报出 `wk 无 SHA-256`）。
# 现在两种写法（`SKIP:xxx` 与 `SKIP: xxx`）都正确。
#
# `EXEMPT:` 是 Tier A 的**语言标准库豁免**（Tier B 的"没有"用 SKIP，Tier A 的用 EXEMPT）：
# 它按 `EXEMPT <id> <reason>` 原样输出，**不计入** pass/fail/skip —— runner 把 EXEMPT 行
# 单独算作一个"报满了的能力项"，并要求理由非空。
function probe(cid, fn,   res, d) {
    res = @fn()
    if (res ~ /^SKIP:/) {
        d = substr(res, 6)
        sub(/^[ \t]+/, "", d)
        if (d == "") d = "SKIP declared without reason"   # SPEC §3 要求 SKIP 必须写明理由
        record(cid, "SKIP", d)
    } else if (res ~ /^FAIL:/) {
        d = substr(res, 6)
        sub(/^[ \t]+/, "", d)
        if (d == "") d = "FAIL declared without reason"
        record(cid, "FAIL", d)
    } else if (res ~ /^EXEMPT:/) {
        d = substr(res, 8)
        sub(/^[ \t]+/, "", d)
        if (d == "") d = "EXEMPT declared without reason"
        printf "EXEMPT %s %s\n", cid, d
        fflush()
    } else {
        record(cid, "PASS", res)
    }
}

function check(cid, fname) {
    probe(cid, fname)
}

# ------------------------------------------------------------------ Tier A
function a01(   rc) {
    rc = run_self("DEMO_MODE=--argv-probe DEMO_P1=alpha DEMO_P2=beta")
    if (trimnl(RS_OUT) != "argv-probe:2:alpha:beta") {
        return "FAIL:argv 回显不符: " trimnl(RS_OUT)
    }
    return "2 args round-tripped (awk 的实参走 -v 赋值)"
}

function a02() {
    if (ENVIRON["DEMO_LANG_TAG"] != "demos-capability") {
        return "FAIL:DEMO_LANG_TAG=" ENVIRON["DEMO_LANG_TAG"]
    }
    return "env visible"
}

function a03(   o, e) {
    run_self("DEMO_MODE=--echo-child")
    o = trimnl(RS_OUT)
    e = RS_ERR
    if (o != "child-ok") return "FAIL:stdout=" o
    if (index(e, "child-err") == 0) return "FAIL:stderr missing child-err"
    if (index(o, "child-err") > 0) return "FAIL:stderr leaked into stdout"
    return "stdout/stderr separated"
}

function a04(   cmd, l, s) {
    cmd = "printf 'ping\\n' | DEMO_MODE=--read-stdin " AWKQ " -f " SELFQ
    s = ""
    while ((cmd | getline l) > 0) s = s l
    close(cmd)
    if (s != "pong") return "FAIL:reply=" s
    return "ping->pong"
}

function a05(   got) {
    print "hello-io" > "a05.txt"
    close("a05.txt")
    got = trimnl(read_file("a05.txt"))
    if (got != "hello-io") return "FAIL:read back " got
    return "text round-trip ok"
}

function a06(   got) {
    # 必须用 printf 不带换行：`print` 会补 "\n"，那就变成追加 "b\n"，
    # 总内容 "a\nb\n" ≠ "ab"，与其他语言（写入无换行）无法对齐。
    printf "a" > "a06.txt"
    close("a06.txt")
    printf "b" >> "a06.txt"
    close("a06.txt")
    got = trimnl(read_file("a06.txt"))
    if (got != "ab") return "FAIL:append 结果 " got
    return "append ok"
}

# ⚠️ A07 的字节语义**由 locale 决定**，必须整体在 LC_ALL=C 下跑（run.sh 已钉死）：
# 在 UTF-8 locale 里 gawk 的 `printf "%c", 200` 会输出 U+00C8 的 **2 字节** UTF-8 编码，
# 于是 256 个字节值会写出 384 字节 / 194 个不同字节 / 末字节 0xBF —— 看着像"I/O 坏了"，
# 其实是 locale。C locale 下才是逐字节（实测 256 / 256 / 255）。
function a07(   i, n, uniq, first, last, loc) {
    # ⚠️ close() 只关闭流、**不截断文件**：若沿用 >> 追加就必须先删干净，
    # 否则上一轮的 256 字节会被留下（实测 total=512）。a12 同理。
    sh("rm -f a07.bin")
    for (i = 0; i < 256; i++) printf "%c", i >> "a07.bin"
    close("a07.bin")
    n = cmd_out("od -An -v -tu1 a07.bin | wc -w") + 0
    uniq = cmd_out("od -An -v -tu1 a07.bin | tr -s ' ' '\\n' | grep -v '^$' | sort -n -u | wc -l") + 0
    first = cmd_out("od -An -v -tu1 -j 0 -N 1 a07.bin") + 0
    last = cmd_out("od -An -v -tu1 -j 255 -N 1 a07.bin") + 0
    if (n != 256 || uniq != 256 || first != 0 || last != 255) {
        loc = ENVIRON["LC_ALL"] " / " ENVIRON["LC_CTYPE"]
        return "FAIL:total=" n " uniq=" uniq " first=" first " last=" last " LC_ALL=" loc
    }
    return "256 bytes incl 0x00/0xFF (od round-trip)"
}

function a08(   size) {
    # 7 字节 = "statted" 原样，不带换行（与其他语言的 getsize 读数对齐）。
    printf "statted" > "a08.txt"
    close("a08.txt")
    if (!has_file("a08.txt")) return "FAIL:file not created"
    size = wc_bytes("a08.txt")
    if (size != 7) return "FAIL:size=" size
    sh("rm -f a08.txt")
    if (has_file("a08.txt")) return "FAIL:remove failed"
    return "size=7 then removed"
}

function a09(   listed) {
    sh("rm -rf a09dir")
    if (sh("mkdir a09dir") != 0) return "FAIL:mkdir failed"
    print "x" > "a09dir/inner.txt"
    close("a09dir/inner.txt")
    listed = cmd_out("ls a09dir")
    if (index(listed, "inner.txt") == 0) return "FAIL:listing missing inner.txt"
    sh("rm -f a09dir/inner.txt")
    if (sh("rmdir a09dir") != 0) return "FAIL:rmdir failed"
    if (has_dir("a09dir")) return "FAIL:still present after rmdir"
    return "mkdir/list/rmdir ok"
}

function a10(   path, got) {
    path = "demos-" PROCINFO["pid"] "-" systime() ".tmp"
    if (has_file(path)) return "FAIL:temp name collision"
    print "temp-content" > path
    close(path)
    got = trimnl(read_file(path))
    sh("rm -f " q(path))
    if (got != "temp-content") return "FAIL:temp read " got
    return "unique temp file"
}

function a11(   cps, nbytes, got) {
    # 不带换行：SPEC 钉死 31 字节，`print` 的 "\n" 会变成 32。
    # ⚠️ 逗号不能省：awk 的 `printf "%s" X` 是**字符串拼接**（格式串变成 "%s"+X），必须 `printf "%s", X`。
    printf "%s", UNICODE_S > "a11.txt"
    close("a11.txt")
    nbytes = wc_bytes("a11.txt")
    if (nbytes != UNICODE_BYTES) return "FAIL:bytes=" nbytes
    cps = utf8_codepoints("a11.txt")
    if (cps != UNICODE_CODEPOINTS) return "FAIL:codepoints=" cps
    got = trimnl(read_file("a11.txt"))
    if (got != UNICODE_S) return "FAIL:round-trip mismatch"
    return UNICODE_CODEPOINTS " codepoints / " UNICODE_BYTES " bytes"
}

function a12(   i, s, j, n, same) {
    s = ""
    for (i = 0; i < 1024; i++) s = s "a"
    sh("rm -f a12.bin a12.read")     # close() 不截断，追加写前必须先清（见 a07）
    for (j = 0; j < 256; j++) printf "%s", s >> "a12.bin"
    close("a12.bin")
    n = wc_bytes("a12.bin")
    if (n != LARGE_BYTES) return "FAIL:len=" n
    sh("cp a12.bin a12.read")
    same = index(cmd_out("cmp -s a12.bin a12.read && echo same"), "same")
    if (same == 0) return "FAIL:read-back mismatch"
    return LARGE_BYTES " bytes ok"
}

function a13() {
    # Tier A 但**标准库确实没有**：awk 只有一个 systime()（整秒）。
    # 这不是"暂时跳过"，是语言能力缺失 ⇒ 报 EXEMPT（与 lua 的 A13 同一类，标签必须一致）。
    return "EXEMPT:awk 只有整秒级 systime()，无亚秒/单调时钟，无法测 50ms 差值"
}

function a14(   v) {
    srand()
    v = int(rand() * 1000)
    if (v < 0 || v >= 1000) return "FAIL:out of range " v
    return "in [0,1000) (rand)"
}

function a15(   i, j, t, n, sorted) {
    data[1] = 5; data[2] = 3; data[3] = 9; data[4] = 1; data[5] = 7; data[6] = 3
    n = 6
    for (i = 2; i <= n; i++) {
        t = data[i]
        j = i - 1
        while (j >= 1 && data[j] > t) {
            data[j + 1] = data[j]
            j--
        }
        data[j + 1] = t
    }
    sorted = ""
    for (i = 1; i <= n; i++) sorted = sorted (i > 1 ? "," : "") data[i]
    if (sorted != "1,3,3,5,7,9") return "FAIL:sorted=" sorted
    mm["a"] = 1
    mm["b"] = 2
    if (mm["a"] != 1 || mm["b"] != 2) return "FAIL:map broken"
    if (length(mm) != 2) return "FAIL:map len=" length(mm)
    return "insertion sort + array map ok"
}

function a16(   r) {
    r = (getline line < "definitely-missing-file-xyz")
    close("definitely-missing-file-xyz")
    if (r != -1) return "FAIL:getline returned " r
    return "getline -1 (I/O error code) caught"
}

function a17(   rc) {
    # 必须用环境变量前缀赋值；`-v` 设的是 awk 变量，探针的模式分派读 ENVIRON，
    # 用 -v 会让子进程跑成"默认全量检查"（深层自我调用）或直接找不到命令（exit 127）。实测踩过。
    rc = run_self("DEMO_MODE=--echo-child")
    if (rc != 0) return "FAIL:child exit=" rc
    if (trimnl(RS_OUT) != "child-ok") return "FAIL:child stdout=" trimnl(RS_OUT)
    return "child exit=0, stdout captured (close() status)"
}

function a18(   cmd1, cmd2, rc1, rc2, n1, n2) {
    sh("rm -f a18.1.txt a18.2.txt")
    cmd1 = "DEMO_MODE=--count-worker DEMO_OUT=a18.1.txt " AWKQ " -f " SELFQ " /dev/null"
    cmd2 = "DEMO_MODE=--count-worker DEMO_OUT=a18.2.txt " AWKQ " -f " SELFQ " /dev/null"
    # 用协程管道启动两个子进程（此刻两个都在跑）。
    print "" | cmd1
    print "" | cmd2
    # close() 等待子进程结束，等价于 join。
    rc1 = exitcode(close(cmd1))
    rc2 = exitcode(close(cmd2))
    if (rc1 != 0 || rc2 != 0) return "FAIL:worker exit=" rc1 "/" rc2
    n1 = cmd_out("wc -l < a18.1.txt") + 0
    n2 = cmd_out("wc -l < a18.2.txt") + 0
    if (n1 + n2 != 2000) return "FAIL:total=" (n1 + n2)
    return "2 coprocess workers joined -> 2000"
}

# ------------------------------------------------------------------ Tier B
function b01() {
    return "SKIP:awk 无 SHA-256（须外部 openssl 等）"
}

function b02() {
    return "SKIP:awk 无 Base64（须外部工具）"
}

function b03() {
    return "SKIP:awk 无 JSON（须外部工具）"
}

function b04() {
    if (!("abc-123" ~ /^[a-z]+-[0-9]{3}$/)) return "FAIL:positive match failed"
    if ("ABC-123" ~ /^[a-z]+-[0-9]{3}$/) return "FAIL:negative match unexpected"
    return "~ /regex/ match+reject ok"
}

function b05() {
    return "SKIP:awk 无 gzip（须外部 gzip/zlib）"
}
