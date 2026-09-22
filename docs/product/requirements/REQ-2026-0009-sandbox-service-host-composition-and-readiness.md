---
id: REQ-2026-0009
title: Define the typed Sandbox Service Host composition and readiness boundary
owner: SDKWork Runtime Platform
status: draft
priority: high
source: platform
problem: The Sandbox repository has lifecycle and persistence candidate ports but no explicit L5 composition boundary that constructs them with typed configuration, safe dependencies, readiness, and shutdown semantics.
goals:
  - Define one provider-neutral Sandbox Service Host composition boundary for standalone and cloud profiles.
  - Make safe configuration, preopened runtime-directory capabilities, PostgreSQL repository authority, Secret/KMS, Workspace Attachment, Provider Registry, and Telemetry dependencies explicit and injected.
  - Bind Service Host readiness to the approved Local, Command, Firecracker, Cloud scheduling, Node Trust, capacity persistence, lifecycle retention, and optional Runtime Pool contracts that each selected profile or capability actually requires.
  - Fail closed when required dependencies or capability/assurance guarantees are unavailable, without silently selecting weaker infrastructure.
  - Produce safe readiness and health observations without exposing secrets, physical paths, provider-private allocation data, or raw command data.
non_goals:
  - Implement a Local, Firecracker, Docker, or other Sandbox Provider.
  - Add an HTTP/RPC listener, internal API, generated SDK, Scheduler, Pool, Quota/Metering, Snapshot, or public Operator API.
  - Read environment variables, `.env` files, source checkout metadata, or Secret Material from L2/L3 code.
  - Create database pools, run migrations, own PostgreSQL schema, or use Memory Repository as a server fallback.
  - Create `sdkwork.app.config.json` or a deployable profile before packaging/deployment is explicitly in scope.
users:
  - SDKWork Runtime Platform maintainers
  - Sandbox Service Host and release operators
  - SDKWork Kernel integrators
  - Sandbox Provider authors
affected_surfaces:
  - rust-components
  - composition
  - config
  - observability
  - security
  - reliability
---

# REQ-2026-0009: Sandbox Service Host Composition 与 Readiness 边界

## Readiness Blockers

本需求在以下边界完成人工评审前保持 `draft`：

- 接受 `SandboxProvider`、REQ-2026-0013 provider-neutral `SandboxWorkspaceAttachmentPort`/L4 mechanism 边界、Kernel 集成和公共 `Sandbox*` 命名 ADR。
- 接受 standalone/cloud 的八个精确 source profile、typed deployment profile/environment/runtime target，以及未来 `etc/` materialization 归属。
- 接受 Runtime Directory Capability、Secret/KMS、Telemetry、外部 Database Composition 与 Workspace Capability 的注入边界及其 owner。
- 确认 Service Host、Standalone Gateway、Internal API、Scheduler、Operator 和部署发布的层级所有权；Service Host 不拥有 HTTP Listener 或业务 Policy。
- 接受适用于所选 Profile/Capability 的依赖契约；任何缺失、`draft`、待人工评审或未授权依赖都必须使对应 Profile/Capability 保持 Not Ready。

## Candidate Acceptance Criteria

