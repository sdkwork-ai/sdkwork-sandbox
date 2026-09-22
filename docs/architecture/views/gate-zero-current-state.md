# Gate 0 Current State View

Status: active

Owner: SDKWork Runtime Platform

Updated: 2026-09-22

## 目的

本视图记录 Gate 0 阶段已物化与未物化的组件，供架构/安全评审时对照。

## 已物化 (Implemented)

```mermaid
flowchart LR
    subgraph L3 Domain
        SPI[sdkwork-sandbox-provider-spi<br/>Provider Port<br/>SandboxProvider]
        SVC[sdkwork-intelligence-sandbox-service<br/>Lifecycle Service<br/>SandboxSession + Lease/Fencing]
    end

    subgraph L4 Adapter
        MEM[sdkwork-intelligence-sandbox-repository-memory<br/>InMemory Repository<br/>Test-only]
        SQL[sdkwork-intelligence-sandbox-repository-sqlx<br/>PostgreSQL Repository<br/>Candidate verified]
        LOCAL[sdkwork-sandbox-provider-local<br/>Local Provider<br/>Fake Host Boundary only]
    end

    subgraph L6 Delivery
        HOST[sdkwork-sandbox-service-host<br/>Service Host<br/>Not activated]
        CLI[sdkwork-sandbox-cli<br/>CLI<br/>Not activated]
    end

    SPI --> SVC
    SVC --> MEM
    SVC --> SQL
    SPI --> LOCAL
    HOST --> SVC
    HOST --> SPI
    CLI --> HOST
```

## 未物化 (Deferred Until Gate 0 Exit)

```mermaid
flowchart LR
    subgraph Deferred
        CMD[SandboxCommandExecutor Port]
        FIRE[sdkwork-sandbox-provider-firecracker<br/>Firecracker Provider]
        HOST_BROKER[Host Isolation Broker]
        NET[Network Policy/Isolation]
        RES[Resource Policy/Usage]
        SCHED[Admission/Scheduler/Capacity]
        NODE[Node Trust/Attestation/Inventory]
        OBS[Observability Runtime]
        QUOTA[Quota/Capacity Persistence]
        POOL[Runtime Pool/Fast Allocation]
        TX[Workspace Runtime Transaction/Checkpoint]
        DATA[Standalone Data Residency/Recovery]
    end
```

## 组件状态矩阵

| 组件 | Crate | 状态 | 门禁证据 |
| --- | --- | --- | --- |
| Provider SPI | `sdkwork-sandbox-provider-spi` | active | `SandboxProvider` Port + Identity Types |
| Lifecycle Service | `sdkwork-intelligence-sandbox-service` | active | 26 tests, Lease/Fencing/Readiness/Idempotency |
| Memory Repository | `sdkwork-intelligence-sandbox-repository-memory` | active (test-only) | 4 tests |
| PostgreSQL Repository | `sdkwork-intelligence-sandbox-repository-sqlx` | candidate | 6 tests + live PG evidence |
| Local Provider | `sdkwork-sandbox-provider-local` | gate-0 | 5 Fake Host Boundary tests |
| Service Host | `sdkwork-sandbox-service-host` | inactive | 21-test Bootstrap/Profile/Capability Gate 0 contracts only; no wiring |
| CLI | `sdkwork-sandbox-cli` | inactive | Stub only |

## 门禁契约状态

| 契约 | 路径 | 状态 |
| --- | --- | --- |
| Provider Delivery Gates | `specs/sandbox-provider-delivery-gates.contract.json` | draft, implementationAuthorized: false |
| Local Host Boundary | `specs/sandbox-local-provider-host-boundary.contract.json` | draft, implementationAuthorized: false |
| Command Contract | `apis/commands/sandbox-command-contract.json` | draft |
| Service Host Composition | `crates/sdkwork-sandbox-service-host/specs/sandbox-service-host-composition.contract.json` | draft, implementationAuthorized: false; all referenced Profile/Capability dependencies closed |
| Firecracker Artifact | `specs/sandbox-firecracker-artifact-compatibility.contract.json` | draft |
| Network Isolation | `specs/sandbox-firecracker-network-isolation.contract.json` | draft |
| Resource Isolation | `specs/sandbox-firecracker-resource-isolation.contract.json` | draft |
| Host Isolation Broker | `specs/sandbox-host-isolation-broker.contract.json` | draft |
| Multi-tenant Scheduling | `specs/sandbox-multi-tenant-scheduling.contract.json` | draft |
| Node Trust | `specs/sandbox-node-trust-and-inventory.contract.json` | draft |
| Quota Persistence | `specs/sandbox-quota-and-capacity-persistence.contract.json` | draft |
| Runtime Pool | `specs/sandbox-runtime-pool.contract.json` | draft |
| Lifecycle History/Idempotency | `specs/sandbox-lifecycle-history-and-idempotency.contract.json` | draft, implementationAuthorized: false |
| Workspace Runtime Transaction | `specs/sandbox-workspace-runtime-transaction.contract.json` | draft, implementationAuthorized: false |
| Standalone Data Residency | `specs/sandbox-standalone-data-residency.contract.json` | draft, implementationAuthorized: false; Local-only release evidence gate |

