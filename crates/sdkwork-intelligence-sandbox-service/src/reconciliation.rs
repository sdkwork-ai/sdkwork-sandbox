use sdkwork_sandbox_provider_spi::SandboxSessionId;

use crate::SandboxSessionState;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SandboxSessionReconciliationOutcome {
    Reconciled,
    Failed,
    /// Another lifecycle controller holds the lease; the item was not
    /// touched this round.
    LeaseUnavailable,
    /// The session's persisted data could not be loaded (for example an
    /// operation history above the retention bound), so it was reported and
    /// skipped instead of aborting the whole reconciliation page. The session
    /// itself is untouched; the retention policy is owned by
    /// `REQ-2026-0020`.
    Unreadable,
    /// The session disappeared while its reconciliation was in flight —
    /// distinct from `LeaseUnavailable`: a vanished session and a
    /// foreign-held lease are different operational tickets.
    Vanished,
}

/// The enumeration projection a repository returns for reconciliation: enough
/// to acquire a lease and report the item's persisted state, deliberately
/// without the operation ledger. A single session with unreadable persisted
/// data must not abort the page, so the authoritative reload happens
/// per session inside the reconciliation loop.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxSessionReconciliationCandidate {
    sandbox_session_id: SandboxSessionId,
    sandbox_session_state: SandboxSessionState,
}

impl SandboxSessionReconciliationCandidate {
    #[must_use]
    pub fn new(
        sandbox_session_id: SandboxSessionId,
        sandbox_session_state: SandboxSessionState,
    ) -> Self {
        Self {
            sandbox_session_id,
            sandbox_session_state,
        }
    }

    #[must_use]
    pub fn sandbox_session_id(&self) -> &SandboxSessionId {
        &self.sandbox_session_id
    }

    #[must_use]
    pub fn sandbox_session_state(&self) -> SandboxSessionState {
        self.sandbox_session_state
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxSessionReconciliationItem {
    sandbox_session_id: SandboxSessionId,
    sandbox_session_state: SandboxSessionState,
    sandbox_reconciliation_outcome: SandboxSessionReconciliationOutcome,
}

impl SandboxSessionReconciliationItem {
    pub(crate) fn new(
        sandbox_session_id: SandboxSessionId,
        sandbox_session_state: SandboxSessionState,
        sandbox_reconciliation_outcome: SandboxSessionReconciliationOutcome,
    ) -> Self {
        Self {
            sandbox_session_id,
            sandbox_session_state,
            sandbox_reconciliation_outcome,
        }
    }

    #[must_use]
    pub fn sandbox_session_id(&self) -> &SandboxSessionId {
        &self.sandbox_session_id
    }

    #[must_use]
    pub fn sandbox_session_state(&self) -> SandboxSessionState {
        self.sandbox_session_state
    }

    #[must_use]
    pub fn sandbox_reconciliation_outcome(&self) -> SandboxSessionReconciliationOutcome {
        self.sandbox_reconciliation_outcome
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxSessionReconciliationPage {
    sandbox_items: Vec<SandboxSessionReconciliationItem>,
    next_sandbox_session_id: Option<SandboxSessionId>,
}

impl SandboxSessionReconciliationPage {
    pub(crate) fn new(
        sandbox_items: Vec<SandboxSessionReconciliationItem>,
        next_sandbox_session_id: Option<SandboxSessionId>,
    ) -> Self {
        Self {
            sandbox_items,
            next_sandbox_session_id,
        }
    }

    #[must_use]
    pub fn sandbox_items(&self) -> &[SandboxSessionReconciliationItem] {
        &self.sandbox_items
    }

    #[must_use]
    pub fn next_sandbox_session_id(&self) -> Option<&SandboxSessionId> {
        self.next_sandbox_session_id.as_ref()
    }
}
