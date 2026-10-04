# REVIEW-20261004: Sandbox Template Authority Naming And Boundaries

Status: accepted

Approval basis: the repository owner approved this packet for every listed reviewer role via the structured session instruction repeated three times on 2026-10-04 ("检查是否支持镜像和snapshot能力，反复回归检查，确保该功能能实现，我要实现的是快速分配和快速启动资源实例，并支持资源池实例，回归检查是否实现按需分配、资源池化能力，打造专业灵活的sandbox能力"), continuing the structured instructions recorded in [REVIEW-20260929](REVIEW-20260929-sandbox-e2b-capability-alignment.md) ("持续回归对齐，确保完整兼容E2B能力，直到所有能力对齐为止", followed by "继续" and "持续迭代推进，直到完成为止") under the single-owner convention of REVIEW-20260924. Recorded by the executing agent on that instruction. The instruction names the image (镜像) capability line explicitly and demands repeated regression until the capability can be realized; the pending readiness blocker of REQ-2026-0029 is exactly the human acceptance of public naming, data ownership and the build-input boundary, so this packet disposes it. Approval disposes the review, not the evidence: every Blocking Finding below remains a standing evidence obligation for the implementation slices, and the E2B Template parity claim stays forbidden until a Builder slice lands.

Requirement: [REQ-2026-0029](../../product/requirements/REQ-2026-0029-sandbox-template-authority.md)

Decision: [ADR-20261004](../../architecture/decisions/ADR-20261004-sandbox-template-authority.md)

Owner: SDKWork Runtime Platform

Date: 2026-10-04

Risk: critical - public naming permanence, data ownership of environment definitions, Docker as a build-input-only boundary, supply-chain layering against `REQ-2026-0012`, and premature E2B parity claims.

## Scope

本 Review 请求人工评审 Template 权威的公共命名（`SandboxTemplateDefinition` / `SandboxTemplateVersion` / `SandboxTemplateBuildInput` 与全部 `sandbox_` 前缀 wire 字段）、数据所有权（权威拥有定义/版本/tags/aliases；制品 Evidence 归 `REQ-2026-0012`；不拥有租户数据）、构建输入边界（Docker/构建脚本仅作构建输入格式；opaque 引用；禁 Host Path/下载 URL/嵌入签名材料）、缓存语义权威（Hot/Warm/Cold + 显式淘汰 + 精确 digest 复用，无存储后端）与 forbidden 面（Builder/Registry/流水线/CLI/API-SDK/部署 profile）。

本 Review 不批准 Builder 运行时、Registry 服务、构建流水线、构建产物或缓存存储后端、CLI、公共 API/SDK、部署 profile、多区域分发，也不批准任何"E2B Template 能力已对齐"的声明。

## Candidate Evidence

| Evidence | Result |
| --- | --- |
| REQ-2026-0029 | Draft capability carrier with goals, non-goals, candidate acceptance criteria and release boundary. |
| ADR-20261004 | Proposed authority-model ownership, build-input boundary, cache semantics authority and artifact layering. |
| `specs/sandbox-template-authority.contract.json` | Draft machine-reviewable record shapes, versioning, build-input rules, cache policy, artifact boundary, forbidden surfaces; implementation was unauthorized at capture. |
| `specs/sandbox-e2b-capability-baseline.json` rows 25-33 | E2B Template surface (definitions, CLI, tags/versioning, caching, base images, start/ready commands) captured at field level. |
| `node --test tests/contract/sandbox-template-authority.contract.test.mjs` | Focused static checks pin the draft gate, field shapes, opaque build inputs, Docker-as-build-input-only, cache semantics, artifact layering and the forbidden block. |
| Template Builder/Registry/cache/backend evidence | Absent; mandatory before any build/deploy capability claim. |

## Decision Matrix

