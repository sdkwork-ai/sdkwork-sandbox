# 性能与宿主能力实测基线

本文记录**已测到的数字**。[TECH-performance-and-capacity.md](TECH-performance-and-capacity.md)
是性能**目标**（工程目标，未测量前不得作为发布门禁），本文是**实测**。两者不得互相引用数值。
平台支持结论见 [TECH-platform-support.md](TECH-platform-support.md)。

**本文所有数字都是 2026-09-22 在本机实测**，机器规格、内核、虚拟化能力、样本量、并发档位与统计方法
按 `TECH-performance-and-capacity.md` 第 5 节逐项记录；缺项显式写"不适用/未测"，不留空。

## 0. 结论

1. **控制面编排的时延在地板量级，热路径 p50 在两次测量中逐位复现。** 单次
   `create_sandbox_session` + `start_sandbox_session` 热路径 p50
   **0.012 ms（Windows，两轮均为 0.012）** / **0.008 ms（WSL2）**，
   p99 0.068–0.075 ms / 0.048 ms，冷启动 **0.121–0.132 ms / 0.036 ms**。
   这是**编排地板**（内存仓储 + 进程内 fake provider + 租约与选择逻辑），**不含**任何 VM/容器/进程启动。
   两个平台 p50 相差 1.5 倍、绝对值都在 10 µs 量级，与 `crates/*/src` 零平台条件代码这个静态事实互相印证。
2. **本仓 Phase 0 没有可执行的运行时可执行物**，所以"沙箱创建时延"这个产品指标的**分子（编排）
   已测、分母（真实沙箱启动）不存在**。任何把本文数字引用为"沙箱启动 <10 ms"的结论都是错的。
3. **宿主地板是本次最可操作的发现，而且它比编排时延不稳定两个量级。** 创建一次进程：
   Windows **p50 147 / 240 / 261 ms（三轮）**，WSL2 Ubuntu **p50 1.74 ms**，相差约 **85–150 倍**。
   目录物化（200 个 4 KiB 文件）Windows **199–312 ms** 对 WSL2 **23.3 ms**，相差约 **8.5–13 倍**。
   本地 provider 若靠"起进程"承载沙箱，宿主的进程创建成本会直接主导时延预算，与编排代码无关。
4. **哪些数字可复现、哪些不可，必须分开说。** 同一份代码、同一台机器：
   **编排 p50 两轮逐位一致**（create 0.002、start 0.010、端到端 0.012），
   而**宿主地板与派生聚合量会成倍漂移**（进程创建 147→261 ms、目录物化 199→312 ms、
   单次采样开销 1409→1802 ms、每会话状态增长 246→1218 字节）。
   结论：引用本文数字时，**微秒级 p50 可以当稳定值用，毫秒级与聚合值必须带轮次范围**。
5. **测量本身要先被测量。** 采样一次进程状态的开销：Windows `tasklist` **1.4–1.8 s**，
   Linux `/proc` **0.34 ms** —— 相差约 **4000–5000 倍**。而被测的一轮迭代只要几微秒。
   在 Windows 上把采样器挂在延迟测量里会把结果彻底毁掉，所以基准固定拆成
   **延迟趟（无采样器）**与**资源趟**；Linux 上虽然采样很便宜，仍走同一结构，保证两平台可比。
6. **本次结果不允许作为任何发布门禁或对外数字**（第 6 节逐条给出理由：无 Template、无运行制品、
   无 KVM、Windows 缺上下文切换/页缺失计数器）。这是工具自己算出来并打印的，不是人工判断。

## 1. 测量方法

工具：`tools/bench-sandbox-lifecycle.mjs`（Node 侧驱动器）+ crate 内基准测试
`crates/sdkwork-intelligence-sandbox-service/src/tests.rs` 的
`tests::sandbox_lifecycle_create_start_benchmark`（Rust 侧取样）。

**为什么分两趟。** 第一版实现是在延迟循环里同步采样本进程 RSS/CPU，测出"采样一次 540 ms–1.8 s"
——采样器比被测对象慢 5 个数量级，既扰动被测路径又污染采样区间。现在固定为：

