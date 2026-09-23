# Gate 0 Exit Readiness Package

Status: active

Owner: SDKWork Runtime Platform

Updated: 2026-09-22

## 目的

本文档汇总 Gate 0 退出所需的人工评审材料，为评审者提供一站式检查入口。

## Gate 0 退出条件

根据 `specs/sandbox-provider-delivery-gates.contract.json`：

1. Provider Delivery Gate 的 11 个 Review Packet 全部完成人工评审前，`implementationAuthorized` 保持 `false`
2. Service Host 与 Observability/Event/Outbox 是运行时激活和商业发布的补充门禁，不能因不在 Provider Delivery Gate 的 `humanReview.reviewPackets` 数组中而跳过
3. 当前全部 **19** 个相关 Review Packet 状态均为 `pending-human-review`（2026-09-24 首批 4 个包——local-provider、command-execution、firecracker-provider、e2b-api-sdk-authority——已获仓库所有者全部评审角色批准进入 `accepted`）。该计数不手工维护：它必须等于 `node tools/check-sandbox-human-review-signoff.mjs` 输出里的 `pending human review`，并且下文表格必须逐行列全这 19 个待签包与已签包的当前状态——两道一致性由同一门禁校验（见本节末尾）。
4. 人工评审通过后，REQ、ADR、门禁契约、Component Contract 与实现证据必须同步更新，不能只修改 `implementationAuthorized`

当前商业发布判定为 **No-Go**。完整问题、交付顺序和发布证据见 [PLAN-2026-0002](plans/PLAN-2026-0002-commercial-cloud-agent-runtime-delivery.md)。

## 评审包总览

| # | Review ID | ADR | REQ | 风险 | 状态 |
| --- | --- | --- | --- | --- | --- |
| 1 | REVIEW-20260729-sandbox-command-execution-architecture-security | ADR-20260729-sandbox-command-execution-and-terminal-boundary | REQ-2026-0007 | critical | accepted |
| 2 | REVIEW-20260729-local-provider-architecture-security | ADR-20260728-local-provider-assurance-and-host-boundaries | REQ-2026-0003 | critical | accepted |
| 3 | REVIEW-20260729-firecracker-provider-architecture-security | ADR-20260729-firecracker-provider-isolation-and-node-boundaries | REQ-2026-0008 | critical | accepted |
| 4 | REVIEW-20260729-sandbox-host-isolation-broker | ADR-20260729-sandbox-host-isolation-broker-boundary | REQ-2026-0011 | critical | pending-human-review |
| 5 | REVIEW-20260729-sandbox-firecracker-artifact-compatibility-and-supply-chain | ADR-20260729-sandbox-firecracker-artifact-compatibility-and-supply-chain | REQ-2026-0012 | critical | pending-human-review |
| 6 | REVIEW-20260729-sandbox-workspace-block-device-attachment-and-sanitization | ADR-20260729-sandbox-workspace-block-device-attachment-and-sanitization | REQ-2026-0013 | critical | pending-human-review |
| 7 | REVIEW-20260729-sandbox-firecracker-network-isolation | ADR-20260729-sandbox-firecracker-network-isolation-and-egress-policy | REQ-2026-0014 | critical | pending-human-review |
| 8 | REVIEW-20260729-sandbox-firecracker-resource-isolation | ADR-20260729-sandbox-firecracker-resource-isolation-and-usage-facts | REQ-2026-0015 | critical | pending-human-review |
| 9 | REVIEW-20260729-sandbox-multi-tenant-admission-scheduling-and-capacity | ADR-20260729-sandbox-multi-tenant-admission-scheduling-and-capacity-reservation | REQ-2026-0016 | critical | pending-human-review |
| 10 | REVIEW-20260729-sandbox-node-trust-enrollment-attestation-and-inventory | ADR-20260729-sandbox-node-trust-enrollment-attestation-and-inventory | REQ-2026-0017 | critical | pending-human-review |
| 11 | REVIEW-20260729-sandbox-postgresql-quota-and-capacity-persistence | ADR-20260729-sandbox-postgresql-quota-and-capacity-reservation-persistence | REQ-2026-0018 | critical | pending-human-review |
| 12 | REVIEW-20260729-sandbox-service-host-composition-and-readiness | ADR-20260729-sandbox-service-host-composition-and-readiness | REQ-2026-0009 | high | pending-human-review |
| 13 | REVIEW-20260729-sandbox-observability-event-audit-outbox | ADR-20260729-sandbox-observability-event-audit-outbox-boundary | REQ-2026-0010 | 未声明 | pending-human-review |
| 14 | REVIEW-20260730-sandbox-runtime-pool-architecture-security | ADR-20260730-sandbox-runtime-pool-claim-and-sanitization | REQ-2026-0019 | critical | pending-human-review |
| 15 | REVIEW-20260730-sandbox-lifecycle-history-and-idempotency-retention | ADR-20260730-sandbox-lifecycle-hot-state-and-idempotency-ledger | REQ-2026-0020 | high | pending-human-review |
| 16 | REVIEW-20260730-sandbox-workspace-runtime-transaction-architecture-security | ADR-20260730-sandbox-workspace-runtime-transaction-and-checkpoint | REQ-2026-0021 | critical | pending-human-review |
| 17 | REVIEW-20260730-sandbox-standalone-data-residency-and-recovery | ADR-20260730-sandbox-standalone-data-residency-and-recovery | REQ-2026-0022 | critical | pending-human-review |
| 18 | REVIEW-20260731-sandbox-interactive-terminal-session | ADR-20260731-sandbox-interactive-terminal-session | REQ-2026-0024 | critical | pending-human-review |
| 19 | REVIEW-20260731-sandbox-internal-control-plane | ADR-20260731-sandbox-internal-control-plane | REQ-2026-0023 | critical | pending-human-review |
| 20 | REVIEW-20260801-sandbox-runtime-secret-projection | ADR-20260801-sandbox-runtime-secret-projection | REQ-2026-0025 | critical | pending-human-review |
| 21 | REVIEW-20260801-sandbox-cloud-data-residency-and-recovery | ADR-20260801-sandbox-cloud-data-residency-and-recovery | REQ-2026-0026 | critical | pending-human-review |
| 22 | REVIEW-20260801-sandbox-cross-repository-version-compatibility | ADR-20260801-sandbox-cross-repository-version-compatibility | REQ-2026-0027 | critical | pending-human-review |
| 23 | REVIEW-20260924-sandbox-e2b-api-sdk-authority | ADR-20260924-sandbox-e2b-api-sdk-authority | REQ-2026-0028 | high | accepted |

