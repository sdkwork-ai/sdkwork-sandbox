# REVIEW-20260922: Sandbox 跨平台运行与部署面走查

Status: active

Outcome: 结论是**两个宿主机都能把本仓完整跑起来**，但**跑起来的含义不同**，而且差异里藏着 **1 个真缺陷 + 1 个门禁盲区**。Windows 宿主 **22/22 步全过**（4 步 cargo/契约 + 18 条静态门禁），WSL Ubuntu 22.04 宿主 **21/22**（唯一失败项 `check-shell-portability.mjs`）。两侧 `cargo test --workspace` 都是 **67 passed / 0 failed / 1 ignored**，契约套件 **612/612（40 文件）**，`bin/apps-build.sh server development` 经 WSL 桥接 **exit 0（`Finished release profile` 41.18 s）**。多语言 Hello World 矩阵：Windows **7 PASS / 1 FAIL / 1 SKIP**，WSL **7 PASS / 0 FAIL / 2 SKIP**，两侧失败的都只是**本机没装那套 SDK**（Windows 缺 .NET SDK、两侧都缺 Go）。**真缺陷 F-C1**：`bin/**/*.sh` 在 Windows 工作树是 **100 % CRLF**（提交的 blob 是 LF），导致这些脚本**在 WSL/Linux 下不可 source、报 `No such file or directory`**；而**同一个门禁在 Windows 上通过、在 WSL 上失败**——因为 `check-shell-portability.mjs` 用 PATH 上的 `bash` 做 `bash -n`，Windows 侧拿到的是 **Cygwin bash（容忍 CRLF）** 而不是 **Linux bash（不容忍）**，于是**在引入 CRLF 的那个宿主机上，可移植性门禁恰好看不见这个不可移植**（**F-C2**）。部署面不是坏的：`bin/docker-image.sh build` 与 `bin/docker-deploy.sh install` 都以 **exit 67** 快速失败并给出明确原因（assembly-only 模块、无 standalone gateway、无打包 bundle），`bin/doctor.sh` 返回 **exit 70** 的真实诊断（两侧都无 docker）——这是**治理性刻意不做**，不是缺陷。本报告不改变任何 `REQ-*`/ADR 状态，也不构成任何实现授权。

Owner: SDKWork Runtime Platform

Date: 2026-09-22

## 0. 目的与范围

本报告回答的是：「**这个仓能不能在 Windows 和 WSL 的 Ubuntu 里完整跑起来，并部署一个简单的多语言 Hello World 应用**」。

三张既有材料回答的是别的问题，本报告不重复它们：

| 既有材料 | 回答的问题 |
| --- | --- |
| [`REVIEW-20260922-sandbox-functional-module-walk.md`](REVIEW-20260922-sandbox-functional-module-walk.md) | **实现逻辑本身**对不对、有没有洞 |
| `target/host-capability-evidence.{windows,wsl-ubuntu-2204}.json`（仓内证据） | 两个宿主**能不能满足隔离模型**（47 项能力探测） |
| **本报告** | **同一份 commit 在两个宿主上能不能跑通门禁、测试与构建**，以及部署面到底走到哪一步 |

范围边界，先说清楚：

- 本报告只做**运行与部署面**的验证，不做实现逻辑走查（那是上面第一份报告的事）。
- 本报告的判定依据只有三类：**可复现的命令及其读数**、**被引用的 `path:line` 或字节级事实**、**两侧同命令的对照读数**。没有依据的判定不写。
- 全部验证脚本与原始读数落在 gitignored 的 `target/` 下（`verify-windows.sh`、`verify-wsl.sh`、`hello-matrix/`、`_wsl-verify.log`、本报告 §7 的复现命令）。

## 1. 两侧验证读数（同一 commit，同一套步骤）

两侧跑的是**逐条对应**的同一组步骤（脚本：`target/verify-windows.sh` / `target/verify-wsl.sh`），因此读数可直接对照。