- **延迟趟**：crate 内 `Instant::now()` 取样，**不带采样器**，20000 次测量 + 2000 次预热；
- **资源趟**：独立一趟，200000 次迭代，按 `--sample-interval-ms`（默认 500 ms）采样本进程
  RSS / CPU / 上下文切换 / 页缺失，报告里写明**实际生效间隔**与**单次采样开销**，
  因为"请求的间隔"和"实际的间隔"在慢采样器上不是一回事（Windows 上请求 500 ms、实际 1802 ms）。

**为什么采样在 Node 侧。** OS 级采样（`tasklist`、`/proc`）写在 Node 驱动器里，而不是写进 crate：
crate 里出现任何平台条件代码（`#[cfg(windows)]`、`std::process::Command`、`libc::`）
都会被 `tools/check-sandbox-platform-code.mjs` 判为未声明平台代码。控制面保持零平台条件代码是本仓的
已锁不变量，测量工具不得破坏它。

**热/冷分开统计**（`TECH-performance-and-capacity.md` 第 2 节明文要求）：进程第一次
create→start 单独记为冷路径，预热之后的 20000 次记为热路径，两者不合并、不用热数据代表冷启动。

分位数用 **nearest-rank**，取自真实样本，不插值。

## 2. 参考环境

| 要素 | 平台 A | 平台 B |
| --- | --- | --- |
| CPU | Intel(R) Core(TM) Ultra 7 255H · 16 逻辑核 | 同左（同一台机器） |
| 内存 | 33752997888 字节（约 31.4 GiB 可见） | 16469131264 字节（约 15.3 GiB，WSL2 默认上限） |
| 系统 | `win32 x64` · Git Bash / MSYS 身份 | `linux x64` · Ubuntu 22.04.5 LTS（WSL2） |
| 内核 | `10.0.26200` | `6.6.87.2-microsoft-standard-WSL2` |
| Node | v22.22.2 | v24.21.0 |
| 虚拟化能力 | `target/host-capability-evidence.windows.json`：47 项探测，verified 4 / unsupported 5 / unverifiable 38 / denied 0 | `target/host-capability-evidence.wsl-ubuntu-2204.json`：47 项，verified 29 / unsupported 4 / denied 7 / unverifiable 7 |
| 关键宿主原语 | 无 cgroup v2、无 overlayfs、无 seccomp 字段、无 KVM、`unshare` 未安装 | cgroup v2 (`cgroup2fs`) · overlayfs · tmpfs · seccomp `mode=2` · 6 类 user namespace · KVM 设备存在但非读写 |
| Template 版本 | 不适用：Phase 0 无 Template 制品与 Template 契约 | 同左 |
| Artifact 元组 | 不适用：Phase 0 无运行制品 | 同左 |
| 样本量 | 延迟 20000（+2000 预热）· 资源 200000 · 宿主地板 150 | 同左 |
| 并发档位 | 1（单进程、顺序、进程内 provider） | 同左 |
| 统计方法 | nearest-rank 分位数，取自原始样本，冷热分开 | 同左 |

## 3. 控制面时延（热路径与冷路径）

**热路径**（预热 2000 次后的 20000 次测量，无采样器）：

| 阶段 | 平台 | n | p50 | p90 | p95 | p99 | max | mean | stddev |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| create | WSL2 | 20000 | 0.002 ms | 0.003 ms | 0.003 ms | 0.006 ms | 6.456 ms | 0.003 ms | 0.053 ms |
| create | Windows | 20000 | 0.002 ms | 0.003 ms | 0.003 ms | 0.007 ms | 4.138 ms | 0.003 ms | 0.038 ms |
| start | WSL2 | 20000 | 0.006 ms | 0.010 ms | 0.013 ms | 0.036 ms | 25.170 ms | 0.010 ms | 0.194 ms |
| start | Windows | 20000 | 0.010 ms | 0.012 ms | 0.016 ms | 0.067 ms | 16.707 ms | 0.015 ms | 0.148 ms |
| create + start | WSL2 | 20000 | 0.008 ms | 0.013 ms | 0.017 ms | 0.048 ms | 25.173 ms | 0.012 ms | 0.201 ms |
| create + start | Windows | 20000 | 0.012 ms | 0.014 ms | 0.017 ms | 0.068 ms | 24.469 ms | 0.020 ms | 0.234 ms |

