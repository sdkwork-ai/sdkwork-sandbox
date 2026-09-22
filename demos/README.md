# demos —— 跨语言运行环境能力矩阵

用**一份语言无关的能力清单**（24 项）在每个语言里各实现一次探针，再由 runner 把
「能力 × 语言」对齐成一张矩阵，回答两个问题：

> 1. 这台机器 + 这套工具链，到底具备哪些"能跑一个正经应用"的基础能力？
> 2. 不同语言对这些能力的支持是否一致？

> 这里测的是**运行环境与语言标准库的能力**，不是本仓（sdkwork-sandbox）的业务实现。
> `demos/` **不参与** crate 依赖图，不被 `cargo` 构建，也不是任何 `REQ-*` 的实现。

## 快速开始

```sh
bash demos/run-all.sh                    # 自动发现 demos/*/run.sh，逐个跑，渲染矩阵
bash demos/run-all.sh "我的机器"          # 可选：给这次运行一个标签
```

- **退出码 0** = 每个语言都无 `FAIL`，且 Tier A 无缺陷。
- 语言是**自动发现**的：只要存在 `demos/<lang>/run.sh` 就会被跑，不需要改 runner。
- 临时产物一律落在 `target/demos-work/<lang>/`（`target/` 已 gitignore）。

WSL 侧不能把逻辑塞进 `wsl.exe` 的命令行（`$(...)`/`$VAR` 过不了 argv 往返），
用现成的驱动脚本：

```sh
wsl.exe -d <distro> -u root bash /mnt/<drive>/.../sdkwork-sandbox/target/verify-demos-wsl.sh
```

`target/verify-demos-wsl.sh` 负责引导**非登录 shell** 的 PATH（nvm / rust.sh / java / dotnet），
并前置断言工具是否齐全——WSL 的那个坑（`wsl.exe ... bash file.sh` 既不读
`/etc/profile` 也不读 `~/.bashrc`）会让 node/cargo/java 凭空"不存在"。

## 两级能力

| 级别 | 项数 | 判定 |
| --- | --- | --- |
| **Tier A** | 19 | 应用运行下限（argv/env/标准流/文件 IO/目录/时间/随机/容器/错误/子进程/并发/socket）。**必须 `PASS`**；语言标准库确实没有该原语时只能报 `EXEMPT` 并写明理由 |
| **Tier B** | 5 | 常见但非所有标准库都自带（SHA-256/Base64/JSON/正则/gzip）。允许 `SKIP`，但**必须写明是"标准库没有"还是"需要第三方库"** |

详细清单、固定输入与输出协议见 [`SPEC.md`](./SPEC.md)。

`EXEMPT` 与 `SKIP` **不可混用**：Tier A 的"标准库没有"用 `EXEMPT`，
Tier B 的"标准库没有"用 `SKIP`。同一项能力在不同语言里标签不一致，runner 会判成缺陷。

## 语言覆盖（14）

| # | 语言 | 探针 | 入口 | 解释器/编译器发现方式 |
| --- | --- | --- | --- | --- |
| 1 | awk | `probe.awk` | `run.sh` | `gawk`（需要 GNU 扩展：协程管道、`PROCINFO`、`@fn()`） |
| 2 | bash | `probe.sh` | `run.sh` | `command -v bash` |
| 3 | C | `probe.c` | `run.sh` | `cc`/`gcc`（Windows 走 MinGW） |
| 4 | C++ | `probe.cpp` | `run.sh` | `c++`/`g++` |
| 5 | C# | `Probe.cs` + `Demo.csproj` | `run.sh` | `dotnet`（自建临时工程，`--nologo`） |
| 6 | Go | `probe.go` | `run.sh` | `go`；Windows 在 `%ProgramFiles%\Go\bin`，不写进用户 PATH |
| 7 | Java | `Probe.java` | `run.sh` | **从 `javac` 自己的 JDK 根推 `java`**（见下方"踩过的坑"） |
| 8 | Lua | `probe.lua` | `run.sh` | `lua`/`lua5.4`（需 ≥ 5.3：`utf8` 库 + `os.execute` 三返回值） |
| 9 | Node.js | `probe.mjs` | `run.sh` | `node` |
| 10 | Perl | `probe.pl` | `run.sh` | `perl`（Windows 是 StrawberryPerl，装在 `\Strawberry`） |
| 11 | PHP | `probe.php` | `run.sh` | `php`（Windows 走 winget 的用户级 Packages 目录） |
| 12 | Python | `probe.py` | `run.sh` | `python3`/`python` |
| 13 | Ruby | `probe.rb` | `run.sh` | `ruby`（Windows 装在 `\Ruby33-x64`） |
| 14 | Rust | `probe.rs` | `run.sh` | `rustc` 单文件编译（**不用 cargo**，所以不需要网络与 crate） |

## 实测结果

两台宿主各跑一次，都是 **14 个语言全跑通、0 `FAIL`、0 `DEFECT`、退出码 0**：

| 宿主 | PASS | SKIP | EXEMPT | FAIL | 缺陷 | 退出码 |
| --- | --- | --- | --- | --- | --- | --- |
| Windows（Git Bash / MSYS） | 304 | 27 | 5 | 0 | 0 | 0 |
| WSL Ubuntu 22.04 | 305 | 26 | 5 | 0 | 0 | 0 |

完整矩阵与逐条明细落在（`target/` 已 gitignore，属**证据**不是缓存，别清理）：

- `target/demos-matrix.windows.txt`
- `target/demos-matrix.wsl-ubuntu-2204.txt`

