#!/usr/bin/env bash
# demos 跨语言能力对齐 runner。
#
# 契约（详见 demos/SPEC.md）：
#   - 每个 demos/<lang>/run.sh 负责编译（若有）并运行探针，往 stdout 打印
#       CAP   <id> <PASS|FAIL|SKIP> <detail>
#       EXEMPT <id> <reason>            （仅 Tier A：本语言标准库确无此能力）
#       SUMMARY <lang> <pass> <fail> <skip>
#     语言在本机不可用时改为打印一行 `UNAVAILABLE <reason>` 并 exit 2。
#   - 本脚本只通过「环境变量 + 退出码 + stdout 文本」与各语言接触。
#
# 判定（三层，见 SPEC §5）：
#   FAIL            ⇒ 缺陷，退出 1
#   Tier A SKIP/缺测 ⇒ 缺陷，退出 1（A 级不许静默跳过）
#   Tier A EXEMPT   ⇒ 允许，但必须在报告里显式列出理由（"标准库没有"≠"环境坏了"）
#   Tier B 分歧     ⇒ 信息项（各语言标准库覆盖面本就不同），不影响退出码
#
# 可移植性：本脚本会被 check-shell-portability.mjs 扫描，故只用 POSIX 构造，
# 不含 sed -i / sha256sum / stat -c / date -d / readlink -f / mapfile 等。
set -u

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "${HERE}/.." && pwd)"
WORK="${ROOT}/target/demos-work"

# timeout 在 Linux 有、macOS 默认没有 —— 有就用，没有就直跑（不写死 GNU 形式）。
# ⚠️ Windows 自带一个语义完全不同的 timeout.exe：它是"等待 N 秒"的交互式命令，
#    不是 GNU 的超时包装器。GNU 版解析出来不带 .exe 后缀，用它来区分。
TIMEOUT_BIN=""
if command -v timeout >/dev/null 2>&1; then
  TB="$(command -v timeout)"
  case "${TB}" in
    *.exe) ;;                     # Windows 自带的那个，不用
    *)     TIMEOUT_BIN="${TB}" ;;
  esac
fi
LIMIT="${DEMO_TIMEOUT_SECONDS:-180}"

LABEL="${1:-$(uname -srm 2>/dev/null)}"

# 能力项总数（Tier A 19 + Tier B 5）。SPEC §3 是权威来源，这里做一份执行副本：
# 每个 run.sh 必须报满，少一项就是静默丢行。
EXPECTED_CAPS=24

rm -rf "${WORK}"
mkdir -p "${WORK}"

TSV="${WORK}/_results.tsv"
: > "${TSV}"
LANGS_FILE="${WORK}/_langs.txt"
: > "${LANGS_FILE}"
FAILED_FILE="${WORK}/_failed.txt"
: > "${FAILED_FILE}"

echo "=== demos capability matrix — ${LABEL} ==="
echo "root   : ${ROOT}"
echo "workdir: ${WORK}"
echo

