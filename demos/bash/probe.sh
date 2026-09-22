#!/usr/bin/env bash
# demos 能力探针 —— Bash 实现。协议见 demos/SPEC.md。
#
# 模式（照抄 SPEC §4，跨语言必须同名）：
#   默认              跑全部检查
#   --echo-child      往 stdout 写 child-ok、往 stderr 写 child-err，exit 0
#   --read-stdin      读一行 stdin，内容为 ping 时回 pong
#   --argv-probe A B  回显 argv（argv-probe:2:A:B）
#   --count-worker F  往文件 F 追加 1000 行（A18 并发用）
#
# 本脚本会被 check-shell-portability.mjs 扫描（bash 3.2 基线），所以：
#   - 不用关联数组（declare -A）、mapfile、${var,,}、|& 等 bash 4+ 语法；
#     A15 的 map 用「平行索引数组 + 查找函数」代替（门禁提示的正是这个写法）。
#   - 不用 GNU-only 的 stat -c / date -d / readlink -f / timeout N 等。
#
# 两点与脚本语言的差异（都在 SPEC 允许范围内）：
#   1) A11 的「字符数」不能用 ${#s}：MSYS 的 bash 按 UTF-16 单元数（13），
#      Linux 的 bash 按码点数（12）——实测两宿主不一致。这里改成从字节流
#      数「非续接字节」，两宿主都得到 12，读数可逐位比对。
#   2) A19 声明 EXEMPT：bash 的 /dev/tcp 只有 connect（客户端）形态，
#      没有 bind/listen，单靠 shell 无法做回环的服务端。
set -u

SELF="${DEMO_SELF:-$0}"

UNICODE_S='中文-日本語-한국어-🚀'
UNICODE_CODEPOINTS=12
UNICODE_BYTES=31
LARGE_BYTES=262144
SHA256_ABC='ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad'
MAX_DEPTH=3

depth="${DEMO_DEPTH:-0}"
pass=0
fail=0
skip=0

clean() {
    printf '%s' "$1" | tr '\t\r\n' '   '
}

emit() {
    printf 'CAP %s %s %s\n' "$1" "$2" "$(clean "$3")"
}

check() {
    cid="$1"
    fn="$2"
    detail="$("$fn" 2>&1)"
    rc=$?
    case "$rc" in
        0) pass=$((pass + 1)); emit "$cid" PASS "$detail" ;;
        1) fail=$((fail + 1)); emit "$cid" FAIL "$detail" ;;
        *) skip=$((skip + 1)); emit "$cid" SKIP "$detail" ;;
    esac
}

# ------------------------------------------------------------------ 小工具
# 退化为 SKIP（return 2）。
skip_now() {
    printf '%s' "$1"
    return 2
}

byte_count() {
    wc -c < "$1" | tr -d ' \r\n'
}

# 从 UTF-8 字节流数码点：非续接字节（高两位不是 10）的个数。
utf8_codepoints() {
    od -An -v -tu1 "$1" | awk '{ for (i = 1; i <= NF; i++) if ($i < 128 || $i >= 192) n++ } END { print n + 0 }'
}

# 用 bash 内建 printf 写出第 n 个字节（含 NUL：printf 的 \0nnn 转义）。
write_byte() {
    printf "\\$(printf '%03o' "$1")" >> "$2"
}

# ------------------------------------------------------------------ Tier A
a01() {
    out="$("$SELF" --argv-probe alpha beta 2>/dev/null)"
    if [ "$out" != "argv-probe:2:alpha:beta" ]; then
        printf 'argv 回显不符: %s' "$out"
        return 1
    fi
    printf '2 args round-tripped'
}

a02() {
    if [ "${DEMO_LANG_TAG:-}" != "demos-capability" ]; then
        printf 'DEMO_LANG_TAG=%s' "${DEMO_LANG_TAG:-<unset>}"
        return 1
    fi
    printf 'env visible'
}

a03() {
    "$SELF" --echo-child >c03.out 2>c03.err
    o="$(cat c03.out)"
    e="$(cat c03.err)"
    if [ "$o" != "child-ok" ]; then
        printf 'stdout=%s' "$o"
        return 1
    fi
    case "$e" in
        *child-err*) ;;
        *) printf 'stderr=%s' "$e"; return 1 ;;
    esac
    case "$o" in
        *child-err*) printf 'stderr leaked into stdout'; return 1 ;;
    esac
    printf 'stdout/stderr separated'
}