## 需求状态

| REQ | 标题 | 状态 |
| --- | --- | --- |
| REQ-2026-0001 | Foundation | accepted |
| REQ-2026-0002 | Lifecycle Core | accepted |
| REQ-2026-0003 | Local Provider | draft |
| REQ-2026-0004 | Agents Workspace Attachment | accepted |
| REQ-2026-0005 | PostgreSQL Repository | accepted |
| REQ-2026-0006 | Key Rotation | accepted |
| REQ-2026-0007 | Command Execution | draft |
| REQ-2026-0008 | Firecracker Provider | draft |
| REQ-2026-0009 | Service Host | draft |
| REQ-2026-0010 | Observability | draft |
| REQ-2026-0011 | Host Isolation Broker | draft |
| REQ-2026-0012 | Firecracker Artifact | draft |
| REQ-2026-0013 | Workspace Block Device | draft |
| REQ-2026-0014 | Network Isolation | draft |
| REQ-2026-0015 | Resource Isolation | draft |
| REQ-2026-0016 | Multi-tenant Admission | draft |
| REQ-2026-0017 | Node Trust | draft |
| REQ-2026-0018 | Quota Persistence | draft |
| REQ-2026-0019 | Runtime Pool And Fast Allocation | draft |
| REQ-2026-0020 | Lifecycle Hot State And Idempotency Retention | draft |
| REQ-2026-0021 | Workspace Runtime Transaction And Checkpoint | draft |
| REQ-2026-0022 | Standalone Data Residency And Recovery | draft |
| REQ-2026-0023 | Internal Control Plane | draft |
| REQ-2026-0024 | Interactive Terminal Session | draft |
| REQ-2026-0025 | Runtime Secret Projection | draft |
| REQ-2026-0026 | Cloud Data Residency And Recovery | draft |
| REQ-2026-0027 | Cross-Repository Version Compatibility And Release Set | draft |

## 验证门禁

```bash
node tools/check-sandbox-cargo-path-dependencies.mjs
node tools/check-sandbox-workspace-dependency-inheritance.mjs
node tools/check-sandbox-doc-integrity.mjs
node tools/check-sandbox-component-contract-alignment.mjs
node tools/check-sandbox-requirement-traceability.mjs
node tools/check-sandbox-e2b-parity-matrix.mjs
node ../sdkwork-specs/tools/check-workspace-path-portability.mjs --root .
node ../sdkwork-specs/tools/check-shell-portability.mjs --root .
node tools/check-sandbox-platform-code.mjs
node tools/check-sandbox-database-contract-reproducibility.mjs
cargo fmt --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
node --test tests/contract/*.test.mjs
node ../sdkwork-specs/tools/check-repository-docs-standard.mjs --root .
node ../sdkwork-specs/tools/check-workspace-packages-layout.mjs --root . --mode enforce
node ../sdkwork-specs/tools/check-component-port-bindings.mjs --root . --strict
node ../sdkwork-specs/tools/check-application-layering.mjs --root .
node ../sdkwork-specs/tools/check-identity-naming.mjs --root .
node ../sdkwork-specs/tools/audit-repository-baseline.mjs --root .
node ../sdkwork-specs/tools/check-database-framework-standard.mjs --root .
node tools/check-sandbox-evidence-traceability.mjs
node tools/check-sandbox-human-review-signoff.mjs
```

格式门禁使用 `cargo fmt --check`；禁止 `cargo fmt --all -- --check`，因为 `--all` 的语义包含本地路径依赖，会把兄弟仓库的格式偏差算到本仓门禁上。`check-sandbox-cargo-path-dependencies.mjs` 必须最先运行：清单里多余的 `..` 会让 `cargo metadata` 失败，导致其后所有 cargo 命令无法运行。`check-component-port-bindings.mjs --strict` 要求每个声明了自有源码的组件 spec 声明 `contracts.layerRole`（`APPLICATION_LAYERED_ARCHITECTURE_SPEC.md` 第 2 节对新的可组合模块是 MUST）：本仓 8 个 crate 级 spec 都正确声明，唯独 2026-08-30 加入的仓根 `specs/component.spec.json` 漏了，于是这条命令在文档里被写了四处、却一直在红；已补 `runtime-composition` 并把该命令接进 `_sdkwork:check`，否则同一个洞会再次漏进来。`check-sandbox-doc-integrity.mjs` 覆盖规范门禁不管的两件事：相对 Markdown 链接可解析、live 文档围栏代码块里的命令可执行（`docs/changelogs/`、`docs/engineering/reviews/`、`docs/releases/`、`docs/archive/` 属时点证据，豁免命令规则）。`check-sandbox-component-contract-alignment.mjs` 从 `COMPONENT_SPEC.md` 散文派生规则（不复制清单），校验声明语言必须有对应源码、有源码必须引用语言与代码风格 spec、`rust-api-assembly` 必须引用其 MUST 句点名的全部 spec 并声明 `component.surface: "api-assembly"`。`check-sandbox-database-contract-reproducibility.mjs` 用临时副本重跑注册的 `db:materialize:contract`，比对 `database/contract/` 三个生成物与提交内容是否逐字节一致；生成器不认识的字段会被静默删除，因此不可复现的注册表意味着运维每次重生成都会丢数据。`check-sandbox-requirement-traceability.mjs` 落实 `DOCUMENTATION_SPEC.md` 第 28 节的追溯链与 `REQUIREMENTS_SPEC.md` 第 6 节"被引用的需求 id 必须解析到真实记录"：live 文档里的 `REQ-*`/`ADR-*` token 必须解析到本仓记录或被同行兄弟仓路径限定；本仓需求与决策记录必须被自身以外的 live 文档引用；`PRD-capabilities.md` 第 11 节每行必须被归类（承载 `REQ-*` / 标 `无` / `同上` 继承），并打印能力普查数字。裸写跨仓 id 是该门禁存在的原因：交付计划把 `sdkwork-kernel/docs/product/requirements/REQ-2026-0002-distributed-execution-placement-control-plane.md` 与 `sdkwork-birdcoder/docs/product/requirements/REQ-2026-0006-hybrid-local-cloud-agent-execution.md` 写成裸 `REQ-2026-0002` 与 `REQ-2026-0006`，而这两个号同时也是本仓的 Lifecycle Core 与 Key Rotation 记录，于是引用表面可解析、实际指向错误记录。

