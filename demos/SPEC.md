# demos 能力矩阵规范（SPEC）

本目录是一套**语言无关的能力一致性探针**：用同一份能力清单，在每个语言里各实现一次，
再由 runner 把「能力 × 语言」的结果对齐成一张矩阵。

它回答的问题是：**这个运行环境（这台机器 + 这套工具链）到底具备哪些应用运行基础能力，
以及不同语言对这些能力的支持是否一致。**

> 这里测的是**运行环境与语言标准库的能力**，不是本仓（sdkwork-sandbox）的业务实现。
> `demos/` 不参与 crate 依赖图，不被 `cargo` 构建，也不是任何 `REQ-*` 的实现。

## 1. 目录结构

```
demos/
  README.md            总览与用法
  SPEC.md              本文件：能力清单 + 输出协议
  run-all.sh           跨语言 runner（探测语言 → 运行 → 对齐 → 报分歧）
  <lang>/              每个语言一个目录
    run.sh             该语言的入口：负责编译（若有）并运行，输出 CAP 行
    probe.<ext>        探针源码（可多个文件）
```

**约定**：runner 只认 `demos/<lang>/run.sh` 这一个契约。
新增语言 = 新增一个目录 + 一个 `run.sh`，**不需要改 runner**。

## 2. 输出协议（唯一的硬契约）

`run.sh` 必须往 **stdout** 打印，每个检查一行：

```
CAP <id> <PASS|FAIL|SKIP> <detail>
```

- `<id>`：能力编号，见 §3，**大小写敏感**。
- `<detail>`：单行、无换行。PASS 时可写观测到的具体值（推荐，便于审计）；FAIL 必须写原因。
- 最后必须打印一行汇总：

```
SUMMARY <lang> <pass> <fail> <skip>
```

- **退出码**：无 `FAIL` 时 exit 0；有 `FAIL` 时 exit 1。`SKIP` 不算失败。
- 探针**允许**往 stderr 写诊断信息；runner 不解析 stderr（但会捕获，便于排查）。
- 探针**不得**依赖网络（`tcp_loopback` 只打本机回环）。
- 探针**只能**用该语言的标准库，**不得**引入第三方依赖。

### 2.1 Tier A 的语言豁免：`EXEMPT`

Tier A 原则上必须 `PASS`，但有一类情况是**语言标准库本身没有这个原语**（不是"本机没装"、
也不是"暂时跳过"）。这类报 `EXEMPT`，单独一行、格式与 `CAP` 并列：

```
EXEMPT <id> <reason>
```

- `<reason>` **必须非空**；空理由按 `FAIL` 处理（防止把"懒得做"伪装成"语言不支持"）。
- `EXEMPT` **不计入** `pass` / `fail` / `skip`，但 **算作"报满了这一项"** ——
  runner 按 `CAP` 行 + `EXEMPT` 行合计校验 §3 的 24 项，少报仍是 `INCOMPLETE`。
- runner 的判定：某一 Tier A 项"其余语言全 `PASS`、只有个别语言 `EXEMPT`"⇒
  判 `ok(exempt)`，**不算缺陷**；但若掺进 `FAIL` 或缺测，仍是 `DEFECT`。
- **`SKIP` 与 `EXEMPT` 不可混用**：Tier A 的"标准库没有"一律用 `EXEMPT`，
  Tier B 的"标准库没有"一律用 `SKIP`。同一项能力在不同语言里必须用同一种标签，
  否则 runner 会判成 Tier A 缺陷（实测踩过：awk 报 `SKIP`、lua 报 `EXEMPT` 被判 `DEFECT`）。

## 3. 能力清单

### Tier A — 基础能力（所有可用语言**必须 PASS**）

Tier A 是"能跑一个正经应用"的下限：命令行、环境、标准流、文件 IO、目录、时间、随机、
容器、错误处理、子进程、并发、本机 socket。**任何 Tier A 的 FAIL 都是缺陷**；
某语言标准库**确实没有**该原语时，只能报 `EXEMPT` 并写明理由（见 §2.1），不得静默 `SKIP`。

| id | 能力 | 判定要点 |
| --- | --- | --- |
| `A01_argv` | 命令行参数 | 能读到 argv 的元素个数与内容 |
| `A02_env` | 环境变量 | 能读到 runner 注入的环境变量并比对 |
| `A03_streams` | stdout + stderr | stdout 与 stderr 都能写，且内容不串 |
| `A04_stdin` | 标准输入 | 能从 stdin 读入一行并回应 |
| `A05_file_rw` | 文本文件读写 | 写入 → 读回 → 内容逐字节相同 |
| `A06_file_append` | 追加写 | 追加后总内容 = 前内容 + 追加内容 |
| `A07_file_binary` | 二进制 IO | 256 字节含 `0x00`／`0xFF` 往返一致 |
| `A08_file_stat` | 文件元信息 | 存在性、字节长度、删除 |
| `A09_dir_ops` | 目录操作 | 建目录 → 列目录（含新建的文件）→ 删除 |
| `A10_temp_file` | 临时文件 | 能创建唯一临时文件并读回 |
| `A11_unicode` | UTF-8 多字节 | 中日韩 + emoji 往返，**字符数**与**字节数**都对 |
| `A12_large_io` | 大块 IO | 256 KiB 写入并读回，长度一致 |
| `A13_time` | 时间与单调时钟 | epoch 时间合理（> 2020-01-01）；睡眠 50 ms 后单调差值 ≥ 40 ms |
| `A14_random` | 随机数 | 生成的数落在指定区间内 |
| `A15_container` | 容器与排序 | 整数数组排序正确；map/dict 增删查正确 |
| `A16_error` | 错误处理 | 能捕获一次预期内的错误（异常 / `Result` / 错误码） |
| `A17_subprocess` | 子进程 | 启动子进程、捕获其 stdout、拿到退出码 |
| `A18_concurrency` | 并发 | 起 2 个并发执行体并 join，累加结果正确 |
| `A19_tcp_loopback` | 本机 socket | 回环 bind → connect → 收发一致 |