- `sdkwork-sandbox-service-host` 作为唯一 L5 `runtime-service-host` Composition Root，使用注入的 Repository/Provider/Workspace/Runtime Directory/Secret/Telemetry/Clock/ID 依赖构造并提供 `SandboxSessionLifecyclePort`；Lifecycle Service 不作为另一个预构造输入注入。Host 不拥有 L2 业务规则、L4 SQL/Provider Mechanism 或 L1 HTTP 适配。
- Host 接收 Runtime Bootstrap 归一化后的 typed `SandboxServiceHostConfig`；八个候选 source profile 精确覆盖 `standalone|cloud` x `development|test|staging|production`，未知或跨环境回退关闭失败。安全配置按 source `etc/`、installed operator config、process-environment safe override、CLI safe override 的固定顺序归一化，记录来源并拒绝未知键；Secret Resolution 保持独立。Host/L2/L3 不读取 process environment、`.env`、filesystem registry、global mutable state 或 Secret Material；本 Gate 不物化 `etc/`。
- Profile 明确区分 `deploymentProfile`、`environment` 与 `runtimeTarget`，只允许标准 `standalone`/`cloud` 组合；profile 不匹配、缺少必需依赖或 Secret/KMS/Telemetry 未就绪时启动关闭失败。
- Source `sandbox_profile_id` 只标识 deployment/environment；Execution Profile 由该 deployment profile 与 Service Policy/Provider Registry 已选择的 Provider Kind 共同派生，调用方不得直接指定。只允许 Standalone Local、Standalone Firecracker 与 Cloud Firecracker 三个精确映射，Cloud Local 和未知组合关闭失败。
- 外部 Database Composition 独占 PostgreSQL `DatabasePool`、连接凭据、Migration Authority 与 Database Status，并只向 Host 注入已构造且就绪的 `SandboxSessionRepository`。Host 不接收具体 Pool、不创建连接、不执行迁移、不读取连接 URL，也不提供 SQLite/Memory Server Fallback；Redis 当前明确禁用，启用前必须具备独立 Ready Requirement。
- Runtime Bootstrap 只向 Host 注入最小权限、预打开且身份绑定的 `SandboxRuntimeDirectoryCapabilities`；Host 不接收或推导物理路径，不扩大权限，Capability 缺失、重复、别名或身份/Profile 不匹配均在 Serving 前关闭失败。
- Provider Registry 只通过 `SandboxProvider` 与已接受的 Provider-neutral Ports 注入；Provider Identity、Capability、Isolation Assurance 与 Readiness 不匹配时，Host 不选择弱 Provider、不把 `SandboxSession` 标记为 Running。
- `standalone/local` 必须额外通过 Local Host Boundary；`standalone/firecracker` 必须通过 Host Broker、Artifact、Workspace、Network 与 Resource Gate；`cloud/firecracker` 还必须通过 Node Trust、Admission/Scheduling 和 PostgreSQL Quota/Capacity Gate。公共 Gate 不得从 Host 自身状态推断依赖已就绪。
- Runtime Pool 是 `cloud/firecracker` 的显式可选加速 Overlay，不是 Cold Firecracker 的正确性前提。请求明确要求 Pool 时不得回退 Cold；Pool 非必需且不可用时，只能在完整 Cold Cloud Profile 已 Ready 的前提下回退，且不得降低 Assurance。
- `sandbox_command` 只有在 Provider Descriptor 声明 Capability、`SandboxCommandExecutor` 已绑定且共同 Command/平台 Cleanup Conformance 通过时才 Ready；`sandbox_terminal` 还必须通过认证、后代监督与清理证据。Descriptor 单独不得开启 Capability，macOS Local Terminal 当前必须关闭失败。
- `SandboxServiceHostReadiness` 是 typed、可审计的内部结果，精确区分 Config、Runtime Directory Capability、Store、Provider Registry、Workspace Attachment、Secret/KMS、Telemetry 与 Fencing 八个公共维度；任一必需维度的 `degraded`/`unknown` 都不是 Ready。结果不得包含 Secret Material、Database URL、Physical Host Path、API Socket、Provider Allocation Reference 或 Raw Command。
- Telemetry Adapter 只有在仍能执行有界接收、脱敏和丢弃计数时才 Ready；外部 Exporter Outage 可进入独立运维健康降级状态，但不得关闭 Redaction/Drop Accounting，也不得冒充 Audit/Outbox Authority。Buffer Policy 失效时 Adapter 必须 Not Ready。
- Host/Bootstrap Process Environment、CLI Argument、Log/Metric/Trace/Event/Readiness 不得承载 Secret Value；未来 Guest/Command Secret Injection 必须由独立 Ready Requirement 定义和授权，本需求不预设其机制。
- Liveness/Readiness/Shutdown 不挂载 HTTP 路由；未来 HTTP/RPC 由独立 Internal API/Standalone Gateway Requirement 定义，Host 只提供可组合的内部 Port/Observation。
- Host Shutdown 在有界 Deadline 内停止新生命周期副作用，释放 Lease/Provider/Telemetry/Store 资源并保持重复调用幂等；超时转为安全的结构化 Internal Failure。
- Standalone 与 Cloud 使用相同 L2/L3 Contract；差异只位于 L5 Composition 的 Infrastructure、Persistence、Cache、Provider Registry 和 Runtime Profile，不允许 Kernel 或 Sandbox Service 分支判断部署拓扑。
- 所有 Host/Readiness/Error/Metric/Audit 关联使用 `sandbox_*` 所有权字段和 Server-owned Trace；禁止输出未脱敏的租户名、Raw ID、路径、命令、环境值或 Provider-private Metadata。

## Candidate Non-functional Requirements