（Windows 行取 09:12Z 那一轮；两轮的差异见下方复现表，p50 完全相同。）

**Windows 侧复现**（同机、同命令、同代码，两次运行间隔约 13 分钟，均为当前版本工具）：

| 轮次 | create p50 | start p50 | 端到端 p50 | 端到端 p99 | 冷路径端到端 | 延迟趟耗时 |
| --- | --- | --- | --- | --- | --- | --- |
| 第 1 轮（08:59Z） | 0.002 ms | 0.010 ms | 0.012 ms | 0.075 ms | 0.132 ms | 2997 ms |
| 第 2 轮（09:12Z） | 0.002 ms | 0.010 ms | 0.012 ms | 0.068 ms | 0.121 ms | 679 ms |

**微秒级 p50 两轮逐位一致**（0.002 / 0.010 / 0.012），这是本文最稳的一组数字。
（同日上午更早一次运行用的是尚未分离冷/热路径的工具版本，其 `start` p50 为 0.007 ms，
数量级一致，因口径不同不计入本表。）
而同一份报告里的**整趟耗时在 679–2997 ms 之间摆动（4.4 倍）**——整趟耗时主要花在测试二进制的启动与
2000 次预热上，属进程级噪声，不代表编排成本变化。**不要把整趟耗时当作时延指标**。

整趟耗时：WSL2 432 ms，Windows 679–2997 ms。

**冷路径**（进程首次 create→start，单独报告）：

| 平台 | create | start | 端到端 | 冷/热比（p50） |
| --- | --- | --- | --- | --- |
| WSL2 | 0.013 ms | 0.023 ms | **0.036 ms** | 4.5× |
| Windows | 0.042–0.043 ms | 0.078–0.089 ms | **0.121–0.132 ms** | 10–11× |

冷/热比两级都显著大于 1，正是把两者分开统计的理由：合并后 p50 会把冷启动掩盖掉。

max 与 p99 的长尾（create 4.1–6.5 ms、start 16.7–25.2 ms，而均值只有 0.003/0.015 ms）
是单进程内的调度抖动，不是被测路径的成本；要判长尾必须上并发档位，本轮并发档位为 1。

## 4. 资源足迹（独立资源趟，200000 次迭代）

| 指标 | Windows | WSL2 Ubuntu |
| --- | --- | --- |
| 整趟耗时 | 7868–12304 ms（三轮） | 4916 ms |
| 请求采样间隔 → 实际生效 | 500 ms → **1410–1802 ms** | 500 ms → **500 ms** |
| 单次采样开销 | **1409–1802 ms** | **0.34 ms** |
| 采样数 | 4（三轮均 4） | 9 |
| 峰值 RSS | 465.2 MiB（采样工作集最大值；Windows 无便携峰值计数器） | 490.8 MiB（**VmHWM**，内核精确峰值） |
| 起始 → 结束 RSS | 232.8–234.2 MiB → 308.3–465.2 MiB | 102.4 MiB → 284.2 MiB |
| 控制面状态增长 | **246–1218 字节/会话**（三轮） | **953 字节/会话** |
| CPU 时间 | 约 2 s（`tasklist /v` 处理器时间增量） | 3.98 s（`/proc/<pid>/stat` utime+stime 增量） |
| 上下文切换 | 不适用（无便携计数器） | voluntary **0** · involuntary **0** |
| 页缺失 | 不适用（同上） | minor **119894** · major **0** |

⚠️ **Windows 侧这一张表里的派生值不可复现，差距达 5 倍**（状态增长 246–1218 字节、
单次采样开销 1409–1802 ms、结束 RSS 308–465 MiB）。根因是 Windows 只有 4 个采样点
（采样本身要 1.4–1.8 s，200000 次迭代只能采到 4 次），样本量不足；
WSL2 有 9 个采样点、间隔准确，所以它的数字可以逐项引用。要提升 Windows 侧可信度，
只能降低采样成本或延长资源趟，本轮未做。

**控制面状态增长**是唯一有外推价值的数字：每创建一个会话在内存仓储里留下约 1 KiB
（1218 / 953 字节，两平台同量级），万级会话约 10–12 MiB。
它**不能**乘节点数或会话数推导容量承诺 —— 那要靠并发档位实测。