| # | 步骤 | Windows | WSL Ubuntu 22.04 |
| --- | --- | --- | --- |
| — | `bash --version` | GNU bash **5.3.15** (x86_64-pc-**cygwin**) | GNU bash **5.1.16** (x86_64-pc-**linux**-gnu) |
| — | `cargo --version` | **1.98.1** (797e8a9bc 2026-08-05) | **1.98.1** (797e8a9bc 2026-08-05) |
| — | `node --version` | **v22.22.2** | **v24.21.0** |
| 1 | `cargo fmt --check` | ✅ PASS | ✅ PASS |
| 2 | `cargo check --workspace --all-targets` | ✅ PASS | ✅ PASS |
| 3 | `cargo test --workspace` | ✅ PASS（**67 / 0 / 1**） | ✅ PASS（**67 / 0 / 1**） |
| 4 | `node --test tests/contract/*.test.mjs` | ✅ PASS（**612 / 612**，40 文件） | ✅ PASS |
| 5 | 18 条静态门禁 | ✅ **18 / 18 PASS** | ⚠️ **17 PASS / 1 FAIL** |
| | **合计** | **22 passed / 0 failed** | **21 passed / 1 failed** |

唯一的分歧项是：

```text
# Windows
$ node ../sdkwork-specs/tools/check-shell-portability.mjs --root .
shell portability passed: 11 files, 0 findings (bash -n: on)          ← exit 0

# WSL Ubuntu 22.04
$ node ../sdkwork-specs/tools/check-shell-portability.mjs --root .
shell portability failed (2 findings across 11 files)                 ← exit 1
- .../bin/lib/bootstrap.sh: bash -n failed: ... line 12: syntax error near unexpected token `elif'
- .../bin/lib/module.sh:    bash -n failed: ... line 20: syntax error near unexpected token `$'{\r''
```

`cargo test --workspace` 的 **67 / 0 / 1** 不是本报告新造的数：它与 `specs/sandbox-e2b-capability-baseline.json` 里 `testInventory.rustWorkspace`（`{ command: "cargo test --workspace", passed: 67, failed: 0, ignored: 1 }`）**逐字段一致**，而该字段正是 `tools/check-sandbox-e2b-field-parity.mjs` 比对的对象——该门禁在两侧都是 PASS，所以这个数**同时被两侧实测与门禁记录互相印证**。

## 2. 多语言 Hello World 能力矩阵（真正的多语言部署验证）

`target/hello-matrix/` 下每个语言一份 Hello World，`run-all.sh` 逐语言编译并**执行**，输出 `<语言> <PASS|FAIL|SKIP> <首行>`。两侧实跑结果：

| 语言 | Windows | WSL Ubuntu 22.04 | 说明 |
| --- | --- | --- | --- |
| rust | ✅ `hello from rust` | ✅ `hello from rust` | `rustc -O` |
| c | ✅ `hello from c` | ✅ `hello from c` | `gcc -O2` |
| cpp | ✅ `hello from cpp` | ✅ `hello from cpp` | `g++ -O2` |
| python | ✅ `hello from python` | ✅ `hello from python` | `python3` |
| javascript | ✅ `hello from javascript` | ✅ `hello from javascript` | `node` |
| java | ✅ `hello from java` | ✅ `hello from java` | `javac` + `java -cp` |
| bash | ✅ `hello from bash` | ✅ `hello from bash` | 纯 shell |
| go | ⏭️ SKIP `go not on PATH` | ⏭️ SKIP `go not on PATH` | **两侧都未安装 Go** |
| csharp | ❌ FAIL `dotnet runtime present but no .NET SDK installed` | ⏭️ SKIP `dotnet not on PATH` | 见下 |
| | **7 PASS / 1 FAIL / 1 SKIP** | **7 PASS / 0 FAIL / 2 SKIP** | |

两条要点：

