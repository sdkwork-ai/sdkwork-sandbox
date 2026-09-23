#![forbid(unsafe_code)]
//! Phase 0 boundary for the local Sandbox provider adapter.
//!
//! The [`host_boundary`] module is the production pure-data boundary: it validates command
//! requests against the Local Provider's fail-closed Executable/Path/Argv/Environment rules
//! without performing any host I/O or process creation. Process execution, supervision, and
//! cleanup arrive in later authorized slices.
//!
//! Host process, filesystem, network, browser, and terminal access are not
//! implemented until capability and isolation policies are approved.

pub mod command_admission;
pub mod host_boundary;

#[cfg(test)]
mod command_admission_tests;

#[cfg(test)]
mod fake_host_boundary;