**这张表的来源与校验**（2026-09-22 复核）：`风险` 列不是本页判定，它是每个 Review Packet 自己 `Risk:` 头的投影；`状态` 列同理取包内 `Status:`。第 18–22 行是本轮复核新补的——此前它们已处于 `pending-human-review`，却既不在本页表中、也不被任何 `specs/` 契约具名，因此**任何只读本页的评审者都会漏掉它们**。

复核同时修正了 4 行被低估的风险值（`REVIEW-20260729-sandbox-firecracker-artifact-compatibility-and-supply-chain`、`REVIEW-20260729-sandbox-workspace-block-device-attachment-and-sanitization`、`REVIEW-20260729-sandbox-firecracker-resource-isolation`、`REVIEW-20260729-sandbox-postgresql-quota-and-capacity-persistence`：本页原写 `high`，包内声明 `critical`）。本页是摘要，包内声明是记录，故以包内为准。

`REVIEW-20260729-sandbox-observability-event-audit-outbox` 写 `未声明`，因为它确实没有 `Risk:` 头——它是 22 个 pending 包中唯一缺失该头的记录，需要在包内补登，而不是由本页代为判定。

上述三项（计数、逐行列全、风险值一致）现在由 `node tools/check-sandbox-human-review-signoff.mjs` 校验，本页不再手工维护。

## 评审决策摘要

### CMD-01 至 CMD-13: Command Execution

| ID | 决策 | 接受效果 |
| --- | --- | --- |
| CMD-01 | SandboxProvider 保持 Lifecycle，Command 独立 Port | 接口隔离 |
| CMD-02 | 公共类型固定命名 | 进入 Provider SPI Public Export |
| CMD-03 | Terminal 第一版只表示有界非交互 Executable+Argv | 禁止 Command String/Shell |
| CMD-04 | Request 必须携带完整 Identity/Ownership | 副作用前验证 |
| CMD-05 | stdout/stderr 有界 Byte Buffer，不强制 UTF-8 | 保留任意字节 |
| CMD-06 | Timeout/Cancel/Output Limit 共用有界 Cleanup | 固定安全收敛语义 |
| CMD-07 | Descriptor 需 Lifecycle+Executor+Conformance 同时存在才声明 Terminal | 关闭失败 |
| CMD-08 | 同 Operation+Fingerprint 可重放，不同 Fingerprint 冲突 | 固定重试语义 |
| CMD-09 | Fingerprint 由 Sandbox Service 派生，Executor 必须重算 | 防伪造 |
| CMD-10 | Cancel 独立幂等，终态 Result 不伪装可重试错误 | 取消可审计 |
| CMD-11 | durable first-terminal CAS 仲裁竞争 | 终态唯一 |
| CMD-12 | Logical Executable 只由绑定期不可变的 Provider Registry 解析 | 禁止 Caller Path、PATH/CWD Search 和重放 Binary 漂移 |
| CMD-13 | Final Environment 从空集合按绑定期不可变 Policy 构造 | 禁止请求扩展 Policy 或覆盖受保护 Provider 值 |