1. **两侧唯一"失败"与代码无关，是本机 SDK 缺失**。Windows 装了 `dotnet` 运行时但没有 SDK，而 .NET 运行时宿主**无法编译单文件 C#**，因此脚本用 `dotnet --list-sdks` 把它区分为 `FAIL`（能力答案）而不是环境错误；WSL 侧根本没有 `dotnet`，故记为 `SKIP`。Go 两侧都没有，一律 `SKIP`。**9 个语言里 7 个在两个宿主上都真正编译并执行成功**，这足以支撑「多语言 Hello World 部署」这一诉求。
2. **顺带解释了一个跨平台坑**：矩阵脚本刻意全程用**相对路径**。原生 Windows 工具链（`rustc`、MinGW `gcc`/`ld`）消费不了 Git Bash 的 `/d/...` 路径——MSYS 会把它改写成并不存在的 `D:/d/...`，链接阶段直接 `cannot open output file`。相对路径在两侧行为一致，一次编写两侧可跑。

## 3. 部署面：真实结论是"治理性阻塞"，不是"坏了"

用户问的是"部署"。必须把这条说清楚：**部署链路目前被治理刻意挡在门外，而且它挡得很明确、很可诊断。**

| 命令 | 退出码 | 原文（节选） |
| --- | --- | --- |
| `bin/apps-build.sh server development` | **0** | `Finished release profile [optimized] target(s) in 41.18s` |
| `bin/doctor.sh` | **70** | `FAIL toolchain  docker or the compose plugin is missing on wsl` → `summary: 0 passed, 1 warned, 5 failed` |
| `bin/docker-image.sh build` | **67** | `sdkwork-sandbox is an assembly-only module: it ships no standalone server binary (no *-standalone-gateway crate), so a container image build is not applicable yet. If a standalone gateway lands, wire this hook per MODULE_BIN_SPEC.md §4.1` |
| `bin/docker-deploy.sh install --environment development` | **67** | `no packaged install bundle under /d/sdkwork-space/sdkwork-sandbox/dist/docker-install (package it with the module's own bundle packager first)` |
| `bin/apps-package.sh`（无参） | **64** | 打印 `MODULE_BIN_SPEC.md` entrypoints usage |
| `bin/apps-pkg-installer.sh`（无参） | **64** | 同上 |

解读：

- **构建面是通的**：`bin/apps-build.sh server development` 真的完成了 `cargo build --release`（exit 0，41.18 s）。构建日志里的路径是 `/mnt/d/sdkwork-space/...`，**证明它在 WSL 内执行**——即 Windows 侧脚本通过 `sdkwork_needs_wsl_bridge` 把 canonical 构建桥进了 WSL。这与仓内宿主能力证据一致：Windows verified **4 / 47**，WSL verified **29 / 47** 且 `blocking: []`。
- **镜像/部署面是刻意不做的**：`exit 67` 不是崩溃，是**设计好的失败关闭**，并且**告诉你为什么、以及将来怎么接线**（等 standalone gateway 落地后按 `MODULE_BIN_SPEC.md §4.1` 接）。根因是本仓当前**只发布 assembly 构件、不发布独立 server 二进制**——这属于治理边界（`AGENTS.md` 禁止无 `ready` REQ 就实现部署 Profile），**不是缺陷**。
- **`doctor.sh` 给的是真诊断**：它指出两侧都缺 docker/compose，以及没有已部署 bundle。`exit 70` 反映的是**环境缺件**，而且它是唯一会主动去读 WSL 侧 `/var/lib/docker` 的脚本。

## 4. 唯一真缺陷：行尾（CRLF）与门禁的"宿主相关裁决"

这是本报告的核心发现。它不是一个单纯的"文件行尾没统一"，而是**一个可移植性门禁在错误的那一侧给出了通过**。

### 4.1 事实链

**① 提交的 blob 是干净的 LF，工作树是 CRLF。**