a04() {
    out="$(printf 'ping\n' | "$SELF" --read-stdin 2>/dev/null)"
    if [ "$out" != "pong" ]; then
        printf 'reply=%s' "$out"
        return 1
    fi
    printf 'ping->pong'
}

a05() {
    printf 'hello-io' >a05.txt
    out="$(cat a05.txt)"
    if [ "$out" != "hello-io" ]; then
        printf 'read back %s' "$out"
        return 1
    fi
    printf 'text round-trip ok'
}

a06() {
    printf 'a' >a06.txt
    printf 'b' >>a06.txt
    out="$(cat a06.txt)"
    if [ "$out" != "ab" ]; then
        printf 'append 结果 %s' "$out"
        return 1
    fi
    printf 'append ok'
}

a07() {
    : >a07.bin
    i=0
    while [ "$i" -lt 256 ]; do
        write_byte "$i" a07.bin
        i=$((i + 1))
    done

    nbytes=$(byte_count a07.bin)
    if [ "$nbytes" != "256" ]; then
        printf 'bytes=%s' "$nbytes"
        return 1
    fi
    # 读回校验：字节个数 + 取值集合恰好是 {0..255} + 首尾定位。
    total=$(od -An -v -tu1 a07.bin | wc -w | tr -d ' \r\n')
    uniq=$(od -An -v -tu1 a07.bin | tr -s ' ' '\n' | grep -v '^$' | sort -n -u | wc -l | tr -d ' \r\n')
    first=$(od -An -v -tu1 -j 0 -N 1 a07.bin | tr -d ' \r\n')
    last=$(od -An -v -tu1 -j 255 -N 1 a07.bin | tr -d ' \r\n')
    if [ "$total" != "256" ] || [ "$uniq" != "256" ] || [ "$first" != "0" ] || [ "$last" != "255" ]; then
        printf 'total=%s uniq=%s first=%s last=%s' "$total" "$uniq" "$first" "$last"
        return 1
    fi
    printf '256 bytes incl 0x00/0xFF (od round-trip)'
}

a08() {
    printf 'statted' >a08.txt
    [ -f a08.txt ] || { printf 'file not created'; return 1; }
    size=$(byte_count a08.txt)
    if [ "$size" != "7" ]; then
        printf 'size=%s' "$size"
        return 1
    fi
    rm -f a08.txt
    [ -f a08.txt ] && { printf 'remove failed'; return 1; }
    printf 'size=7 then removed'
}

a09() {
    rm -rf a09dir
    mkdir a09dir
    : >a09dir/inner.txt
    found=no
    for f in a09dir/*; do
        case "$f" in
            */inner.txt) found=yes ;;
        esac
    done
    if [ "$found" != "yes" ]; then
        printf 'listing missing inner.txt'
        return 1
    fi
    rm -f a09dir/inner.txt
    rmdir a09dir
    [ -d a09dir ] && { printf 'rmdir failed'; return 1; }
    printf 'mkdir/list/rmdir ok'
}

a10() {
    path="$(mktemp ./demos-XXXXXX)"
    if [ -z "$path" ]; then
        printf 'mktemp failed'
        return 1
    fi
    printf 'temp-content' >"$path"
    out="$(cat "$path")"
    rm -f "$path"
    if [ "$out" != "temp-content" ]; then
        printf 'temp read %s' "$out"
        return 1
    fi
    printf 'unique temp file (mktemp)'
}

a11() {
    printf '%s' "$UNICODE_S" >a11.txt
    nbytes=$(byte_count a11.txt)
    if [ "$nbytes" != "$UNICODE_BYTES" ]; then
        printf 'bytes=%s' "$nbytes"
        return 1
    fi
    cps=$(utf8_codepoints a11.txt)
    if [ "$cps" != "$UNICODE_CODEPOINTS" ]; then
        printf 'codepoints=%s' "$cps"
        return 1
    fi
    out="$(cat a11.txt)"
    if [ "$out" != "$UNICODE_S" ]; then
        printf 'round-trip mismatch'
        return 1
    fi
    printf '%s codepoints / %s bytes' "$UNICODE_CODEPOINTS" "$UNICODE_BYTES"
}

