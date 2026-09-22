#!/usr/bin/env bash
# C++ 探针入口。cygpath 说明见 demos/python/run.sh / demos/c/run.sh。
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

CXX_BIN=""
if command -v c++ >/dev/null 2>&1; then
  CXX_BIN=c++
elif command -v g++ >/dev/null 2>&1; then
  CXX_BIN=g++
elif command -v clang++ >/dev/null 2>&1; then
  CXX_BIN=clang++
fi

if [ -z "${CXX_BIN}" ]; then
  echo "UNAVAILABLE no C++ compiler (c++/g++/clang++) on PATH"
  exit 2
fi

OUT="${PWD_POSIX}/_cxxout"
OUT_NATIVE="${PWD_NATIVE}/_cxxout"
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

# 字符集必须显式钉住（同 demos/c/run.sh：本机 gcc 默认执行字符集是系统 ANSI 代码页）。
if ! "${CXX_BIN}" -O2 -std=c++17 -Wall -Wextra \
        -finput-charset=UTF-8 -fexec-charset=UTF-8 \
        -o "${OUT_NATIVE}/${BIN}" "${HERE}/probe.cpp" \
        ${LDLIBS} >"${PWD_POSIX}/_cxx.log" 2>&1 ; then
  echo "cpp: compilation failed (see _cxx.log)" >&2
  tail -30 "${PWD_POSIX}/_cxx.log" >&2
  exit 1
fi

DEMO_SELF="${OUT_NATIVE}/${BIN}"
export DEMO_SELF
exec "${OUT_NATIVE}/${BIN}" "$@"
