#!/usr/bin/env bash
# C# / .NET 探针入口。cygpath 说明见 demos/python/run.sh。
#
# .NET 8 没有 "dotnet run file.cs"（那是 .NET 10 的文件式应用），所以要建一个最小工程。
# 工程文件与源码从本目录复制到**当前工作目录**（runner 已把它设为
# target/demos-work/csharp），因此 obj/ bin/ 等编译中间物全部落在 gitignore 的 target/ 下。
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

if ! command -v dotnet >/dev/null 2>&1; then
  echo "UNAVAILABLE dotnet not on PATH"
  exit 2
fi

BUILD="${PWD_POSIX}/_csbuild"
OUT="${PWD_POSIX}/_csout"
LOG="${PWD_POSIX}/_csbuild.log"
rm -rf "${BUILD}" "${OUT}"
mkdir -p "${BUILD}"
cp "${HERE}/Demo.csproj" "${BUILD}/Demo.csproj"
cp "${HERE}/Probe.cs"     "${BUILD}/Probe.cs"

export DOTNET_CLI_TELEMETRY_OPTOUT=1
export DOTNET_NOLOGO=1
export DOTNET_SKIP_FIRST_TIME_EXPERIENCE=1
export DOTNET_CLI_UI_LANGUAGE=en

# 编译失败 = 探针自己的问题，不是"本机没有 .NET"；所以不要报 UNAVAILABLE，
# 让它以"无 CAP 输出"的形式暴露出来（runner 会把它列进"未跑通的语言"并置退出码 1）。
if ! dotnet build "${PWD_NATIVE}/_csbuild/Demo.csproj" -c Release -o "${PWD_NATIVE}/_csout" \
        --nologo -v quiet >"${LOG}" 2>&1; then
  echo "csharp: dotnet build failed (see _csbuild.log)" >&2
  tail -25 "${LOG}" >&2
  exit 1
fi

exec dotnet "${PWD_NATIVE}/_csout/demo-probe.dll" "$@"