a12() {
    # 1 KiB 填充块重复 256 次 = 262144 字节（不用 yes/head -c：那会掺进换行）。
    awk 'BEGIN { s = ""; for (i = 0; i < 1024; i++) s = s "a"; for (j = 0; j < 256; j++) printf "%s", s }' >a12.bin
    n=$(byte_count a12.bin)
    if [ "$n" != "$LARGE_BYTES" ]; then
        printf 'len=%s' "$n"
        return 1
    fi
    cat a12.bin >a12.read
    if ! cmp -s a12.bin a12.read; then
        printf 'read-back mismatch'
        return 1
    fi
    printf '%s bytes ok' "$LARGE_BYTES"
}

a13() {
    now=$(date +%s)
    if [ "$now" -lt 1577836800 ]; then
        printf 'epoch=%s' "$now"
        return 1
    fi
    if [ -z "${EPOCHREALTIME:-}" ]; then
        printf 'bash 无 EPOCHREALTIME（bash 5+ 才有亚秒时钟）'
        return 1
    fi
    t0="$EPOCHREALTIME"
    sleep 0.05
    t1="$EPOCHREALTIME"
    delta=$(awk -v a="$t0" -v b="$t1" 'BEGIN { printf "%.1f", (b - a) * 1000 }')
    if ! awk -v d="$delta" 'BEGIN { exit (d >= 40) ? 0 : 1 }'; then
        printf 'sleep 只测到 %s ms' "$delta"
        return 1
    fi
    printf 'sleep %.0fms (bash 无单调时钟，用 EPOCHREALTIME)' "$delta"
}

a14() {
    v=$((RANDOM % 1000))
    if [ "$v" -lt 0 ] || [ "$v" -ge 1000 ]; then
        printf 'out of range %s' "$v"
        return 1
    fi
    printf 'in [0,1000) ($RANDOM)'
}

# 平行索引数组 + 线性查找：bash 3.2 没有关联数组（门禁也禁 declare -A）。
MAP_KEYS=''
MAP_VALS=''
map_set() {
    # 首个元素不加前导空格，否则 cut -f1 会取到空字段。
    if [ -z "$MAP_KEYS" ]; then
        MAP_KEYS="$1"
        MAP_VALS="$2"
    else
        MAP_KEYS="$MAP_KEYS $1"
        MAP_VALS="$MAP_VALS $2"
    fi
}
map_get() {
    k="$1"
    i=1
    for key in $MAP_KEYS; do
        if [ "$key" = "$k" ]; then
            printf '%s' "$(printf '%s' "$MAP_VALS" | cut -d' ' -f"$i")"
            return 0
        fi
        i=$((i + 1))
    done
    return 1
}
map_len() {
    n=0
    for _k in $MAP_KEYS; do
        n=$((n + 1))
    done
    printf '%s' "$n"
}

a15() {
    sorted="$(printf '5\n3\n9\n1\n7\n3\n' | sort -n | tr '\n' ',' | sed 's/,$//')"
    if [ "$sorted" != "1,3,3,5,7,9" ]; then
        printf 'sorted=%s' "$sorted"
        return 1
    fi
    map_set a 1
    map_set b 2
    if [ "$(map_get a)" != "1" ]; then
        printf 'map[a] broken'
        return 1
    fi
    if [ "$(map_get b)" != "2" ]; then
        printf 'map[b] broken'
        return 1
    fi
    if [ "$(map_len)" != "2" ]; then
        printf 'map len=%s' "$(map_len)"
        return 1
    fi
    printf 'sort -n + array-backed map ok'
}

a16() {
    if cat definitely-missing-file-xyz >/dev/null 2>&1; then
        printf 'no error raised for missing file'
        return 1
    fi
    printf 'non-zero status from cat caught'
}

a17() {
    out="$("$SELF" --echo-child 2>/dev/null)"
    rc=$?
    if [ "$rc" != "0" ]; then
        printf 'child exit=%s' "$rc"
        return 1
    fi
    if [ "$out" != "child-ok" ]; then
        printf 'child stdout=%s' "$out"
        return 1
    fi
    printf 'child exit=0, stdout captured'
}

