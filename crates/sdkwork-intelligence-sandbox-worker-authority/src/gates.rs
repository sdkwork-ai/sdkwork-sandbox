//! Evidence gates and the line layering
//! (`contract`: `evidenceGates`, `layering`, `bindingSemantics`).
//!
//! The gates are fail-closed documentation: they state, as constants, what
//! must be true before any execution adapter is authorized. The layering
//! constants pin the authorities this line consumes by opaque reference and
//! never redefines.

/// The Launch-plan authority whose `planned -> consumed` handoff is the only
/// bridge into execution (`layering.launchPlanAuthority`); the worker never
/// writes plan state directly.
pub const SANDBOX_WORKER_LAUNCH_PLAN_AUTHORITY: &str = "REQ-2026-0033";

/// The Command-execution authority whose executor port runs the start
/// command (`layering.commandExecutionAuthority`); the worker reimplements
/// no executor.
pub const SANDBOX_WORKER_COMMAND_EXECUTION_AUTHORITY: &str = "REQ-2026-0007";

/// The Provider SPI boundary allocation and start go through
/// (`layering.providerSpiBoundary`).
pub const SANDBOX_WORKER_PROVIDER_SPI_BOUNDARY: &str = "REQ-2026-0002";

/// The Firecracker provider authority the VMM lane stays locked behind
/// (`layering.firecrackerProviderAuthority`).
pub const SANDBOX_WORKER_FIRECRACKER_PROVIDER_AUTHORITY: &str = "REQ-2026-0008";

/// The Pool evidence gate any warm-slot consumption through the worker
/// additionally requires (`layering.warmMicroVmSlotGate`).
pub const SANDBOX_WORKER_WARM_SLOT_GATE: &str = "REQ-2026-0019";

/// Real worker execution evidence (lane-scoped) is release-blocking.
pub const SANDBOX_WORKER_REAL_EXECUTION_EVIDENCE_REQUIRED: bool = true;

/// KVM-lane evidence is release-blocking.
pub const SANDBOX_WORKER_KVM_LANE_EVIDENCE_REQUIRED: bool = true;

/// First-command-zero-wait measurement evidence is release-blocking.
pub const SANDBOX_WORKER_FIRST_COMMAND_ZERO_WAIT_EVIDENCE_REQUIRED: bool = true;

/// Whether registering this authority enables the Pool warm slot. It does
/// not.
pub const SANDBOX_WORKER_REGISTRATION_ENABLES_WARM_SLOT: bool = false;

/// Whether this registration touches the VMM. It does not.
pub const SANDBOX_WORKER_REGISTRATION_DOES_NOT_TOUCH_THE_VMM: bool = true;

/// Whether a `started` claim may be made without the executor reporting
/// success. It may not: `started` is earned through the executor.
pub const SANDBOX_WORKER_COMPLETION_MUST_BE_EARNED: bool = true;

/// Whether the authority-model slice is authorized. It is (REVIEW-20261006,
/// single-owner structured approval); the runtime surfaces below stay shut.
#[must_use]
pub const fn sandbox_worker_authority_model_slice_authorized() -> bool {
    true
}

/// Whether the local-lane execution adapter is authorized. It is not: it is
/// its own later slice.
#[must_use]
pub const fn sandbox_local_lane_execution_slice_authorized() -> bool {
    false
}

/// Whether the Firecracker lane execution is authorized. It is not.
#[must_use]
pub const fn sandbox_firecracker_lane_execution_authorized() -> bool {
    false
}
