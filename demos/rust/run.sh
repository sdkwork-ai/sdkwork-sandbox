#!/usr/bin/env bash
# Rust 探针入口。cygpath 说明见 demos/python/run.sh。
#
# 直接用 rustc 编译单文件：**不走 cargo**，因此不需要 Cargo.toml、不碰 registry、
# 也不需要网络。这与 SPEC 的"只用标准库"约束一致。
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

RUSTC=""
if command -v rustc >/dev/null 2>&1; then
  RUSTC=rustc
fi

if [ -z "${RUSTC}" ]; then
  echo "UNAVAILABLE rustc not on PATH"
  exit 2
fi

OUT="${PWD_POSIX}/_rustout"
OUT_NATIVE="${PWD_NATIVE}/_rustout"
rm -rf "${OUT}"
mkdir -p "${OUT}"

BIN="probe"
case "$(uname -s 2>/dev/null)" in
  MINGW*|MSYS*|CYGWIN*) BIN="probe.exe" ;;
esac

# 输出路径必须给**原生**形式：MSYS 会把 `/d/...` 原样交给 rustc.exe，
# 后者按当前盘符解析成 `D:/d/...`（实测报 "couldn't create a temp dir"）。
if ! "${RUSTC}" -O --edition 2021 -o "${OUT_NATIVE}/${BIN}" "${HERE}/probe.rs" \
        >"${PWD_POSIX}/_rustc.log" 2>&1 ; then
  echo "rust: rustc failed (see _rustc.log)" >&2
  tail -25 "${PWD_POSIX}/_rustc.log" >&2
  exit 1
fi

exec "${OUT_NATIVE}/${BIN}" "$@"