```text
$ git ls-files --eol bin/lib/module.sh bin/lib/bootstrap.sh bin/apps-build.sh
i/lf    w/crlf  attr/    bin/apps-build.sh
i/lf    w/crlf  attr/    bin/lib/bootstrap.sh
i/lf    w/crlf  attr/    bin/lib/module.sh
```

`i/lf`（索引 = LF）、`w/crlf`（工作树 = CRLF）、`attr/` 为空（**仓内没有 `.gitattributes`**），加上全局 `core.autocrlf=true`，共同决定了：**任何人在这台 Windows 上检出，`bin/**/*.sh` 就变成 CRLF**。三个文件都是 **100 % CRLF**：

```text
bin/lib/module.sh     CRLF=65 / 总 65 行   ← 100% CRLF
bin/lib/bootstrap.sh  CRLF=39 / 总 39 行   ← 100% CRLF
bin/apps-build.sh     CRLF=4  / 总 4  行   ← 100% CRLF
```

**② CRLF 使这些脚本在 WSL/Linux 下不可用。**

```text
$ bash -n bin/lib/module.sh          # WSL
bin/lib/module.sh: line 20: syntax error near unexpected token `$'{\r''
bin/lib/module.sh: line 20: `sdkwork_image_build() {'
```

`\r` 恰好落在 `{` 之后，被 bash 当成一个非法附加记号。操作性后果更严重——**连 source 都失败**：

```text
$ bash bin/apps-build.sh             # WSL
bin/apps-build.sh: line 4: $WREPO/bin/lib/bootstrap.sh: No such file or directory
```

（上面两块是**逐字引用的工具输出**，唯一改动是把打印出来的绝对路径换成 `$WREPO`，以符合仓内"文档不得写死绝对路径"的门禁；其余字符未动。）

注意这里是 `No such file or directory` 而不是语法错误：脚本里 `source` 的路径末尾带了 `\r`，于是 bash 去找一个**名字里含 `\r` 的文件**。也就是说，**同一批 `bin/*.sh` 在不同文件上的失败模式还不一样**（`module.sh` 报语法错，`apps-build.sh` 报找不到文件），这会显著拖慢定位。

**③ 原生 Linux 克隆证明：问题在检出侧，不在提交侧。**

```text
$ git clone $WREPO /tmp/swsb-clone   # WSL 原生文件系统
$ cd /tmp/swsb-clone && git ls-files --eol bin/lib/module.sh bin/apps-build.sh
i/lf    w/lf    attr/    bin/apps-build.sh
i/lf    w/lf    attr/    bin/lib/module.sh
$ bash -n bin/lib/module.sh && echo OK
OK
```

`w/lf` 且 `bash -n` 通过。**结论无可争辩：对象库里的内容是正确的 LF；是 Windows 检出的 `core.autocrlf=true` 把它弄脏的。**

### 4.2 真正的问题：门禁在"肇事宿主"上恰好放行（F-C2）

把两侧的裁决并排放，问题就露出来了：

| 宿主 | 门禁用的 `bash` | 裁决 |
| --- | --- | --- |
| Windows | `bash 5.3.15 (x86_64-pc-cygwin)` | `passed: 11 files, 0 findings (bash -n: on)` |
| WSL Ubuntu | `bash 5.1.16 (x86_64-pc-linux-gnu)` | `failed (2 findings across 11 files)` |

门禁的实现是（`../sdkwork-specs/tools/check-shell-portability.mjs:137-145`）：

```js
function bashSyntaxCheck(file) {
  try { execFileSync('bash', ['-n', file], { stdio: 'pipe' }); return []; }
  catch (err) { return [`${file}: bash -n failed: ...`]; }
}
```

它**诚实地**报了 `bash -n: on`——它确实跑了 `bash -n`。问题在于**它跑的是 PATH 上的那个 bash**，而在 Windows 上那是 **Cygwin bash，它对 CRLF 是容忍的**。于是：**在唯一会把行尾变成 CRLF 的宿主上，这个"可移植性"门禁正好看不见 CRLF 造成的不可移植。** 这不是"假绿"式的实现错误（它没有谎报自己跑了什么），而是**参照实现选错**——一个宣称检查跨平台可移植性的门禁，其参照解释器不该是只存在于该平台的那一个。