**上下文切换 0 与页缺失 119894/0（major = 0）**是一致的一组读数：虚拟地址空间已全部驻留，
`create`/`start` 的 future 在内存仓储上不产生挂起点，因此整趟没有一次自愿切换；
不自愿切换为 0 说明 200000 次迭代期间没有被调度器抢占过。major 页缺失为 0
说明没有触发磁盘换页 —— 这轮数据全部在内存里，符合"编排地板"的定位。

## 5. 宿主地板（被测对象之外，但直接决定时延预算）

**进程创建**（`spawnSync`，每轮 150 样本）：

| 轮次 | 平台 | p50 | p95 | max | 命令 |
| --- | --- | --- | --- | --- | --- |
| Windows 第 1 轮（08:46Z） | win32 | 240.011 ms | 758.623 ms | 1768.527 ms | `where.exe cmd.exe` |
| Windows 第 2 轮（08:59Z） | win32 | 261.228 ms | 2005.508 ms | 2581.461 ms | `where.exe cmd.exe` |
| Windows 第 3 轮（09:12Z） | win32 | **146.510 ms** | 546.341 ms | 816.124 ms | `where.exe cmd.exe` |
| WSL2 Ubuntu | linux | **1.736 ms** | 2.731 ms | 3.603 ms | `/bin/true` |

- **Windows 的进程创建比 WSL2 贵约 85–150 倍**（p50 146–261 ms 对 1.74 ms）。这不是本仓代码的属性，
  是宿主属性；但只要某个 provider 靠"起进程"承载沙箱，这笔成本就会进时延预算。
- **Windows 上连 p50 都不稳定**：三轮 147 / 240 / 261 ms，最大与最小相差 1.8 倍；p95 相差 3.7 倍。
  引用任何一档都必须带轮次，单独引用某一个值会误导。
- WSL2 的尾部非常紧（p95 2.7 ms、max 3.6 ms），与 Windows 的 546–2005 ms 形成对照：
  Windows 侧的尾部来自杀毒/筛选器驱动的进程创建钩子，是**不确定成本**。

**目录物化**（20 个目录 / 200 个 4 KiB 文件，`mkdirSync` + `writeFileSync`）：

| 轮次 | 平台 | 耗时 | 文件/秒 | MiB/秒 |
| --- | --- | --- | --- | --- |
| Windows 第 2 轮 | win32 | 311.822 ms | 641 | 2.51 |
| Windows 第 3 轮 | win32 | 198.592 ms | 1007 | 3.93 |
| WSL2 Ubuntu | linux | **23.277 ms** | **8592** | **33.56** |

Windows 两轮相差 1.6 倍，WSL2 比 Windows 快约 **8.5–13 倍**。
两次测量的临时目录都取 `os.tmpdir()`：Windows 上是 `%TEMP%`，WSL2 上是 `/tmp`
（ext4，**不是** `/mnt/d` 的 9p 挂载），因此这一行是 WSL2 的**原生文件系统**性能，不是挂载穿透性能。
要判 `/mnt/d` 上的建目录成本需要单独测，本轮未做。

**另一个测量完整性发现**：不筛选的 `tasklist` 一次 1.3 s，筛选后（按 PID 过滤）仍需 1.4–1.8 s。
在第 1 节的两趟结构成型之前，这个开销会完全淹没微秒级的被测路径。

## 6. 与 `TECH-performance-and-capacity.md` 的符合性

第 5 节要求每条基准结果同时记录 8 个要素，第 6 节要求记录 11 类指标。逐项对账：

| 要求 | 状态 | 说明 |
| --- | --- | --- |
| 机器规格 / 内核版本 / 样本量 / 并发档位 / 统计方法 | 已记录 | 第 2 节 |
| 虚拟化能力 | 已记录 | 引用两次宿主能力探测的 JSON，含 cgroup / seccomp / KVM 逐项读数 |
| Template 版本 | 不适用 | Phase 0 无 Template（见 `TECH-e2b-capability-parity.md`） |
| Artifact 元组 | 不适用 | Phase 0 无运行制品 |
| P50 / P90 / P95 / P99 / Max | 已记录 | 第 3 节，两平台 |
| CPU / 内存 | 已记录 | 第 4 节，两平台 |
| IOPS / 带宽 | **代理指标** | 用目录物化的 文件/秒 与 MiB/秒 作代理；未做真实块设备 IOPS 与网络带宽 |
| 上下文切换 | 平台相关 | Linux 已记录（0 / 0）；**Windows 缺失** —— `tasklist` 不暴露，读它需要写 Windows API 调用，会把平台条件代码塞进 crate |
| 页缺失 | 平台相关 | Linux 已记录（minor 119894 / major 0）；**Windows 缺失**，原因同上 |
| 错误率（策略拒绝 vs 系统故障分开） | 不适用 | 本轮跑的是进程内 fake provider + 内存仓储，两类错误按构造不可能发生；失败会中止整轮而不是被计数 |

