#!/usr/bin/env bash
# Go 探针入口。cygpath 说明见 demos/python/run.sh。
#
# go 在两个宿主上的位置不同：
#   - Windows：winget 装的 GoSDK 在 "%ProgramFiles%\Go\bin"，但**不写进用户 PATH**，
#              所以这里用环境变量拼出候选（不写死绝对路径，避免踩路径可移植性门禁）。
#   - WSL：/usr/local/go/bin 或 /usr/lib/go/bin。
#
# 单文件构建需要 module 上下文，所以先在自己的构建目录里写一个最小 go.mod
# （无任何依赖 ⇒ 不需要网络，也不会去碰用户的 module 缓存）。
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

GO=""
if command -v go >/dev/null 2>&1; then
  GO=go
else
  for cand in \
      "${ProgramFiles:-}/Go/bin/go" \
      "${PROGRAMFILES:-}/Go/bin/go" \
      /usr/local/go/bin/go \
      /usr/lib/go/bin/go ; do
    case "${cand}" in
      /Go/bin/go) continue ;;                 # 两个环境变量都没设时的空拼接
    esac
    if [ -x "${cand}" ]; then
      GO="${cand}"
      break
    fi
  done
fi

if [ -z "${GO}" ]; then
  echo "UNAVAILABLE go not found on PATH or standard install roots"
  exit 2
fi

BUILD="${PWD_POSIX}/_gobuild"
OUT="${PWD_POSIX}/_goout"
rm -rf "${BUILD}" "${OUT}"
mkdir -p "${BUILD}" "${OUT}"
cp "${HERE}/probe.go" "${BUILD}/probe.go"
# go.mod 的 go 指令取**两个宿主的公共下限**：WSL 是 go1.18.1（发行版包），
# Windows 是 go1.27，写 1.21 会让 1.18 直接拒绝。1.18 同时是 `-buildvcs` 的下限。
printf 'module demoprobe\n\ngo 1.18\n' > "${BUILD}/go.mod"

export GO111MODULE=on
export GOFLAGS=-mod=mod
export GOPROXY=off
export GOTOOLCHAIN=local

BIN="probe"
case "$(uname -s 2>/dev/null)" in
  MINGW*|MSYS*|CYGWIN*) BIN="probe.exe" ;;
esac

# 编译失败 = 探针自己的问题，不是"本机没有 Go"（见 demos/csharp/run.sh 的同款说明）。
#
# ⚠️ `-buildvcs=false` 是必须的：构建目录落在**本仓工作树内**，Go 会去取 VCS 状态，
# 而 git 在"仓库属主与当前用户不一致"时会拒绝，报
#   error obtaining VCS status: exit status 128 / Use -buildvcs=false to disable VCS stamping
# 于是探针一行 CAP 都发不出来（WSL 上以 root 跑实测踩过）。关掉 VCS 盖章即可，
# 本探针也不需要 build info。
if ! ( cd "${BUILD}" && "${GO}" build -buildvcs=false -o "${PWD_NATIVE}/_goout/${BIN}" . ) \
        >"${PWD_POSIX}/_gobuild.log" 2>&1 ; then
  echo "go: go build failed (see _gobuild.log)" >&2
  tail -25 "${PWD_POSIX}/_gobuild.log" >&2
  exit 1
fi

exec "${PWD_NATIVE}/_goout/${BIN}" "$@"
