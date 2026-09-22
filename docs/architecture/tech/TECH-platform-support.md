# Sandbox 平台支持与宿主能力

本文回答一个问题：**sdkwork-sandbox 的控制面与执行环境能在哪些平台上跑，证据是什么，哪些平台只是没测过。**
性能数字见 [TECH-performance-baseline.md](TECH-performance-baseline.md)；性能**目标**见 [TECH-performance-and-capacity.md](TECH-performance-and-capacity.md)。

本文的表格**结构与引证**由 `tools/check-sandbox-platform-code.mjs` 逐行校验（平台词表覆盖、
状态取值、每行的仓内路径或能力 id 可解析、平台条件代码声明与源码双向一致、§4.1 点名的门禁已接线）。
**单元格里的实测数字不由门禁校验真值**——它们来自 §2 的两条探测命令与
[TECH-performance-baseline.md](TECH-performance-baseline.md) 的原始产物，改动数字必须重跑那两条命令。

## 0. 结论

**控制面与执行环境必须分开回答，因为二者的平台结论相反。**

1. **控制面是平台中立的，且有机器证据。** `crates/*/src` 全树 **0 处**平台条件代码（`#[cfg(windows)]`、
   `#[cfg(unix)]`、`std::process::Command`、`tokio::process`、`libc::`、`nix::`、`std::path::MAIN_SEPARATOR`
   等 14 类标记全为零命中）。这不是"看起来可移植"，而是被门禁锁死的不变量：任何平台条件代码一旦落地，
   必须先在本文 §3.1 声明它属于哪个平台、为什么。
2. **执行环境需要 Linux 内核，Windows 原生宿主在本仓的隔离模型下不可用。** 本仓的宿主能力词汇表
   （`tools/testing/sandbox-host-capability-evidence.mjs` 的 `HOST_CAPABILITY_IDS`）全部是 Linux 内核原语：
   命名空间、cgroup v2、overlayfs、seccomp。在 Git Bash（MSYS）身份下实测 **47 项里只有 4 项 verified，
   且没有一项与隔离有关**。
3. **WSL Ubuntu 22.04 是本机唯一具备真实隔离原语的宿主**，实测 47 项中 29 项 verified，但**以非特权身份
   有 7 项被拒**。也就是说"能不能跑"取决于身份与委派配置，不只取决于内核。
4. **本轮没有任何平台达到 `verified`。** 本仓 Phase 0 不存在运行时可执行物，"创建环境"本身尚未实现，
   因此平台结论只能是**原语可用性**结论，而不是端到端可用性结论。

## 1. 平台支持矩阵

### 1.1 平台词汇

平台 id 由 `tools/check-sandbox-platform-code.mjs` 的 `PLATFORM_IDS` 固定，矩阵必须逐行覆盖，不能新增。

| 平台 id | 含义 |
| --- | --- |
| `windows-x64` | Windows 原生（含 Git Bash / MSYS 身份） |
| `linux-x64-wsl2` | WSL2 下的 Ubuntu（微软标准内核） |
| `linux-x64-native` | 裸机 x86_64 Linux |
| `linux-aarch64` | 裸机 arm64 Linux |
| `macos-arm64` | Apple Silicon macOS |

### 1.2 平台支持矩阵