### 两台宿主唯一的读数差异

`B04_regex` 的 **C** 一列：Windows 是 `SKIP`、WSL 是 `PASS`（正好差这 1 个 PASS/SKIP）。
原因是 `regex.h` 是 POSIX 而非 ISO C，**MinGW 不提供**，glibc 提供。
这是工具链差异、不是环境缺陷，属 Tier B 允许的信息项。

### Tier A 的 2 项语言标准库豁免

| 能力 | 语言 | 理由 |
| --- | --- | --- |
| `A13_time` | awk | 只有整秒级 `systime()`，无亚秒/单调时钟 |
| `A13_time` | Lua | `os.time` 只到整秒，`os.clock` 是 CPU 时间不是墙钟 |
| `A19_tcp_loopback` | awk | gawk 的 `/inet` 是编译期扩展（本机未编入），POSIX awk 无 socket |
| `A19_tcp_loopback` | bash | `/dev/tcp` 只有 connect（客户端），POSIX shell 无 bind/listen |
| `A19_tcp_loopback` | Lua | 标准库无任何 socket API（须 luasocket 等第三方模块） |

### Tier B 的 5 项分歧（信息项）

`B01_sha256` / `B02_base64` / `B03_json` / `B05_gzip`：C / C++ / Rust / awk / bash / Lua
都报"标准库没有"；Python / Node / Ruby / Perl / PHP / C# / Go / Java 自带。
`B04_regex` 见上。逐条理由都写在矩阵的明细段里。

## 加一个新语言

1. `mkdir demos/<lang>`
2. 写 `probe.<ext>`：实现 `SPEC.md` §3 的 24 项，遵守 §2 的输出协议与 §4 的固定输入。
3. 写 `run.sh`：`chmod +x`。它负责**编译（若有）并运行**，并且：
   - 只用**相对路径**；
   - **把工作目录钉在 `target/demos-work/<lang>`**（runner 会 cd 过去，但直接
     `bash demos/<lang>/run.sh` 时不会——不钉就会把生成物留在 `demos/<lang>/` 里）；
   - 找不到解释器时打印 `UNAVAILABLE <理由>` 并 `exit 2`（runner 记为不可用，不算 FAIL）；
   - 编译失败时**不要**报 UNAVAILABLE，那是探针自己的问题，`exit 1` 并回显日志。
4. `bash demos/run-all.sh` 看这个语言是否全绿、是否与其他语言对齐。

## 踩过的坑（都在对应 `run.sh` / `probe.*` 里留了注释）

这些坑的共同特征是**症状看着像"环境/工具坏了"，实际是路径、编码或版本问题**：

1. **MSYS 路径改写**：原生 Windows 解释器看不懂 `/d/...`，MSYS 还会把它改写成并不
   存在的形式。所以编译型脚本一律用 `cygpath -w` 把输出路径转成 Windows 原生路径。
2. **java/javac 版本错配**：各取 PATH 上的第一个 `java` 和 `javac`，会得到
   "javac 21 编译 / java 11 运行" ⇒ `UnsupportedClassVersionError`，探针**一行 CAP 都发不出来**。
   必须从 `javac` 自己的 JDK 根推 `java`。
3. **在 git 仓库内 `go build`**：Go 会去取 VCS 状态，而 git 在"仓库属主与当前用户不一致"
   时拒绝 ⇒ `error obtaining VCS status: exit status 128`。加 `-buildvcs=false`。
4. **gawk 的 locale**：UTF-8 locale 下 `printf "%c", 200` 会输出 **2 字节** UTF-8 编码，
   于是"写 256 个字节值"变成 384 字节。二进制检查必须整体在 `LC_ALL=C` 下跑。
5. **`print` 补的那个换行**：`print "statted"` 是 **8** 字节不是 7。跨语言对齐时，
   写死长度的检查要用 `printf`/`binwrite` 这类**不补换行**的写法。
6. **Lua 的 `os.execute` 命令不能以引号开头**：它走 MSVC `system()` → `cmd /c`，
   而 cmd 的规则是"整行以 `"` 开头就剥掉首尾两个引号"，`"lua.exe" "probe.lua"` 会被拆坏。
   所以命令行永远带一个环境变量前缀（`set X=Y && ...`）。
7. **`IO#read(n)` 不是"读一次"而是"读满 n 字节或到 EOF"**：用定长小报文测回环会死锁，
   要用 `readpartial`（Ruby）/ `sysread`（Perl、Lua）。
8. **`close()` 不截断文件**：靠 `>>` 追加写的检查必须先删干净，否则上一轮的字节会留下。
9. **WSL 非登录 shell 丢 PATH**：`wsl.exe -d X bash file.sh` 既不读 `/etc/profile`
   也不读 `~/.bashrc`，还会被注入 Windows PATH ⇒ node/cargo/java/dotnet 全部"不存在"。
   另外 **nvm 可能只装在某个普通用户家里**（实测：以 root 跑时 `$HOME` 是 root 的家目录，
   而 nvm 在另一个用户的家目录下），所以要额外扫普通用户的家目录（`/home/*`）。
   ⚠️ 说明文字里**不要**写出「`/home/` + 用户名」形式的完整路径——
   `check-workspace-path-portability` 会把它当机器绝对路径报 `MACHINE-ABS`（实测踩过）。
10. **`check-workspace-path-portability` 会把反斜杠归一后再匹配**：注释里写
    "盘符 + 冒号 + 斜杠"的完整路径字面量也会被报 `MACHINE-ABS`。说明文字别写完整路径。