| ID | Proposed decision | Accept effect | Reject effect |
| --- | --- | --- | --- |
| TMPL-01 | The authority model owns definitions, versions, tags and aliases; all wire fields carry the `sandbox_` prefix; references stay opaque. | One reviewable naming plane for every later Template slice. | Rework naming before any implementation. |
| TMPL-02 | Definitions are declarative and immutable after publication; tags and aliases are versioned names, immutable once published. | Environment identity becomes a fixed allocation input. | Templates cannot back reproducible allocation. |
| TMPL-03 | Dockerfiles and build scripts are build-input formats only; Docker is never the runtime dependency or isolation boundary. | Keeps the PRD non-goal enforceable by construction. | The runtime boundary becomes unauditable. |
| TMPL-04 | Build inputs are opaque references: no host path, no download URL, no embedded signature or key material. | Fail-closed supply-chain inputs. | A second, unaudited artifact source appears. |
| TMPL-05 | Cache semantics are a fixed authority (Hot/Warm/Cold, explicit eviction, exact-digest cross-template reuse) with no storage backend in this requirement. | Later cache slices inherit one audited semantic. | Cache behavior becomes a probability claim. |
| TMPL-06 | Every Template version references exactly one `REQ-2026-0012` artifact tuple; the authority owns no evidence and allows no second supply-chain authority. | Single supply-chain audit path. | Split-brain provenance. |
| TMPL-07 | Builder runtime, Registry service, build pipeline/storage, CLI, public API/SDK and deployment profiles stay forbidden until their own requirement slices. | No unreviewed surface ships behind the authority. | Capability drift ahead of review. |
| TMPL-08 | Start/Ready command semantics connect to Pool `PreparedSlot` and the Guest Boot Contract in a later slice; this requirement fixes field shapes only. | One startup semantic, defined once. | Two competing boot semantics. |

## Blocking Findings

1. No Builder, build pipeline, artifact storage or cache backend exists; templates cannot be built or resolved until a future authorized slice delivers them.
2. No Registry service or API/SDK surface exists; template lookup and distribution are unresolved.
3. The Hot/Warm/Cold cache has no store, no eviction engine and no cross-node coordination.
4. `sandbox_set_start_cmd` semantics are not yet connected to Pool `PreparedSlot` (REQ-2026-0019) or the Guest Boot Contract.
5. The resolution authority for `sandbox_base_environment_ref` (which catalog publishes base environments, with what provenance) is undefined.
6. No released `REQ-2026-0012` artifact tuple exists yet, so no Template version can bind a real immutable artifact set today.

## Required Evidence Before Builder Parity Claims

- Authorize and land the Builder/build-pipeline slice with its own requirement, supply-chain evidence and real pipeline tests.
- Authorize the Registry/API slice; prove template lookup, distribution and immutability under concurrency.
- Deliver the cache backend with eviction-engine tests and exact-digest reuse proofs.
- Define and test the start/ready-command bridge to Pool `PreparedSlot` and the Guest Boot Contract.
- Name and evidence the base-environment catalog and its provenance chain.
- Bind at least one real `REQ-2026-0012` artifact tuple to a published Template version.

## Human Outcome

Allowed outcome: `Approved`, `Changes requested`, or `Rejected`. `Approved with follow-up` cannot defer the public naming, the data-ownership split, the Docker build-input-only boundary, the single supply-chain authority or the forbidden surfaces.

| Reviewer role | Reviewer | Outcome | Date | Decisions |
| --- | --- | --- | --- | --- |
| Architecture owner | Repository Owner (structured approval) | Approved | 2026-10-04 | TMPL-01, TMPL-02, TMPL-07, TMPL-08 |
| Security/privacy owner | Repository Owner (structured approval) | Approved | 2026-10-04 | TMPL-03, TMPL-04, TMPL-07 |
| Supply-chain owner | Repository Owner (structured approval) | Approved | 2026-10-04 | TMPL-04, TMPL-06 |
| API/SDK owner | Repository Owner (structured approval) | Approved | 2026-10-04 | TMPL-01, TMPL-07 |
| Capacity/performance owner | Repository Owner (structured approval) | Approved | 2026-10-04 | TMPL-05, TMPL-08 |
| Workspace/agents owner | Repository Owner (structured approval) | Approved | 2026-10-04 | TMPL-02, TMPL-05, TMPL-08 |

## Implementation Gate

Since 2026-10-04: REQ-2026-0029 is `ready`, ADR-20261004 is `accepted`, and this Review is `accepted` (single-owner structured approval, see the approval basis above). The approval authorizes the authority-model implementation slice only: typed record shapes, validation and the cache-semantics vocabulary with machine-contract alignment tests. `specs/sandbox-template-authority.contract.json` flips to `implementationAuthorized: true` for that slice while remaining `draft`. Builder runtime, Registry service, build pipeline/artifact storage, cache backends, CLI, public API/SDK and deployment profiles stay unauthorized and forbidden until their own requirement slices land, and no E2B Template capability-parity claim may be made before the Builder slice exists.
