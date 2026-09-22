#!/usr/bin/env bash
# awk 探针入口。
#
# awk 没有位置参数（argv 里除选项外都是输入文件），所以模式与实参经 -v 赋值传入；
# 默认检查跑 `/dev/null` 作为唯一输入，避免 awk 去读本脚本的 stdin。
#
# 需要 gawk（GNU awk）：探针用到协程管道、PROCINFO、间接函数调用 @fn()、length(数组)，
# 这些都不是 POSIX awk 的特性。本机两宿主的 awk 都是 gawk（MSYS 5.4 / Ubuntu 5.1）。
#
# ⚠️ LC_ALL=C 是**必须的**，不是保险：gawk 的 `printf "%c", N` 在 UTF-8 locale 下对 N>=128
# 会输出 UTF-8 编码（2 字节），A07 的 256 字节二进制往返就会得到 384 字节 / 194 个不同值。
# C locale 下才是逐字节语义，与 python/node 等语言的 raw byte IO 对齐。
set -u

LC_ALL=C
export LC_ALL

HERE_POSIX="$(cd "$(dirname "$0")" && pwd)"
HERE="${HERE_POSIX}"
# 工作目录必须钉在 <repo>/target/demos-work/<lang>（SPEC §6）：探针的临时产物
# 绝不落进仓库。runner 本来就会 cd 到这里，但**直接 `bash demos/<lang>/run.sh`**
# 时不会 —— 实测直接跑会把 a05.txt / a07.bin / self.out 等生成物拉在 demos/<lang>/ 里。
DEMO_ROOT="$(cd "${HERE_POSIX}/../.." && pwd)"
DEMO_WORK="${DEMO_ROOT}/target/demos-work/$(basename "${HERE_POSIX}")"
mkdir -p "${DEMO_WORK}"
cd "${DEMO_WORK}"
if command -v cygpath >/dev/null 2>&1; then
  HERE="$(cygpath -w "${HERE_POSIX}")"
fi

AWK_BIN=""
if command -v gawk >/dev/null 2>&1; then
  AWK_BIN="$(command -v gawk)"
elif command -v awk >/dev/null 2>&1; then
  AWK_BIN="$(command -v awk)"
fi

if [ -z "${AWK_BIN}" ]; then
  echo "UNAVAILABLE awk/gawk not on PATH"
  exit 2
fi

# 必须能跑且是 gawk（探针依赖 gawk 扩展）。
if ! "${AWK_BIN}" --version 2>&1 | head -1 | grep -q 'GNU Awk'; then
  echo "UNAVAILABLE awk is not GNU awk (gawk); the probe needs gawk extensions"
  exit 2
fi

DEMO_AWK="${AWK_BIN}"
DEMO_SELF="${HERE}/probe.awk"
export DEMO_AWK DEMO_SELF

# --read-stdin 时让 awk 读 stdin（不给文件操作数）；其余模式用 /dev/null 占位。
# 模式与实参必须走**真环境变量**：awk 的 `-v` 只设 awk 变量，而探针读的是 ENVIRON。
if [ "${1:-}" = "--read-stdin" ]; then
  DEMO_MODE=--read-stdin
  export DEMO_MODE
  exec "${AWK_BIN}" -f "${HERE}/probe.awk"
fi

if [ -n "${1:-}" ]; then
  # 例如 --argv-probe alpha beta → 按位置映射到 DEMO_P1/DEMO_P2
  DEMO_MODE="$1"
  DEMO_P1="${2:-}"
  DEMO_P2="${3:-}"
  export DEMO_MODE DEMO_P1 DEMO_P2
  exec "${AWK_BIN}" -f "${HERE}/probe.awk" /dev/null
fi

exec "${AWK_BIN}" -f "${HERE}/probe.awk" /dev/null