| 领域 | 要求 |
| --- | --- |
| Security | 依赖缺失、Assurance 不足、Secret/KMS 失败、Provider Fencing 不可证明时关闭失败；不允许弱 Provider 回退。 |
| Privacy | Config、Readiness、Log、Metric、Audit 只包含最小安全身份和低基数 Outcome；Secret/Path/Allocation/Command 不进入普通输出。 |
| Performance | Host Bootstrap 按固定顺序执行，不进行无界扫描或同步远程 Secret/KMS 调用；Database、Secret 和 Telemetry 使用已评审的有界预算，Readiness 检查有超时且可重复。 |
| Reliability | 初始化、Readiness、Shutdown、Provider Outage 与 Store Unavailable 有明确状态和重试/不可重试分类；多副本协调继续由后续 Scheduler/Lease Requirement 负责。 |
| Operations | Health/Readiness、Trace、Metric、Audit 的 Port Owner、Retention、Redaction 和证据位置必须在实现前固定。 |

## Trace

Specs: `REQUIREMENTS_SPEC.md`, `ARCHITECTURE_DECISION_SPEC.md`, `APPLICATION_LAYERED_ARCHITECTURE_SPEC.md`, `SOURCE_CONFIG_SPEC.md`, `CONFIG_SPEC.md`, `ENVIRONMENT_SPEC.md`, `DEPLOYMENT_SPEC.md`, `OBSERVABILITY_SPEC.md`, `SECURITY_SPEC.md`, `RUNTIME_DIRECTORY_SPEC.md`, `COMPONENT_SPEC.md`, `RUST_CODE_SPEC.md`, `TEST_SPEC.md`.

Components: `crates/sdkwork-sandbox-service-host`, `crates/sdkwork-intelligence-sandbox-service`, `crates/sdkwork-intelligence-sandbox-repository-sqlx`, `crates/sdkwork-sandbox-provider-spi`, and future approved Composition Ports.

Decision: [ADR-20260729: Sandbox Service Host Composition And Readiness](../../architecture/decisions/ADR-20260729-sandbox-service-host-composition-and-readiness.md).

## Verification Plan

```bash
cargo fmt --check
cargo check -p sdkwork-sandbox-service-host
cargo test -p sdkwork-sandbox-service-host
cargo clippy -p sdkwork-sandbox-service-host --all-targets -- -D warnings
node ../sdkwork-specs/tools/check-component-port-bindings.mjs --root . --strict
node ../sdkwork-specs/tools/check-application-layering.mjs --root .
node ../sdkwork-specs/tools/check-rust-backend-composition.mjs --root .
node ../sdkwork-specs/tools/check-identity-naming.mjs --root .
node ../sdkwork-specs/tools/check-repository-docs-standard.mjs --root .
```

真实 Local/Firecracker Provider、Secret/KMS、PostgreSQL Multi-replica、Internal API/SDK、Deployment、Observability Backend、Scheduler/Quota 和商业 Release Evidence 是后续 Requirement 的强制证据，不由本需求的 Host Unit Test 替代。

## Current Boundary

2026-07-30 已扩展 Service Host Gate 0 机器契约：18 个跨契约依赖具有可解析路径和统一的 fail-closed 状态规则；独立 Bootstrap 契约固定八个 source profile、安全配置 allowlist、预打开 Runtime Directory Capability、Secret/KMS、外部 Database Composition、Redis 禁用、有界 Telemetry、初始化/逆序清理和责任矩阵。公共 Readiness 为八个维度，并继续拆分 `standalone/local`、`standalone/firecracker`、`cloud/firecracker`、可选 Runtime Pool Overlay，以及 Command/Terminal 条件门禁。Workspace Runtime Transaction 是所有 Lane 的公共依赖；REQ-2026-0022 Standalone Data Residency/Recovery 只属于 `sandbox_standalone_local`，不得传播到 Firecracker Profile。Workspace 仍只通过 provider-neutral `SandboxWorkspaceAttachmentPort` 注入；Profile Gate 是 L5 依赖闭包，不把 L4 机制、L2 Scheduler Policy 或四仓数据权威收归 Host。完整契约套件已通过，精确当前数字记录在 PLAN-2026-0002 Verification Checkpoint；所有引用契约仍为 `draft`、待人工评审或 `implementationAuthorized: false`，Host 仍无 Public Export、Config Key、Runtime Entrypoint、依赖绑定或运行时实现。本需求保持 `draft`。