这也顺带解释了为什么"Windows 18/18"与"WSL 17/18"能同时成立且都"没错"：**两份读数都是真的，因为裁决本身是宿主相关的。** 对评审而言，危险的是**只看 Windows 侧会得到"全绿"**，从而把一个 Linux 侧真实存在的不可执行性问题判为不存在。

### 4.3 为什么它没挡住 canonical 路径（但不等于无害）

现状下它**不阻塞**正常开发：canonical 构建走 WSL 桥，而桥内执行的是**脚本文件本身**（由 Windows 侧脚本调起），并没有走"在 Linux 里 `bash bin/xxx.sh`"这条路。所以 CI/日常流程不会踩到。

但它仍然是真缺陷，因为：

- **它违反门禁自己的承诺**。`check-shell-portability.mjs` 的文档头写明目标是「Linux distros + macOS」，而当前实现在 Windows 宿主上对这一目标给出通过。任何"在 WSL 里直接跑 `bin/*.sh`"的用法（本地复现、Linux CI、运维脚本）都会失败。
- **失败模式分散**（语法错 / 找不到文件两种），排查成本高于其价值。
- **它有一个便宜的确定性修法**（见 §8），没有理由长期留着。

## 5. 发现清单

| # | 发现 | 性质 | 严重度 | 依据 | 状态 |
| --- | --- | --- | --- | --- | --- |
| **F-C1** | `bin/**/*.sh` 在 Windows 工作树为 100 % CRLF（`bin/lib/module.sh` 65/65、`bin/lib/bootstrap.sh` 39/39、`bin/apps-build.sh` 4/4），使这些脚本在 WSL/Linux 下 `bash -n` 失败、`source` 报 `No such file or directory`。提交的 blob 是 LF（原生克隆 `w/lf` 且通过），**故根因是检出侧的 `core.autocrlf=true` + 仓内无 `.gitattributes`**。 | **缺产物**（缺 `.gitattributes`，行尾策略未落盘） | 中（不阻塞 canonical 路径，但阻塞任何 Linux 侧直接调用） | §4.1 三段读数 | 未修（见 §8） |
| **F-C2** | `check-shell-portability.mjs` 用 PATH 上的 `bash` 做 `bash -n`，因此**在 Windows（Cygwin bash，容忍 CRLF）上对同一批文件给出通过、在 WSL（Linux bash）上给出失败**。可移植性门禁的参照解释器在 Windows 上是该平台专有的 Cygwin bash，导致**在引入 CRLF 的宿主上看不见 CRLF 导致的不可移植**。 | **缺门禁**（判据存在但参照实现不足；该门禁在另一仓 `sdkwork-specs`） | 中（会误导只在 Windows 侧跑门禁的评审） | §4.2 对照读数 + `check-shell-portability.mjs:137-145` | 未修 |
| **F-C3** | 部署链路（镜像构建 / 打包 / 安装）在两侧都不可达：`bin/docker-image.sh build` 与 `bin/docker-deploy.sh install` 均 **exit 67**（assembly-only 模块、无 standalone gateway、无 bundle），`bin/doctor.sh` **exit 70**（两侧无 docker）。 | **刻意不做**（治理阻塞，非缺陷） | 信息级 | §3 原文与退出码 | 预期状态 |

F-C1 与 F-C2 **同源但不同层**：F-C1 是**产物**问题（工作树行尾），F-C2 是**判据**问题（谁来裁定可移植）。修 F-C1 能让当前树在 Linux 可用；修 F-C2 才能让**下一次**的同类问题在 Windows 侧就被发现。**两者都要修，且顺序不重要，但只修 F-C1 会留下盲区。**