| 平台 | 状态 | 证据与判据 |
| --- | --- | --- |
| `windows-x64` | `unsupported` | 实测：47 项宿主能力中 verified 仅 4 项（`platform.kernel` 报 `MINGW64_NT-10.0-26200`、`platform.arch`、`filesystem.root-type` 报 `UNKNOWN`、`toolchain.mount`），`platform.distro` unverifiable（无 `/etc/os-release`），`cgroup.v2-mounted` / `filesystem.overlayfs-supported` 为 `unsupported`，`security.seccomp-mode` 为 `unverifiable`。判据来自契约 `docs/architecture/tech/TECH-security-and-operations.md` 与本仓能力词汇表，见 `tools/testing/sandbox-host-capability-evidence.mjs`。作为**宿主**承载隔离执行环境不可用；作为**控制面客户端**不受此结论约束 |
| `linux-x64-wsl2` | `partial` | 实测（内核 `6.6.87.2-microsoft-standard-WSL2`，Ubuntu 22.04.5 LTS）：47 项中 verified 29 / denied 7 / unsupported 4 / unverifiable 7。verified 含全部 6 类 **user namespace**、`cgroup.v2-mounted`（`cgroup2fs`）、`cgroup.controllers`（`cpuset cpu io memory hugetlb pids rdma`）、`cgroup.pids-controller`、`cgroup.subtree-control`（`enabled=[memory pids]`）、`filesystem.overlayfs-supported`、`filesystem.tmpfs-supported`、`filesystem.openat2-kernel-version`、`security.seccomp-mode`（`mode=2`）、`security.userns-budget`（`max_user_namespaces=62785`）、以及 `unshare`/`mount`/`ip`/`nft`/`iptables`/`capsh`/`nsenter` 工具链。denied 7 项为**直接**（非 user namespace）的 `namespace.mount`、`namespace.pid`、`namespace.uts`、`namespace.ipc`、`namespace.net`、`namespace.cgroup`、`namespace.time`，原因 `unshare failed: Operation not permitted` —— 这是**探测身份（非 root、无委派）**的限制，不是内核缺失。unsupported 4 项为 `cgroup.kill-file`、`cgroup.events-file`、`cgroup.root-writable`、`isolation.kvm-device`（存在但非读写）。判据：以特权身份或配置 cgroup 委派后需重测才能升级为 `verified`，见 `docs/architecture/tech/TECH-runtime-backends-and-pools.md` |
| `linux-x64-native` | `unmeasured` | 本机没有裸机 Linux 节点。该行拒答而非猜测：宿主能力探测工具 `tools/testing/sandbox-host-capability-evidence.mjs` 一次只探本机，本机两个身份分别是 MSYS 与 WSL2，都不是裸机。补齐前的判据与测法见本文 §5，记录口径见 `docs/architecture/tech/TECH-performance-and-capacity.md`；在补齐前不得引用为可用平台 |
| `linux-aarch64` | `unmeasured` | 同上，且 arm64 需单独实测：`security.seccomp-mode` 与 `cgroup.controllers` 在 arm64 内核上的取值与 x86_64 不必然一致，`isolation.kvm-device` 的可用性也取决于 ARM 虚拟化扩展。判据与测法见本文 §5 |
| `macos-arm64` | `unsupported` | 词汇表层面的判据，非实测：本仓 `tools/testing/sandbox-host-capability-evidence.mjs` 的 `HOST_CAPABILITY_IDS` 要求的原语（`namespace.mount`、`cgroup.v2-mounted`、`filesystem.overlayfs-supported`、`security.seccomp-mode`）在 Darwin 内核上不存在，macOS 上的容器实现本身依赖 Linux 虚拟机。结论依据该文件的能力词汇表；未做实测，故不声明任何实测数字 |

**读法提醒**：`unsupported` 在这里的含义是"不能作为宿主承载本仓目标隔离模型"，不是"装不上"。
控制面（生命周期服务、仓储、契约）在三个平台上都能编译运行。

## 2. 宿主能力实测对照

两次探测由同一个工具在**同一台机器**上完成：`node tools/testing/sandbox-host-capability-evidence.mjs`。
命令与产物：

```bash
node tools/testing/sandbox-host-capability-evidence.mjs --json --out target/host-capability-evidence.windows.json
node tools/testing/sandbox-host-capability-evidence.mjs --target wsl:Ubuntu-22.04 --json --out target/host-capability-evidence.wsl-ubuntu-2204.json
```

参考环境：Intel(R) Core(TM) Ultra 7 255H · 16 逻辑核 · 16 GiB 内存。

| 维度 | Windows（Git Bash / MSYS 身份） | WSL Ubuntu 22.04 |
| --- | --- | --- |
| 探测项数 | 47 | 47 |
| verified | 4 | 29 |
| denied | 0 | 7 |
| unsupported | 5 | 4 |
| unverifiable | 38 | 7 |
| 平台内核标识 | `MINGW64_NT-10.0-26200 3.6.9-b4195d69.x86_64 x86_64 Msys` | `Linux 6.6.87.2-microsoft-standard-WSL2 x86_64` |
| 发行版 | unverifiable（无 `/etc/os-release`） | `Ubuntu 22.04.5 LTS` |
| user namespace | 不可用（`unshare` 未安装） | 6 类全通过 |
| cgroup v2 | `unsupported`（无 `/sys/fs/cgroup`） | `cgroup2fs`，controllers / pids / subtree_control 均 verified |
| overlayfs / tmpfs | `unsupported`（`/proc/filesystems` 无 overlay） | 两项 verified |
| seccomp | `unverifiable`（无 Seccomp 字段） | `mode=2` |
| KVM 设备 | 不适用 | 存在但 `crw-rw---- root kvm`，非读写 |

三项结论：