a18() {
    : >a18.1.txt
    : >a18.2.txt
    ( i=0; while [ "$i" -lt 1000 ]; do printf 'x\n' >>a18.1.txt; i=$((i + 1)); done ) &
    p1=$!
    ( i=0; while [ "$i" -lt 1000 ]; do printf 'x\n' >>a18.2.txt; i=$((i + 1)); done ) &
    p2=$!
    wait "$p1"
    r1=$?
    wait "$p2"
    r2=$?
    if [ "$r1" != "0" ] || [ "$r2" != "0" ]; then
        printf 'worker exit=%s/%s' "$r1" "$r2"
        return 1
    fi
    n1=$(wc -l <a18.1.txt | tr -d ' \r\n')
    n2=$(wc -l <a18.2.txt | tr -d ' \r\n')
    total=$((n1 + n2))
    if [ "$total" -ne 2000 ]; then
        printf 'total=%s' "$total"
        return 1
    fi
    printf '2 background jobs joined -> 2000'
}

# ------------------------------------------------------------------ Tier B
b01() {
    # bash 没有内置 SHA-256；外部只有 coreutils 的哈希工具（属第三方依赖，SPEC §2 禁用）。
    skip_now 'bash 无内置 SHA-256（须外部 coreutils 哈希工具）'
}

b02() {
    skip_now 'bash 无内置 Base64（须外部 base64 工具）'
}

b03() {
    skip_now 'bash 无内置 JSON（须外部 jq/python 等）'
}

b04() {
    if [[ ! "abc-123" =~ ^[a-z]+-[0-9]{3}$ ]]; then
        printf 'positive match failed'
        return 1
    fi
    if [[ "ABC-123" =~ ^[a-z]+-[0-9]{3}$ ]]; then
        printf 'negative match unexpected'
        return 1
    fi
    printf '[[ =~ ]] match+reject ok'
}

b05() {
    skip_now 'bash 无内置 gzip（须外部 gzip 工具）'
}

# -------------------------------------------------------------------- 主流程
if [ "$depth" -gt "$MAX_DEPTH" ]; then
    printf 'FATAL: probe recursion depth %s exceeded\n' "$depth" >&2
    exit 3
fi

# ⚠️ 模式分派必须先于任何检查执行：探针会自我调用，一旦派错就会指数级派生进程。
case "${1:-}" in
    --echo-child)
        printf 'child-ok\n'
        printf 'child-err\n' >&2
        exit 0
        ;;
    --read-stdin)
        IFS= read -r line
        if [ "$line" = "ping" ]; then
            printf 'pong\n'
        else
            printf 'unexpected:%s\n' "$line"
        fi
        exit 0
        ;;
    --argv-probe)
        shift
        n=$#
        s=''
        first=1
        for a in "$@"; do
            if [ "$first" = "1" ]; then
                s="$a"
                first=0
            else
                s="$s:$a"
            fi
        done
        printf 'argv-probe:%s:%s\n' "$n" "$s"
        exit 0
        ;;
    --count-worker)
        f="$2"
        i=0
        while [ "$i" -lt 1000 ]; do
            printf 'x\n' >>"$f"
            i=$((i + 1))
        done
        exit 0
        ;;
esac

check A01_argv a01
check A02_env a02
check A03_streams a03
check A04_stdin a04
check A05_file_rw a05
check A06_file_append a06
check A07_file_binary a07
check A08_file_stat a08
check A09_dir_ops a09
check A10_temp_file a10
check A11_unicode a11
check A12_large_io a12
check A13_time a13
check A14_random a14
check A15_container a15
check A16_error a16
check A17_subprocess a17
check A18_concurrency a18

# A19 声明豁免：bash 的 /dev/tcp 只有 connect 形态，没有 bind/listen。
printf 'EXEMPT A19_tcp_loopback %s\n' \
  'bash 的 /dev/tcp 只有 connect（客户端），POSIX shell 无 bind/listen'

check B01_sha256 b01
check B02_base64 b02
check B03_json b03
check B04_regex b04
check B05_gzip b05

printf 'SUMMARY bash %s %s %s\n' "$pass" "$fail" "$skip"
if [ "$fail" -gt 0 ]; then
    exit 1
fi
exit 0