### Local Provider (REQ-2026-0003) 决策

| 决策 ID | 决策 | 接受效果 |
| --- | --- | --- |
| LOCAL-01 | Kind 固定 `local`，Assurance 固定 `HostUser` | 不承诺更强或多租户隔离 |
| LOCAL-02 | 只消费 Composition 打开的 Runtime/Workspace Capability Handle | 不接收 Host Root，不从 ID 推导 Path |
| LOCAL-03 | Windows suspended Job Object、Linux race-free delegated cgroup v2；macOS 当前拒绝 Terminal | Capability 与真实平台证据一致 |
| LOCAL-04 | Filesystem 使用 handle-relative no-follow/file-identity verification | canonicalize/check-then-open 不构成安全边界 |
| LOCAL-05 | Command 只使用共享 `SandboxCommandExecutor` 与 Provider-owned Logical Executable Registry | 无 Local-private DTO、Caller Path、PATH/CWD Resolver 或 Shell Wrapper |
| LOCAL-06 | Environment 从空集合按绑定期不可变 Policy 构造 | 不继承 Credential Channel，调用方不能扩展 Policy 或覆盖受保护 Provider 值 |
| LOCAL-07 | 未验证 Egress 前不声明 Network/Browser/Port/Shell | 默认拒绝且无弱回退 |
| LOCAL-08 | Stop/Destroy 幂等清理，失败 Quarantine；不删除 Agents Workspace | 保持 Workspace 所有权和零残留门禁 |

上述决策已由 `specs/sandbox-local-provider-host-boundary.contract.json` 和对应 13 项 Contract Test 固定为可机检候选，但仍等待四类人工 Reviewer 和真实平台证据，不能据此开始 Host I/O 或 Process Spawn。

### Firecracker Provider (REQ-2026-0008) 决策

| 决策 ID | 决策 | 接受效果 |
| --- | --- | --- |
| FIR-01 | Kind `firecracker`，Assurance `MicroVm` | 最强隔离级别 |
| FIR-02 | Artifact Tuple 需精确匹配 | 供应链完整 |
| FIR-03 | Node Trust 优先于 Provider Allocation | Cloud fails closed |
| FIR-04 | Confirmed Reservation-before-Allocate | 容量原子预留 |
| FIR-05 | Guest Auth 不替代 Host Attestation | 双层验证 |

### Workspace Runtime Transaction (REQ-2026-0021) 决策

| 决策 ID | 决策 | 接受效果 |
| --- | --- | --- |
| WRT-01 | Sandbox Service 组合现有窄 Port，不接管 Agents/Storage/Provider 权威 | 高内聚且不形成新单体 |
| WRT-02 | Local/Cloud 共用 Revision/Command/Checkpoint/Compensation，Assurance 和 Adapter 分离 | 一套产品语义，无虚假隔离声明 |
| WRT-03 | ReadWrite 使用单 Writer Revision Target 和 Fencing | 禁止共享写和旧 Writer 复活 |
| WRT-04 | Durable Candidate/Handoff 先于 Runtime Release，Agents 独占 Revision CAS Promotion | 不丢写、不复制 Workspace 权威 |
| WRT-05 | Disconnect Grace 有界，到期执行 Fenced Checkpoint/Cleanup | IDE 恢复与容量安全兼顾 |
| WRT-06 | Host/Storage/Checkpoint/Cleanup 不确定即 Quarantine 且容量继续占用 | 不跨租户复用不确定资源 |

### Standalone Data Residency And Recovery (REQ-2026-0022) 决策

| 决策 ID | 决策 | 接受效果 |
| --- | --- | --- |
| SDR-01..SDR-02 | Local-only Data Gate；持久化与严格本地处理声明分离 | 拓扑/Provider 不再冒充隐私承诺 |
| SDR-03..SDR-04 | 保留四仓单一权威；Server PostgreSQL 与显式 `client-local` SQLite 分离 | 不新增 BirdCoder 业务库或弱 Server Fallback |
| SDR-05..SDR-07 | Workspace/Service/Runtime/Cache/Log/Secret/Temp 分离，默认拒绝隐式传输并保留 Workspace | 清理、同步与用户数据生命周期解耦 |
| SDR-08..SDR-10 | Export/Purge 覆盖派生副本；Backup 角色正确且经 Restore；故障关闭 | 无虚假删除、恢复或 Cloud Fallback |
| SDR-11..SDR-12 | Telemetry 内容安全；四仓 Windows/macOS/Linux + Network/Residue Evidence | 静态合同不能冒充商业声明 |

### 待决策：REQ-2026-0020（P0 最小可批范围，建议最先评审）

