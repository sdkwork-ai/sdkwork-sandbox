#![forbid(unsafe_code)]
//! Firecracker microVM Sandbox provider boundary (`REQ-2026-0008`).
//!
//! This crate is the Gate 0 provider-boundary slice: the exact-tuple artifact
//! manifest validation ([`artifact`]), the fail-closed node preflight
//! ([`preflight`]), the provider-private fencing state with a durable atomic
//! store ([`fencing`]), the host isolation broker seam ([`broker`]), the
//! pure-data guest command boundary and its admission sequence
//! ([`guest_boundary`], [`command_admission`]), the bounded command executor
//! over the authenticated guest channel seam ([`guest_channel`],
//! [`command_executor`]), and the fenced lifecycle ([`lifecycle`]) that
//! implements the provider-neutral `SandboxProvider` port.
//!
//! The slice performs no host I/O beyond the fencing store's own
//! provider-private records, spawns no process, and opens no socket: every
//! host or guest interaction sits behind an injectable seam (the broker port,
//! the guest channel port, the host facts source), and the real
//! Linux-KVM/Jailer/VMM execution is a separate evidence-gated slice owned by
//! `REQ-2026-0011` through `REQ-2026-0017`. The provider truthfully reports
//! `MicroVm` assurance and derives its claimed capabilities from supplied
//! evidence only - on a host whose preflight fails, health reports
//! Unavailable and no capability is claimed. There is no fallback to a weaker
//! provider, by contract.

pub mod artifact;
pub mod broker;
pub mod command_admission;
pub mod command_executor;
pub mod fencing;
pub mod guest_boundary;
pub mod guest_channel;
pub mod lifecycle;
pub mod preflight;

#[cfg(test)]
mod command_conformance_tests;
#[cfg(test)]
mod fake_host;
#[cfg(test)]
mod lifecycle_tests;
