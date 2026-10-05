//! Evidence gates and the line layering
//! (`contract`: `evidenceGates`, `layering`, `bindingSemantics`).
//!
//! The gates are fail-closed documentation: they state, as constants, what
//! must be true before any worker or execution surface is authorized. The
//! layering constants pin the five authorities this line consumes by opaque
//! reference and never redefines.

/// The Template authority whose immutable versions own the start command
/// (`layering.templateAuthority`); the plan consumes them by opaque
/// reference and never copies the command.
pub const SANDBOX_LAUNCH_TEMPLATE_AUTHORITY: &str = "REQ-2026-0029";

/// The Build authority whose records produce the artifacts behind the
/// template version (`layering.buildAuthority`).
pub const SANDBOX_LAUNCH_BUILD_AUTHORITY: &str = "REQ-2026-0032";

/// The Pool authority whose fenced claims the plan binds
/// (`layering.poolClaimAuthority`).
pub const SANDBOX_LAUNCH_POOL_CLAIM_AUTHORITY: &str = "REQ-2026-0019";

/// The Command-execution authority that owns command semantics
/// (`layering.commandExecutionAuthority`); this line executes nothing.
pub const SANDBOX_LAUNCH_COMMAND_EXECUTION_AUTHORITY: &str = "REQ-2026-0007";

/// The Snapshot/Fork authority whose derived instances plan through this
/// authority too (`layering.snapshotForkAuthority`).
pub const SANDBOX_LAUNCH_SNAPSHOT_FORK_AUTHORITY: &str = "REQ-2026-0031";

/// The Pool evidence gate any warm-slot consumption through the launch path
/// additionally requires (`layering.warmMicroVmSlotGate`).
pub const SANDBOX_LAUNCH_WARM_SLOT_GATE: &str = "REQ-2026-0019";

/// Real fast-start runtime evidence is release-blocking.
pub const SANDBOX_LAUNCH_REAL_FAST_START_RUNTIME_EVIDENCE_REQUIRED: bool = true;

/// First-command-zero-wait measurement evidence is release-blocking.
pub const SANDBOX_LAUNCH_FIRST_COMMAND_ZERO_WAIT_EVIDENCE_REQUIRED: bool = true;

/// Whether registering this authority enables the Pool warm slot. It does
/// not.
pub const SANDBOX_LAUNCH_REGISTRATION_ENABLES_WARM_SLOT: bool = false;

/// Whether this registration executes any command. It does not.
pub const SANDBOX_LAUNCH_REGISTRATION_EXECUTES_NO_COMMANDS: bool = true;

/// Whether a `planned` plan alone authorizes execution. It does not: the
/// worker slice is a separate authorization
/// (`bindingSemantics.plannedAloneAuthorizesExecution`).
pub const SANDBOX_LAUNCH_PLANNED_ALONE_AUTHORIZES_EXECUTION: bool = false;

/// Whether the authority-model slice is authorized. It is (REVIEW-20261006,
/// single-owner structured approval); the runtime surfaces below stay shut.
#[must_use]
pub const fn sandbox_launch_authority_model_slice_authorized() -> bool {
    true
}

/// Whether launch execution or a worker is authorized. It is not.
#[must_use]
pub const fn sandbox_launch_execution_or_worker_authorized() -> bool {
    false
}
