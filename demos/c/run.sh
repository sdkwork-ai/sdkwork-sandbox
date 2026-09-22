#!/usr/bin/env bash
# C 探针入口。cygpath 说明见 demos/python/run.sh。
#
# 探针用 `system()` + shell 重定向来启动自身（见 demos/c/probe.c 的设计说明），
# 所以这里把二进制路径导出成 DEMO_SELF，避免依赖 argv[0] 在不同 shell 下的形态。
set -u

HERE_POSIX="$(cd "$(dirname "$0")" && pwd)"
HERE="${HERE_POSIX}"
# 工作目录必须钉在 <repo>/target/demos-work/<lang>（SPEC §6）：探针的临时产物
# 绝不落进仓库。runner 本来就会 cd 到这里，但**直接 `bash demos/<lang>/run.sh`**
# 时不会 —— 实测直接跑会把 a05.txt / a07.bin / self.out 等生成物拉在 demos/<lang>/ 里。
DEMO_ROOT="$(cd "${HERE_POSIX}/../.." && pwd)"
DEMO_WORK="${DEMO_ROOT}/target/demos-work/$(basename "${HERE_POSIX}")"
mkdir -p "${DEMO_WORK}"
cd "${DEMO_WORK}"
PWD_POSIX="$(pwd)"
PWD_NATIVE="${PWD_POSIX}"
if command -v cygpath >/dev/null 2>&1; then
  HERE="$(cygpath -w "${HERE_POSIX}")"
  PWD_NATIVE="$(cygpath -w "${PWD_POSIX}")"
fi

CC_BIN=""
if command -v cc >/dev/null 2>&1; then
  CC_BIN=cc
elif command -v gcc >/dev/null 2>&1; then
  CC_BIN=gcc
elif command -v clang >/dev/null 2>&1; then
  CC_BIN=clang
fi

if [ -z "${CC_BIN}" ]; then
  echo "UNAVAILABLE no C compiler (cc/gcc/clang) on PATH"
  exit 2
fi

OUT="${PWD_POSIX}/_cout"
OUT_NATIVE="${PWD_NATIVE}/_cout"
rm -rf "${OUT}"
mkdir -p "${OUT}"

BIN="probe"
LDLIBS="-pthread"
case "$(uname -s 2>/dev/null)" in
  MINGW*|MSYS*|CYGWIN*)
    BIN="probe.exe"
    LDLIBS="-pthread -lws2_32"
    ;;
esac

# 输出路径用原生形式（见 demos/rust/run.sh 里 MSYS 路径改写的说明）。
# ⚠️ 必须显式钉住字符集：本机 gcc 的**执行字符集**默认跟着系统 ANSI 代码页（CP936/GBK），
#    源码里的中文会被编成 GBK 字节，输出到 UTF-8 管道就成了乱码（实测 '标准库无' → '鏍囧噯搴撴棤'）。
# 编译失败 = 探针自己的问题，不是"本机没有 C 编译器"。
if ! "${CC_BIN}" -O2 -std=c11 -Wall -Wextra \
        -finput-charset=UTF-8 -fexec-charset=UTF-8 \
        -o "${OUT_NATIVE}/${BIN}" "${HERE}/probe.c" \
        ${LDLIBS} >"${PWD_POSIX}/_cc.log" 2>&1 ; then
  echo "c: compilation failed (see _cc.log)" >&2
  tail -30 "${PWD_POSIX}/_cc.log" >&2
  exit 1
fi

DEMO_SELF="${OUT_NATIVE}/${BIN}"
export DEMO_SELF
exec "${OUT_NATIVE}/${BIN}" "$@"