- **Windows 侧的 4 项 verified 里没有一项与隔离有关**，`platform.kernel` 报的还是 MSYS 层而不是 Windows 内核本身。
  这暴露一个结构性问题：`HOST_CAPABILITY_IDS` 里**没有 Windows 原生的能力 id**
  （Hyper-V、Windows Sandbox、Job Object、AppContainer 都没有），所以这个框架**在结构上无法为 Windows 宿主
  提供任何证据**，无论 Windows 实际具备什么。要评估 Windows 原生隔离，必须先扩展词汇表。
- **WSL2 的 7 项 denied 是身份问题，不是内核问题。** 探测以非特权 `ubuntu` 执行；不带 user namespace 的
  mount/pid/net 命名空间创建需要 `CAP_SYS_ADMIN`，因此在初始 user namespace 里必然 `EPERM`。
  以 user namespace 包裹的同名探测全部通过。
- **WSL2 的 4 项 unsupported 中有 3 项是权限而非缺失**：`cgroup.root-writable` 是该身份不可写，
  `cgroup.kill-file` / `cgroup.events-file` 在 root cgroup 上不存在（只在非 root cgroup 上出现）。
  这三项正是 `--probe-write` 覆盖的范围。

## 3. 平台条件代码

### 3.1 平台条件代码声明

规则：`crates/*/src` 下**任何**平台标记都必须在下列表格中声明，声明项必须标明所属平台与理由；
声明表与代码双向一致，且声明总数必须与 §表末的计数一致。测试代码同样计入，因为平台条件的**测试**
也是一种平台声明。

当前声明：0

| 文件 | 标记 | 平台 | 理由 |
| --- | --- | --- | --- |
| （无） | | | |

**为什么现在是空的，以及它为什么必须保持为空或被显式声明。** 控制面之所以可移植，是因为它把
所有平台相关工作都留给 Provider SPI 背后的适配器，服务层只做编排。因此：
新增任何平台条件代码前，必须先确定它属于"控制面"还是"某个 Provider 实现"。属于控制面的平台条件代码
是架构缺陷，应改为 SPI 能力位（`RuntimeCapability` / `IsolationAssurance`）；属于具体 Provider 的，
应在该 Provider 的 crate 内落地并在此登记。

性能测量所需的 OS 采样也被刻意留在仓库外
（[`tools/bench-sandbox-lifecycle.mjs`](../../../tools/bench-sandbox-lifecycle.mjs) 的 Node 侧），
就是为了让这条规则保持成立。

## 4. 可移植性门禁

### 4.1 可移植性门禁

| 门禁 | 脚本 | package.json 脚本名 |
| --- | --- | --- |
| 机器绝对路径（跨 Windows/macOS/Linux 的核心不变量） | `../sdkwork-specs/tools/check-workspace-path-portability.mjs` | `portability:paths:check` |
| shell 语法与跨平台写法 | `../sdkwork-specs/tools/check-shell-portability.mjs` | `portability:shell:check` |
| 平台条件代码与平台声明一致性（本文的校验器） | `tools/check-sandbox-platform-code.mjs` | `portability:platform:check` |

前两条门禁来自 `sdkwork-specs`，但**在本仓此前从未被执行**：两者都通过（路径 365 个文件 0 问题、
shell 11 个文件 0 问题），却都不在 `package.json` 里，因此从未进入 `pnpm run check`。
`DEPENDENCY_MANAGEMENT_SPEC.md` 第 1 节要求的"跨 Windows、macOS、Linux 可移植"因此长期没有执行者。
本仓现已把它们接入检查链。

## 5. 复核方式

```bash
node tools/check-sandbox-platform-code.mjs
node tools/check-sandbox-platform-code.mjs --json
node tools/check-sandbox-platform-code.mjs --root <dir>
node --test tests/contract/sandbox-platform-code-tool.contract.test.mjs
node tools/testing/sandbox-host-capability-evidence.mjs --target wsl:Ubuntu-22.04 --probe-write --json --out target/host-capability-evidence.wsl-ubuntu-2204-probe-write.json
```

`--root` 审计另一棵树，契约测试用它证明每条规则族都能变红。

**`--probe-write` 尚未在本轮执行**，因此 `cgroup.delegation-writable`、`cgroup.delegated-subtree-files`、
`filesystem.overlayfs-usable` 三项在本文中仍是 `unverifiable`。这三项是判定 WSL2 能否承载
命名空间型沙箱的关键，应在下一次能力复核中补测。

升格任一行到 `verified` 的前提，按 `TECH-performance-and-capacity.md` §9 的口径：
必须在记录参考环境、Template、工作负载与统计方法之后，用真实硬件基准测得，不得用原语可用性代替。