`check-sandbox-e2b-parity-matrix.mjs` 是唯一读 `docs/architecture/tech/TECH-e2b-capability-parity.md` 的门禁：四级状态标记必须与声明词表一致、矩阵行必须 `1..N` 连续且五格齐备、普查分类必须与矩阵小节按序一一对应、逐分类计数与合计行与重算总量必须三方一致。合计行含 Markdown 强调（`| **合计** | **78** | … |`），不剥离星号就会被解析成「没有合计行」，于是全部合计断言被静默跳过、门禁把**重算**值当成**已核对**值打印出来——这是本门禁自己踩到的静默漏洞，已修并锁进契约测试。同一次运行还解析文档里全部 `REQ-*`/`ADR-*` token，并要求该文档被 `TECH_ARCHITECTURE.md` 与 tech README 链接、在 `docs/INDEX.yaml` 登记。当前读数：17 分类 / 78 行 / `✅ 0 | 🟡 16 | ❌ 60 | ⛔ 2`。

`check-sandbox-platform-code.mjs` 是唯一读 `docs/architecture/tech/TECH-platform-support.md` 的门禁，补的是上游两条可移植性门禁留下的那个不变式：`check-workspace-path-portability.mjs` 管机器绝对路径、`check-shell-portability.mjs` 管 shell 语法，**两者都不看 `#[cfg(windows)]`、`std::process::Command`、`libc::` 或 `std::path::MAIN_SEPARATOR`**，因此某个运行后端可以变成只支持单一平台而全部门禁仍然全绿。它同时补掉了自己的接线洞：2026-09-22 之前这两条上游门禁对本仓是**通过但从未执行**（都不在 `package.json` 里），`DEPENDENCY_MANAGEMENT_SPEC.md` 第 1 节的跨平台要求长期没有执行者。门禁要求：矩阵每行必须引一条可解析的仓内路径或探测词表里的能力 id（词表是 import 进来的，不是抄一遍）、`crates/*/src` 里每个平台标记都必须在文档中声明平台与理由并与源码双向一致、文档点名的每条门禁必须在 `package.json` 里真的调用同一个脚本。门禁在真实文档上跑出过两个解析缺陷并已锁进契约测试：反引号内的 `|`（`namespace.mount|pid|uts`）会切断表格行，使证据格提前结束、其后的引证（含一条不存在的路径）完全不被检查；空声明表的占位行 `| （无） | | | |` 被当成声明读入，于是「当前声明 0」反而报出三条错误。当前读数：5 个平台（`windows-x64` unsupported、`linux-x64-wsl2` partial、`linux-x64-native`/`linux-aarch64` unmeasured、`macos-arm64` unsupported）、`crates/*/src` 平台条件标记 0 处。实测量见 `docs/architecture/tech/TECH-performance-baseline.md`。

上述 Phase 0 Repository Baseline 在 2026-07-30 通过。Service Host 现要求 18 个 Gate 依赖，其中 Workspace Runtime Transaction 是 Local/Cloud 公共关闭失败依赖，Standalone Data Residency/Recovery 只适用于 `sandbox_standalone_local` 并在 Firecracker Profiles 中禁止；聚焦测试还覆盖 Local/Cold/Pool 分离、Revision/Checkpoint 顺序、Command/Terminal 条件门禁，以及 11 类 Local 数据、数据库角色、Capability 分离、无隐式传输、Backup/Restore 和 Purge。完整验证数字以 PLAN-2026-0002 当前 Checkpoint 为准。Provider、Service Host、Cloud、Pool、Local Data Claim 与商业 Release Gate 仍因 `implementationAuthorized: false` 和待人工评审保持关闭；不得把 Baseline PASS 解释为运行时或发布就绪。
