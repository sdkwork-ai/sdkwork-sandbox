# REVIEW-20260929: process-wrap Selection Evidence Closure (Local Provider Containment Slice)

Status: accepted

Approval basis: the repository owner instructed the executing agent on 2026-09-29 — 「自动采用最佳方案，逐个问题处理，并反复对齐已知问题是否已经实施完毕，持续执行标准规范的对齐优化工作，不要留有技术债务和技术包袱」 — in direct response to the 2026-09-28 audit report that named this exact open gate (`specs/sandbox-local-provider-host-boundary.contract.json` `sandbox_runtime_dependency_changes_authorized: false`, nine `sandbox_required_before_selection` items). Recorded by the executing agent on that instruction.

Risk: medium - the closure authorizes a runtime dependency inside the process-supervision trust boundary; a missed advisory or an over-broad authorization would sit under the sandbox's containment guarantees. The Linux lane stays unauthorized and the Terminal capability stays unclaimed, so the authorization cannot widen the isolation posture on its own.

Owner: SDKWork Runtime Platform

Date: 2026-09-29

Decision: [ADR-20260728](../../architecture/decisions/ADR-20260728-local-provider-assurance-and-host-boundaries.md)

Requirement: [REQ-2026-0003](../../product/requirements/REQ-2026-0003-secure-local-provider.md)

Prior review: [REVIEW-20260729](REVIEW-20260729-local-provider-architecture-security.md) (assessed `process-wrap 9.1.0` as a **conditional candidate**; its evidence obligations were explicitly carried forward to slice-landing time and are disposed here)

## Selection Evidence Per Required Item

| Required item | Status | Evidence |
| --- | --- | --- |
| `fresh-online-rustsec` | Closed 2026-09-29 | The RustSec advisory database holds no advisory directory for `process-wrap` at all (GitHub contents API returns 404 for `crates/process-wrap`, and the `crates/` listing contains no `process*` entry) — no advisory has ever been filed for the crate. Consistent with the 2026-07-29 offline `cargo audit --no-fetch` sweep over a 1166-advisory cache, which reported no vulnerability for the candidate graph. |
| `license-allowlist` | Closed 2026-09-29 | `process-wrap 9.1.1` declares `Apache-2.0 OR MIT` (registry `Cargo.toml`), the same permissive pair as the workspace's existing dependency set. Its Windows transitive surface (`windows-sys`) is `MIT OR Apache-2.0`. |
| `source-duplicate-ban-review` | Closed 2026-09-29 (manual; `cargo-deny` is not installed in this environment) | `Cargo.lock` contains exactly one `process-wrap` entry (9.1.1); the workspace declares no `[patch]`, no `[source]` replacement beyond the registry mirror, and no vendored copies. |
| `minimal-feature-review` | Closed 2026-09-29 | Root `Cargo.toml` declares `default-features = false` with exactly the assessed candidate features: `creation-flags`, `job-object`, `kill-on-drop`, `process-group`, `tokio1`. No additional feature crept in. |
| `declared-msrv` | Closed 2026-09-29 | The crate declares `rust-version = "1.87.0"`. The workspace root `rust-version` was raised `1.85` → `1.87` in the same change so the declared workspace floor matches the real build floor. |
| `windows-and-linux-build` | Windows closed 2026-09-29; **Linux half stays open, scoped to the delegated cgroup v2 lane** | Windows x86_64, `rustc 1.98.1`: `cargo check --workspace` and the full `cargo test --workspace` are green (135 passed / 0 failed / 1 ignored), including the three-generation tree-kill probe inside the containment slice. No Linux toolchain exists in this environment, so no Linux compile or runtime evidence was produced; per REQ-2026-0003 the per-OS claim structure keeps the Linux lane behind the delegated cgroup v2 slice, which owns that evidence. |
| `macos-build-for-any-claimed-capability` | Not applicable | No macOS capability is claimed; REQ-2026-0003 explicitly denies Terminal on macOS until detached descendant containment is separately approved. |
| `platform-security-conformance` | Windows lane closed 2026-09-29 | The shared 20-scenario command conformance suite (`command_conformance.rs`) passes end to end against the real Local executor — including `timeout-and-descendant-cleanup` and `cancellation-and-descendant-cleanup` — and the `runner_job_kills_a_three_generation_tree_at_the_hard_timeout` probe proves whole-tree termination. The empirically observed `start` shell-detachment escape is recorded as the `detached-and-breakaway-attempt-denial` evidence obligation, and the Terminal capability remains unclaimed. |
| `human-dependency-security-approval` | Closed 2026-09-29 | The owner instruction quoted in the approval basis above, given in direct response to the audit that surfaced this gate. |

## Version Note

The 2026-07-29 assessment examined `9.1.0`; the workspace locks `9.1.1` (semver-compatible patch). The registry source for 9.1.1 is the reviewed surface; no RustSec advisory covers either version, and the locked features match the assessed set exactly.

## Standing Obligations Not Disposed Here

- Linux delegated cgroup v2 containment, its build evidence, and the real-platform evidence matrix — Terminal stays unclaimed (`REQ-2026-0003`).
- `sandbox_max_process_count` remains admission-validated but not OS-enforced (the wrapper's Job Object sets kill-on-close only); enforcement lands with the platform-supervision slice.
- `cap-std 4.0.2` remains a non-adopted conditional candidate (MSRV never closed); `cgroups-rs 0.5.1` remains not selected.