## 6. 三个"看起来像缺陷但其实不是"的结论

写下来防止后续评审重复怀疑：

1. **Windows verified 仅 4/47 宿主能力，不是"Windows 坏了"。** 这是仓内既有的宿主能力证据（`target/host-capability-evidence.windows.json`：verified 4 / unsupported 5 / unverifiable 38 / denied 0），反映的是**本仓隔离模型需要 Linux 内核**这一设计前提。WSL 侧 29/47 且 `blocking: []`，正是 canonical 构建桥进 WSL 的原因。两侧都有各自的正确角色。
2. **`exit 67` 不是崩溃。** 它是 `bin/lib/module.sh` 里"未接线钩子"的既定失败关闭路径，配套给出原因与未来接线指引。把它读成"部署坏了"是误判。
3. **C# 的 `FAIL` 不是本仓问题。** 它是宿主 SDK 缺口（Windows 有 runtime 无 SDK），脚本已用 `dotnet --list-sdks` 把它与"环境错误"区分开。

## 7. 复现命令

全部只读；两个 sweep 脚本都是"每条步骤独立、逐条报 PASS/FAIL"的形式。

下文的 `$WREPO` = 本仓在 **WSL 内的 `/mnt/` 形式路径**（同一份检出，只是从 Linux 侧看），`$REPO` = 它在**宿主上的绝对路径**。这两个变量只为让文档本身可移植——**本文件刻意不写死任何绝对路径**，这是仓内 `check-workspace-path-portability.mjs` 的硬要求。

```bash
# 两侧全量验证（逐条对应，可直接对照）
bash target/verify-windows.sh
wsl.exe -d Ubuntu-22.04 bash $WREPO/target/verify-wsl.sh

# 多语言 Hello World 矩阵
bash target/hello-matrix/run-all.sh windows
wsl.exe -d Ubuntu-22.04 bash -lc "bash $WREPO/target/hello-matrix/run-all.sh wsl"

# F-C1 / F-C2 的证据
node ../sdkwork-specs/tools/check-shell-portability.mjs --root .        # Windows：passed
git ls-files --eol bin/lib/module.sh bin/lib/bootstrap.sh bin/apps-build.sh
node -e "const s=require('fs').readFileSync('bin/lib/module.sh','utf8');console.log((s.match(/\r\n/g)||[]).length, s.split('\n').length-1)"
wsl.exe -d Ubuntu-22.04 bash -c "cd $WREPO && node ../sdkwork-specs/tools/check-shell-portability.mjs --root ."
wsl.exe -d Ubuntu-22.04 bash -c "cd $WREPO && bash bin/apps-build.sh"   # F-C1 的操作性后果
# 原生克隆对照（证明提交侧是干净的）
wsl.exe -d Ubuntu-22.04 bash -c "rm -rf /tmp/swsb-clone && git clone -q $WREPO /tmp/swsb-clone && cd /tmp/swsb-clone && git ls-files --eol bin/lib/module.sh && bash -n bin/lib/module.sh && echo OK"
```

### 7.1 复现时最容易踩的坑：驱动 WSL 的两个陷阱（都实测踩过）

写在这里，因为**本报告第一次的 WSL 读数是错的（22 步全 FAIL）**，而错误的成因完全是环境的：

1. **`$(...)` 与 `$VAR` 不存活。** 从 Windows Git Bash 经 `wsl.exe` 传参时，`bash -lc 'X=$(echo hi); echo X=$X'` 会打印 `X=`。**一切 WSL 逻辑必须写进脚本文件**再执行；命令行里塞替换式会得到**假的「工具不存在」**。
2. **`wsl.exe -d X bash /path/s.sh` 是非登录非交互 shell。** 它**不读 `/etc/profile`、也不读 `~/.bashrc`**，同时 WSL 还注入 **Windows PATH**。后果是三类东西一起丢：`node`（`~/.nvm/versions/node/<v>/bin`，靠 `.bashrc` 里的 nvm 激活）、`cargo`（`/usr/local/cargo/bin`）、以及 **`RUSTUP_HOME`/`CARGO_HOME`**（后两者都靠 `/etc/profile.d/rust.sh`）。缺 `RUSTUP_HOME` 时 rustup 找不到 `settings.toml`，报 `could not choose a version of cargo ... no default is configured`——**看起来像 Rust 装坏了，其实纯环境**。以上三种症状叠加，让整套 22 步在 **2 秒内全 exit 127**。