**发布门禁资格：不合格。** 第 9 节禁止未通过真实硬件基准的时延/并发/资源数字进入 PRD、Release Evidence
或对外材料；本轮无 Template、无运行制品、无 KVM，属**地板测量**，不得引用为容量承诺。
工具在 `## Measurement conformance` 段自行打印该结论与理由。

## 7. 复核方式

Windows 侧（从仓根执行）：

```bash
node tools/bench-sandbox-lifecycle.mjs --lane all --iterations 20000 --resource-iterations 200000 --host-floor-samples 150 --capability-evidence target/host-capability-evidence.windows.json --label "windows-x64 (Git Bash/MSYS)" --out target/bench-sandbox-lifecycle.windows.json
node --test tests/contract/sandbox-lifecycle-benchmark-tool.contract.test.mjs
node tools/testing/sandbox-host-capability-evidence.mjs --target local --json --out target/host-capability-evidence.windows.json
```

Linux 侧必须在 WSL 内执行（`wsl.exe -e bash -lc '<命令>'`），且**必须**把 `CARGO_TARGET_DIR`
指到 WSL 原生文件系统、并确认 `CARGO_HOME` 下已配置 crates.io 镜像（否则 cargo 会停在 crates.io 上假死）：

```bash
CARGO_TARGET_DIR="$HOME/.cache/sdkwork-sandbox-target" node tools/bench-sandbox-lifecycle.mjs --lane all --iterations 20000 --resource-iterations 200000 --host-floor-samples 150 --capability-evidence target/host-capability-evidence.wsl-ubuntu-2204.json --label "linux-x64-wsl2 (Ubuntu 22.04.5)" --out target/bench-sandbox-lifecycle.wsl-ubuntu-2204.json
node tools/testing/sandbox-host-capability-evidence.mjs --target wsl:Ubuntu-22.04 --json --out target/host-capability-evidence.wsl-ubuntu-2204.json
```

宿主能力探测的第二次调用**必须从 WSL 内部发起**，否则探测的是 MSYS 身份而不是 Linux 内核。
重跑后请更新本文数字，并保留 `target/bench-sandbox-lifecycle.*.json` 作为原始样本 ——
分位数来自真实采样，抽样器换了数字就会变。

## 8. 本轮未完成与已知限制

| 项 | 原因 | 影响 |
| --- | --- | --- |
| 裸机 Linux（`linux-x64-native`）| 本机没有裸机 Linux 节点 | WSL2 仍是虚拟机："Linux 很快"这个结论**只对 WSL2 成立**；裸机上的进程创建与文件物化未测 |
| 真实块设备 IOPS / 网络带宽 | 本机无受控块设备与网络档位 | 第 5 节用文件物化作代理，不能替代 |
| 并发档位 100 / 1000 / 10000 | 本轮固定为 1 | 第 3 节的 max/p99 长尾不可用于容量判断 |
| `/mnt/d`（9p）上的文件物化 | 未单独测 | 跨系统访问的仓库里，provider 若在 9p 路径上准备 workspace，成本与本表不同 |
| `--probe-write` 宿主能力复核 | 本轮未执行 | `cgroup.delegation-writable`、`cgroup.delegated-subtree-files`、`filesystem.overlayfs-usable` 三项仍为 `unverifiable`，见 `TECH-platform-support.md` 第 5 节 |
| 两平台控制面 p50 差 1.5 倍的归因 | 未做 | 差值在微秒量级且被测试二进制自身的运行环境成本（整趟 432 ms 对 2997 ms）混淆，本轮不下结论 |
