#![forbid(unsafe_code)]
//! Provider-neutral authority model for SDKWork Sandbox instance fast-start
//! launches (`REQ-2026-0033`, `ADR-20261006`).
//!
//! The crate carries the authority-model slice REVIEW-20261006 authorized:
//! the [`launch::SandboxInstanceLaunchPlan`] record with the closed lifecycle
//! [`state`] machine (`planned` is the only initial state and authorizes no
//! execution; `consumed`/`expired`/`quarantined` are terminal), the binding
//! rules (every plan binds one fenced pool claim; an expired or released
//! claim can only ever expire its plan), and the [`gates`] evidence and
//! layering constants. Records are private-fielded with read-only accessors,
//! so every lifecycle move is a state-machine transition.
//!
//! What this crate deliberately does not own: any launch execution runtime,
//! worker, CLI, public API/SDK or deployment profile — the contract's
//! `forbidden` block keeps every one of those surfaces closed until its own
//! requirement slice lands, no E2B fast-create capability-parity claim may be
//! made before a worker slice exists, and warm-slot consumption additionally
//! requires the `REQ-2026-0019` pool evidence gate. The start command's
//! semantics stay owned by `REQ-2026-0029` and are consumed by opaque
//! reference, never copied.

mod bounds;
mod error;
mod gates;
mod launch;
mod state;

pub use bounds::MAX_SANDBOX_LAUNCH_PLAN_ID_LENGTH;
pub use error::{SandboxLaunchAuthorityError, SandboxLaunchAuthorityResult};
pub use gates::{
    sandbox_launch_authority_model_slice_authorized, sandbox_launch_execution_or_worker_authorized,
    SANDBOX_LAUNCH_BUILD_AUTHORITY, SANDBOX_LAUNCH_COMMAND_EXECUTION_AUTHORITY,
    SANDBOX_LAUNCH_FIRST_COMMAND_ZERO_WAIT_EVIDENCE_REQUIRED,
    SANDBOX_LAUNCH_PLANNED_ALONE_AUTHORIZES_EXECUTION, SANDBOX_LAUNCH_POOL_CLAIM_AUTHORITY,
    SANDBOX_LAUNCH_REAL_FAST_START_RUNTIME_EVIDENCE_REQUIRED,
    SANDBOX_LAUNCH_REGISTRATION_ENABLES_WARM_SLOT,
    SANDBOX_LAUNCH_REGISTRATION_EXECUTES_NO_COMMANDS, SANDBOX_LAUNCH_SNAPSHOT_FORK_AUTHORITY,
    SANDBOX_LAUNCH_TEMPLATE_AUTHORITY, SANDBOX_LAUNCH_WARM_SLOT_GATE,
};
pub use launch::SandboxInstanceLaunchPlan;
pub use state::SandboxLaunchPlanState;

#[cfg(test)]
mod tests;