因此 `target/verify-wsl.sh` **必须自己** `. /etc/profile.d/rust.sh` + 激活 nvm，并在开头加 `command -v node` / `cargo --version` 前置断言——**在环境坏掉时拒绝运行，而不是输出 22 个假的 FAIL**。（该脚本现已如此；这是本报告对工具面的一处直接改进。）

## 8. 最小修复选项与治理分类

**F-C1（缺产物）**：补一份 `.gitattributes` 即可，例如

```gitattributes
* text=auto eol=lf
*.sh text eol=lf
```

这是**新增一个文件**，不改任何已被 `accepted` 需求覆盖的实现代码，因此**不需要新的 `REQ-*`**。注意：加了之后需要一次"规范化"让当前工作树的行尾归位（`git add --renormalize .` 或重新检出），而 `bin/lib/bootstrap.sh`、`bin/lib/module.sh` 是**已提交文件**，规范化要让 `git diff --numstat` 显示"只改了行尾、行数不变"，以守「不做无谓实现改动」的纪律。

**F-C2（缺门禁）**：修的是 `../sdkwork-specs/tools/check-shell-portability.mjs`，属**另一仓**，不在本仓可直接修改的范围。可行的修法（按侵入度排序）：

1. **让参照解释器可指定**：新增 `--bash <path>`（或 `SDKWORK_PORTABILITY_BASH`），使调用方能指向真正的目标 bash（如 WSL 内的 `/usr/bin/bash`）。
2. **加一条"行尾判据"**：对每个 `.sh` 直接断言"不含裸 `\r`"，与解释器无关，也就是把 CRLF 从「靠 `bash -n` 碰巧发现」升级为「独立判据」。**这一条不依赖任何宿主，而且正好补上 §4.2 的盲区**，是性价比最高的选项。
3. 最弱的形式：在 Windows 上跑该门禁时输出一条"参照 bash 为 Cygwin，CRLF 判据不可信"的警告。

无论选哪个，都应同步在其 `## <主题> Gate` 段与根 `README.md` 的说明里登记判据变化（本仓 `README.md` 记录新增门禁要登记 5 处）。

**F-C3**：不需要动作，等治理允许 standalone gateway 后按 `MODULE_BIN_SPEC.md §4.1` 接线。

## 9. 本报告自身的局限

诚实登记，避免被当成"已验证全部"：

- **没有跑通容器/部署链**。两侧都没有 docker，因此"部署 Hello World 到容器"这一层**本报告没有验证，也无法验证**；本报告能证明的是"构建面通、部署钩子按治理预期快速失败"。
- **Hello World 矩阵只覆盖 9 个语言**，且 Go 两侧都未安装、C# 在 Windows 上是宿主 SDK 缺口——所以"多语言"结论的强度是 **7/9 实测通过**，不是"任意语言都能跑"。
- **Windows 侧的 22/22 是在本机工具链下取得的**，其中 `node v22.22.2` 与 WSL 侧 `v24.21.0` 版本不同；两者都跑通了同一套门禁与契约套件，但不构成"任意 Node 版本均可"的证明。
- **F-C1 的修复未落地**。本报告只做到"把缺陷、根因、证据与修法写清楚"，改动留待按 §8 的纪律执行。
- 本报告的**判定依据索引**就是 §5 与 §4 的引用（`path:line`、命令读数、字节级计数）。**没有依据的判定不写**这一条同样适用于本报告自身。