# ---------------------------------------------------------------- 逐个语言运行
for d in "${HERE}"/*/ ; do
  [ -d "${d}" ] || continue
  [ -f "${d}run.sh" ] || continue
  lang="$(basename "${d}")"

  lw="${WORK}/${lang}"
  mkdir -p "${lw}"
  out="${lw}/stdout.txt"
  err="${lw}/stderr.txt"

  # CWD 放在 target/demos-work/<lang>，临时文件绝不落进仓库
  if [ -n "${TIMEOUT_BIN}" ]; then
    ( cd "${lw}" && DEMO_LANG_TAG=demos-capability \
        "${TIMEOUT_BIN}" "${LIMIT}" bash "${d}run.sh" < /dev/null ) >"${out}" 2>"${err}"
  else
    ( cd "${lw}" && DEMO_LANG_TAG=demos-capability \
        bash "${d}run.sh" < /dev/null ) >"${out}" 2>"${err}"
  fi
  rc=$?

  # ⚠️ 不要用 grep 解析探针输出。含非 ASCII 的**且不是合法 UTF-8** 的文件会被 grep
  # 判成 "binary file"，它只打印一行 `Binary file X matches` 而**丢掉全部匹配行**；
  # 那一行又会被 awk 当成字段解析，凭空造出一个叫 `file` 的"语言"（实测踩过：
  # 同时吞掉了 java 唯一带中文的那条记录、并伪造出一条 DEFECT）。
  # 单遍 awk 既读得动任意字节，又能顺手做 CRLF 归一。
  meta="$(awk -v L="${lang}" -v TSVOUT="${TSV}" '
    { sub(/\r$/, "") }
    /^CAP / {
      ncap++
      s = ""
      for (i = 4; i <= NF; i++) s = s (i > 4 ? " " : "") $i
      printf "%s\t%s\t%s\t%s\n", $2, L, $3, s >> TSVOUT
      next
    }
    /^EXEMPT / {
      ncap++
      s = ""
      for (i = 3; i <= NF; i++) s = s (i > 3 ? " " : "") $i
      if (s == "") printf "%s\t%s\tFAIL\tEXEMPT without reason\n", $2, L >> TSVOUT
      else         printf "%s\t%s\tEXEMPT\t%s\n", $2, L, s >> TSVOUT
      next
    }
    /^UNAVAILABLE/ { unavail = 1; reason = substr($0, 13); next }
    /^SUMMARY/     { summary = $0; next }
    END {
      printf "%d|%s|%d|%s\n", ncap, summary, unavail, reason
    }
  ' "${out}")"

  n="${meta%%|*}"
  rest="${meta#*|}"
  sum="${rest%%|*}"
  rest="${rest#*|}"
  unavail="${rest%%|*}"
  reason="${rest#*|}"

  if [ "${unavail}" = "1" ] && [ "${rc}" -eq 2 ]; then
    printf 'UNAVAILABLE  %-12s %s\n' "${lang}" "${reason}"
    printf '%s\t%s\t%s\n' "UNAVAILABLE" "${lang}" "${reason}" >> "${TSV}"
    continue
  fi

  if [ "${n:-0}" -eq 0 ]; then
    printf 'NO-OUTPUT    %-12s exit=%s  (see %s)\n' "${lang}" "${rc}" "${err}"
    printf '%s\t%s\t%s\n' "NO-OUTPUT" "${lang}" "exit=${rc}" >> "${TSV}"
    printf '%s\n' "${lang}" >> "${FAILED_FILE}"
    continue
  fi

  printf '%s\n' "${lang}" >> "${LANGS_FILE}"
  printf 'ran          %-12s %s\n' "${lang}" "${sum:-<no SUMMARY>}"

  # 完整性自检：每个 run.sh **必须**报满全部能力项。少报一项就意味着有人静默丢行
  # （正是上面那个 grep binary-heuristic 会造成的后果），必须当场失败而不是让
  # 矩阵少一格、看起来"恰好没测到"。
  if [ "${sum}" = "" ]; then
    printf 'INCOMPLETE   %-12s 没有 SUMMARY 行\n' "${lang}"
    printf '%s\n' "${lang}" >> "${FAILED_FILE}"
  elif [ "${n}" -ne "${EXPECTED_CAPS}" ]; then
    printf 'INCOMPLETE   %-12s 只报出 %s 项（期望 %s）\n' "${lang}" "${n}" "${EXPECTED_CAPS}"
    printf '%s\n' "${lang}" >> "${FAILED_FILE}"
  fi
done

LANGS="$(sort -u "${LANGS_FILE}" | tr '\n' ' ')"
NLANG="$(sort -u "${LANGS_FILE}" | wc -l | tr -d ' ')"

# 语言清单即使 0 个也要让后面的 awk 拿到列宽，所以先算好
if [ "${NLANG}" -eq 0 ]; then
  echo
  echo "!! 没有任何语言跑起来（检查 toolchain 与 run.sh）"
  exit 1
fi

echo
echo "已运行语言（${NLANG}）: ${LANGS}"
echo

# ---------------------------------------------------------------- 渲染矩阵
# 状态码单字符化：18+ 个语言才排得下；完整明细见下方附录。
awk -v langs="${LANGS}" '
BEGIN {
  n = split(langs, L, " ")
  for (i = 1; i <= n; i++) { col[L[i]] = i }
  FS = "\t"
  legend = "P=PASS  S=SKIP  E=EXEMPT  F=FAIL  -=未测"
}
$1 == "UNAVAILABLE" || $1 == "NO-OUTPUT" { next }
{
  id = $1; lg = $2; st = $3
  if (!(id in seen)) { seen[id] = 1; order[++k] = id }
  cell[id, lg] = st
  if (st == "FAIL") failcount++
  else if (st == "PASS") passcount++
  else if (st == "SKIP") skipcount++
  else if (st == "EXEMPT") exemptcount++
}
END {
  printf "%-20s", "capability"
  for (i = 1; i <= n; i++) printf "%2d", i
  printf "  %s\n", "verdict"
  printf "%-20s", "--------------------"
  for (i = 1; i <= n; i++) printf "%2s", "--"
  printf "\n"

  for (j = 1; j <= k; j++) {
    id = order[j]
    tier = substr(id, 1, 1)
    printf "%-20s", id
    np = 0; ns = 0; nf = 0; ne = 0; nm = 0
    for (i = 1; i <= n; i++) {
      st = cell[id, L[i]]
      ch = "?"
      if (st == "") { ch = "-"; nm++ }
      else if (st == "PASS") { ch = "P"; np++ }
      else if (st == "SKIP") { ch = "S"; ns++ }
      else if (st == "EXEMPT") { ch = "E"; ne++ }
      else { ch = "F"; nf++ }
      printf "%2s", ch
    }

    verdict = "ok"
    if (nf > 0) verdict = "DEFECT"
    else if (tier == "A") {
      # Tier A 不允许 SKIP，也不允许某语言没测到（说明 run.sh 少发了一行）
      if (nm > 0 || ns > 0) verdict = "DEFECT"
      else if (ne > 0) verdict = "ok(exempt)"
    } else {
      # Tier B：覆盖面因语言而异是正常的，只在混用时报"分歧"（信息项）
      if (np > 0 && (ns + ne) > 0) verdict = "diverge(B)"
      else if (np == 0 && (ns + ne) > 0) verdict = "all-skip(B)"
    }
    printf "  %s\n", verdict

    if (verdict == "DEFECT") { nbad++; bad[j] = id; badv[j] = verdict }
    else if (verdict == "diverge(B)") { ninfo++; info[j] = id }
    else if (verdict == "ok(exempt)") { nex++; exo[j] = id }
  }

  printf "\n合计: PASS=%d  SKIP=%d  EXEMPT=%d  FAIL=%d     [%s]\n", \
         passcount, skipcount, exemptcount, failcount, legend
  printf "缺陷(DEFECT) = %d    Tier B 信息项 = %d    Tier A 语言豁免 = %d\n", nbad, ninfo, nex

  printf "\n语言编号:\n"
  for (i = 1; i <= n; i++) {
    printf "  %2d=%-12s", i, L[i]
    if (i % 4 == 0) printf "\n"
  }
  if (n % 4 != 0) printf "\n"

  if (nbad > 0) {
    printf "\n!! 缺陷（%d 项，必须修）:\n", nbad
    for (j = 1; j <= k; j++) if (j in bad) printf "   %-20s %s\n", bad[j], badv[j]
  }
  if (nex > 0) {
    printf "\nTier A 语言豁免（%d 项，属标准库差异而非环境缺陷；理由见下方明细）:\n", nex
    for (j = 1; j <= k; j++) if (j in exo) printf "   %s\n", exo[j]
  }
  if (ninfo > 0) {
    printf "\nTier B 分歧（%d 项，信息项，不影响退出码）:\n", ninfo
    for (j = 1; j <= k; j++) if (j in info) printf "   %s\n", info[j]
  }
  if (nbad == 0 && failcount == 0) {
    printf "\n对齐: 无 FAIL，Tier A 无缺陷\n"
  }
  printf "__EXITMARK__%d\n", (nbad > 0 || failcount > 0) ? 1 : 0
}' "${TSV}" > "${WORK}/_matrix.txt"

grep -v '__EXITMARK__' "${WORK}/_matrix.txt"

# ---------------------------------------------------------------- 明细附录
echo
echo "--- 非 PASS 明细（SKIP / EXEMPT / FAIL）---"
awk -F'\t' '$1 != "UNAVAILABLE" && $1 != "NO-OUTPUT" && $3 != "PASS" \
  { printf "  %-20s %-10s %-7s %s\n", $1, $2, $3, $4 }' "${TSV}" | sort -u

if [ -s "${FAILED_FILE}" ]; then
  echo
  echo "--- 未跑通的语言 ---"
  sort -u "${FAILED_FILE}" | sed 's/^/  /'
fi

EXITCODE="$(grep '__EXITMARK__' "${WORK}/_matrix.txt" | head -1 | cut -c13-)"
# 有语言没跑通（NO-OUTPUT / 缺 SUMMARY）也必须是失败退出，否则"少跑一个语言"会被
# 静默当成通过 —— 而 UNAVAILABLE 是**声明式**的（run.sh 明确说本机没有该工具链），
# 所以不算失败。
if [ -s "${FAILED_FILE}" ]; then
  EXITCODE=1
fi
echo
echo "exit=${EXITCODE:-0}"
exit "${EXITCODE:-0}"