### Tier B — 扩展能力（可 `SKIP`，但**必须写明理由**）

Tier B 需要的能力并非所有标准库都自带（例如 C 没有 JSON / SHA-256 / Base64）。
这些项允许 `SKIP`，但 `<detail>` 里必须写清是"标准库没有"还是"需要第三方库"。
**不允许**静默省略——`SKIP` 就是显式声明。

| id | 能力 | 典型标准库来源 |
| --- | --- | --- |
| `B01_sha256` | SHA-256 | `hashlib` / `crypto` / `MessageDigest` / `sha2` |
| `B02_base64` | Base64 编解码 | `base64` / `Buffers` / `Base64` |
| `B03_json` | JSON 序列化 + 解析 | `json` / `JSON` / `serde_json` |
| `B04_regex` | 正则匹配 | `re` / JS RegExp / `regex` |
| `B05_gzip` | gzip 压缩解压 | `gzip` / `zlib` / `java.util.zip` |

## 4. 与语言无关的固定输入

为了能跨语言比对，输入是**约定死的**，不由各语言自行决定：

| 约定 | 值 |
| --- | --- |
| 环境变量 | `DEMO_LANG_TAG=demos-capability`（runner 注入，A02 比对它） |
| `A01_argv` 的注入参数 | 探针**自我调用**时传 `--argv-probe alpha beta`，须回显参数个数与内容 |
| `A04_stdin` 输入 | 探针**自我调用**模式 `--read-stdin`，父侧向其 stdin 写 `ping`，子侧须回 `pong` |
| `A11_unicode` 测试串 | `中文-日本語-한국어-🚀`（**12 个码点** / 13 个 UTF-16 单元 / **31 字节** UTF-8） |
| `A12_large_io` 大小 | `262144` 字节（256 KiB） |
| `A13_time` 睡眠 | `50` ms |
| `A15_container` 排序输入 | `[5, 3, 9, 1, 7, 3]` → 排序后 `1,3,3,5,7,9` |
| `A17_subprocess` | 子进程 = 探针**自身**以 `--echo-child` 启动，只打印 `child-ok` 并 exit 0 |
| `B01_sha256` 输入 | 字符串 `abc` → `ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad` |

> **设计要点：所有涉及"另一个进程"的检查都靠探针自我调用完成，runner 不注入参数、不喂 stdin。**
> 这样 runner 与各语言之间只有"环境变量 + 退出码 + stdout 文本"三个接触面，少一整类时序/协调缺陷。
> 探针因此需要四种模式：默认（跑全部检查）、`--echo-child`、`--read-stdin`、`--argv-probe …`。
> 模式名必须照抄上表，否则跨语言无法互认。

## 5. 判定与对齐

runner 的判定逻辑：

1. **每个语言自身**：`FAIL` 计数必须为 0（若 `run.sh` 缺失或语言不可用 ⇒ 记 `UNAVAILABLE` + 原因，不算 FAIL）。
   每个语言必须报满 §3 的 24 项（`CAP` + `EXEMPT` 合计），少报 ⇒ `INCOMPLETE` 并按失败处理。
2. **跨语言对齐**：对每个 `cap` id，收集所有"已运行语言"的状态。
   - 全部 `PASS` ⇒ 该能力**对齐**。
   - 出现 `PASS` 与 `SKIP`（或 `EXEMPT`）混杂 ⇒ **分歧**（有语言做到了，有语言说没有 ⇒ 需要复核理由）。
   - 出现 `FAIL` ⇒ **分歧 + 缺陷**。
3. **Tier A 不容忍 `SKIP`**：Tier A 的每项要么 `PASS`，要么 `EXEMPT`（理由非空）。
   "其余语言全 `PASS`、个别语言 `EXEMPT`" ⇒ `ok(exempt)`，是**标准库差异**，不算缺陷；
   但一旦掺进 `FAIL` 或某语言缺测，就是 `DEFECT`。
   Tier B 的分歧必须在 §3 里能被"标准库差异"解释（属信息项，不影响退出码）。

## 6. 加一个新语言要做什么

1. `mkdir demos/<lang> && cd demos/<lang>`
2. 写 `probe.<ext>`，实现 §3 的检查，遵守 §2 的输出协议与 §4 的固定输入。
3. 写 `run.sh`：`chmod +x`，负责编译（若需要）并运行；**只用相对路径**，把工作目录放在
   `target/demos-work/<lang>`（`target/` 已被 gitignore，避免把临时文件写进仓库）。
4. 跑 `bash demos/run-all.sh` 看这个语言是否全绿、是否与其他语言对齐。

## 7. 硬约束（来自本仓的门禁）

`demos/` 里新增的文件会被本仓的门禁扫到，所以：

- **不得写死绝对路径**（`check-workspace-path-portability.mjs`）。
- **`.sh` 一律 LF 且不得用 GNU-only 或 bash4+ 专有语法**（`check-shell-portability.mjs`）：
  禁用 `sed -i`、`sha256sum`、`stat -c`、`date -d`、`timeout N`、`readlink -f`、`mapfile`、
  `${var,,}`、`|&`、`;;&` 等；确有需要时在同语句上方加 `PORTABILITY:allow` 注释。
- 临时产物一律落在 `target/demos-work/`，**不要把生成物提交进仓库**。