功能走查（REVIEW-20260922 §4）与 2026-09-23 对齐评审共同指向：这是唯一被仓库自身的 Phase 表标注「立即可执行」、且范围是封闭数值的签署包（`REVIEW-20260730-sandbox-lifecycle-history-and-idempotency-retention`，`Risk: high`，受 `sandbox-lifecycle-history-and-idempotency.contract.json` 具名约束）。评审者需要给出的决策清单（数值不得由实现者猜测——REQ Goals 第 4 条）：

| # | 决策点 | 现状锚点 | 批准 Owner |
| --- | --- | --- | --- |
| D1 | 最大 Session Operation 数 | 代码临时上界 `10_000`（`repository-sqlx` 读窗口失败关闭；写路径不设上界为已登记事实） | Reliability + Product |
| D2 | 最大活动 Session 生命周期 | 未定义 | Product + Operations |
| D3 | 终态幂等保留窗口 | 未定义 | Product + Security/Privacy |
| D4 | 窗口结束后的 Late Retry Typed Outcome | 未定义；边界禁令：记录缺失不得静默当新请求执行 | API/Kernel Owner |
| D5 | 热状态/账本物理命名 | 领域名候选 `SandboxSessionHotState` / `SandboxLifecycleIdempotencyRecord`；物理表名未定 | Database Owner |
| D6 | `MIG-*` 迁移策略与真实证据 | 六段式 expand→backfill→verify→dual-read→cutover→retire；禁止原地重写 `0001` | Database Owner |
| D7 | 公开 Error 与 Kernel 映射 | 未定义 | API/Kernel Owner |

批准即解锁下方 Phase 0.5（bounded Hot State + point-lookup Idempotency Ledger 的 expand/backfill/verify/cutover 实施）；在此之前 `MAX_SANDBOX_SESSION_OPERATIONS` 行为被 REQ-2026-0005 边界段冻结，不得删除/截断/过期任何幂等记录。

## 评审退出后立即可执行的工作项

### Phase 0.5: Bounded Lifecycle Persistence
批准 REQ-2026-0020 的最大 Operation 数、最大活动 Session 生命周期、终态保留、Late Retry、Repository 命名与 `MIG-*` 后，以 expand/backfill/verify/cutover 方式将当前有界历史 hydrate（读取上限 `MAX_SANDBOX_SESSION_OPERATIONS`，超限失败关闭）收敛为 bounded Hot State + point-lookup Idempotency Ledger；真实 PostgreSQL 证据完成前不改变现有持久化行为。

### Phase 1: Command Execution Port
在 REQ-2026-0021 事务边界中创建经过批准的 Workspace Runtime Transaction/Checkpoint 组合 Port，并在 `sdkwork-sandbox-provider-spi` 创建 `SandboxCommandExecutor` Port 与公共类型；各窄 Port 保持独立所有权。

### Phase 2: Local Provider
严格按 Local Host Boundary 实现：Workspace/Runtime opened Capability Handle、handle-relative Filesystem；Windows suspended Job Object + Completion Port 后 Resume；Linux 用户代码前 race-free delegated cgroup v2 membership；macOS 保持 Terminal denial，直到独立机制和证据获批。Environment 从空集合按 allowlist 构造，Timeout/Output/PID/Cancellation/Fencing 有界，Cleanup 不确定时 Quarantine。任何 Local 数据声明还必须独立通过 REQ-2026-0022 的四仓 Store/Transfer/Backup/Restore/Purge/OS Evidence，不能由 Provider Ready 推导。

### Phase 3: Firecracker Provider
KVM Preflight、Artifact 校验、Host Isolation Broker、Workspace/Network/Resource 实现。

### Phase 4: Service Host + CLI
L5 Typed Composition + Readiness，Operator CLI 命令。

### Phase 5: Cloud Scheduling + Runtime Pool
完成 Node Trust、Admission/Scheduler/Capacity、PostgreSQL Authority 后，按 REQ-2026-0019 先实现 tenant-neutral `PreparedSlot`，并通过 REQ-2026-0021 的 Revision -> Claim -> Attachment -> Command -> Durable Checkpoint -> Sanitization -> Release 全链路；再以独立真实 KVM 证据评审 `WarmMicroVmSlot`。

## 已完成的人工评审

| Review | 状态 |
| --- | --- |
| REVIEW-20260728-sandbox-foundation-verification | accepted |
| REVIEW-20260728-sandbox-lifecycle-core-verification | conditional-pass |
| REVIEW-20260728-sandbox-postgresql-persistence-verification | conditional-pass |
| REVIEW-20260728-sandbox-workspace-attachment-boundary-verification | conditional-pass |
| REVIEW-20260729-sandbox-provider-allocation-key-rotation-verification | conditional-pass |
