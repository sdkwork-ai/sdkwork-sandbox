#![forbid(unsafe_code)]
//! Local Sandbox provider adapter.
//!
//! The [`host_boundary`] module is the production pure-data boundary: it validates command
//! requests against the Local Provider's fail-closed Executable/Path/Argv/Environment rules
//! without performing any host I/O or process creation. The [`process_runner`] module is the
//! real tokio-based execution slice behind the runner seam: bounded streamed output, hard
//! timeout with kill and reap, kill-on-drop, provider-owned executable resolution, and an
//! empty base environment. The [`command_executor`] module admits requests and tracks live
//! executions in a bounded registry with fenced cancellation.
//!
//! Descendant containment has landed for the Windows lane (suspended kill-on-close Job
//! Object supervision via `process-wrap`, with shell-detachment escapes recorded as the
//! `detached-and-breakaway-attempt-denial` evidence obligation); the Linux delegated
//! cgroup v2 lane and the real-platform evidence matrix are still pending, so the
//! Terminal capability stays unclaimed until that evidence-gated slice lands
//! (REQ-2026-0003). Host filesystem, network, and browser access are not
//! implemented until capability and isolation policies are approved.

pub mod command_admission;
pub mod command_executor;

#[cfg(test)]
mod command_conformance_tests;

#[cfg(test)]
mod command_executor_tests;
pub mod host_boundary;

#[cfg(test)]
mod command_admission_tests;

#[cfg(test)]
mod fake_host_boundary;
pub mod process_runner;
