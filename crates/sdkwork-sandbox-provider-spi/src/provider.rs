use std::collections::BTreeSet;

use async_trait::async_trait;

use crate::{
    IsolationAssurance, RuntimeCapability, SandboxFencingToken, SandboxId,
    SandboxProviderAllocationRef, SandboxProviderId, SandboxProviderKind, SandboxProviderResult,
    SandboxRuntimeBindingId, SandboxSessionId, SandboxWorkspaceId, TenantId,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxProviderDescriptor {
    sandbox_provider_id: SandboxProviderId,
    sandbox_provider_kind: SandboxProviderKind,
    sandbox_capabilities: BTreeSet<RuntimeCapability>,
    sandbox_isolation_assurance: IsolationAssurance,
}

impl SandboxProviderDescriptor {
    pub fn new(
        sandbox_provider_id: SandboxProviderId,
        sandbox_provider_kind: SandboxProviderKind,
        sandbox_capabilities: impl IntoIterator<Item = RuntimeCapability>,
        sandbox_isolation_assurance: IsolationAssurance,
    ) -> Self {
        Self {
            sandbox_provider_id,
            sandbox_provider_kind,
            sandbox_capabilities: sandbox_capabilities.into_iter().collect(),
            sandbox_isolation_assurance,
        }
    }

    #[must_use]
    pub fn sandbox_provider_id(&self) -> &SandboxProviderId {
        &self.sandbox_provider_id
    }

    #[must_use]
    pub fn sandbox_provider_kind(&self) -> &SandboxProviderKind {
        &self.sandbox_provider_kind
    }

    #[must_use]
    pub fn sandbox_runtime_capabilities(&self) -> &BTreeSet<RuntimeCapability> {
        &self.sandbox_capabilities
    }

    #[must_use]
    pub fn sandbox_isolation_assurance(&self) -> IsolationAssurance {
        self.sandbox_isolation_assurance
    }

    #[must_use]
    pub fn satisfies_sandbox_requirements(
        &self,
        sandbox_required_capabilities: &BTreeSet<RuntimeCapability>,
        sandbox_minimum_assurance: IsolationAssurance,
    ) -> bool {
        self.sandbox_isolation_assurance >= sandbox_minimum_assurance
            && sandbox_required_capabilities.is_subset(&self.sandbox_capabilities)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SandboxProviderHealthStatus {
    Ready,
    Degraded,
    Unavailable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxProviderHealth {
    pub sandbox_provider_health_status: SandboxProviderHealthStatus,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxProviderAllocationRequest {
    pub tenant_id: TenantId,
    pub sandbox_workspace_id: SandboxWorkspaceId,
    pub sandbox_session_id: SandboxSessionId,
    pub sandbox_id: SandboxId,
    pub sandbox_runtime_binding_id: SandboxRuntimeBindingId,
    pub sandbox_fencing_token: SandboxFencingToken,
    pub sandbox_required_capabilities: BTreeSet<RuntimeCapability>,
    pub sandbox_minimum_assurance: IsolationAssurance,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxProviderAllocation {
    pub sandbox_allocation_reference: SandboxProviderAllocationRef,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxProviderStartRequest {
    pub tenant_id: TenantId,
    pub sandbox_workspace_id: SandboxWorkspaceId,
    pub sandbox_session_id: SandboxSessionId,
    pub sandbox_id: SandboxId,
    pub sandbox_runtime_binding_id: SandboxRuntimeBindingId,
    pub sandbox_fencing_token: SandboxFencingToken,
    pub sandbox_allocation_reference: SandboxProviderAllocationRef,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SandboxProviderReadiness {
    pub sandbox_provider_ready: bool,
    pub sandbox_policy_enforced: bool,
    pub sandbox_workspace_attached: bool,
}

impl SandboxProviderReadiness {
    #[must_use]
    pub fn is_sandbox_running_ready(self) -> bool {
        self.sandbox_provider_ready
            && self.sandbox_policy_enforced
            && self.sandbox_workspace_attached
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxProviderStopRequest {
    pub tenant_id: TenantId,
    pub sandbox_session_id: SandboxSessionId,
    pub sandbox_id: SandboxId,
    pub sandbox_runtime_binding_id: SandboxRuntimeBindingId,
    pub sandbox_fencing_token: SandboxFencingToken,
    pub sandbox_allocation_reference: SandboxProviderAllocationRef,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxProviderDestroyRequest {
    pub tenant_id: TenantId,
    pub sandbox_session_id: SandboxSessionId,
    pub sandbox_id: SandboxId,
    pub sandbox_runtime_binding_id: SandboxRuntimeBindingId,
    pub sandbox_fencing_token: SandboxFencingToken,
    pub sandbox_allocation_reference: Option<SandboxProviderAllocationRef>,
}

#[async_trait]
pub trait SandboxProvider: Send + Sync {
    fn sandbox_provider_descriptor(&self) -> &SandboxProviderDescriptor;

    async fn sandbox_provider_health(&self) -> SandboxProviderResult<SandboxProviderHealth>;

    async fn allocate(
        &self,
        sandbox_request: SandboxProviderAllocationRequest,
    ) -> SandboxProviderResult<SandboxProviderAllocation>;

    async fn start(
        &self,
        sandbox_request: SandboxProviderStartRequest,
    ) -> SandboxProviderResult<SandboxProviderReadiness>;

    async fn stop(&self, sandbox_request: SandboxProviderStopRequest) -> SandboxProviderResult<()>;

    async fn destroy(
        &self,
        sandbox_request: SandboxProviderDestroyRequest,
    ) -> SandboxProviderResult<()>;
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use crate::{
        IsolationAssurance, RuntimeCapability, SandboxProviderId, SandboxProviderKind,
        SandboxProviderReadiness,
    };

    use super::SandboxProviderDescriptor;

    #[test]
    fn sandbox_provider_descriptor_fails_closed_on_capability_and_assurance() {
        let sandbox_provider_id = SandboxProviderId::parse("local");
        assert!(sandbox_provider_id.is_ok());
        let sandbox_provider_kind = SandboxProviderKind::parse("local");
        assert!(sandbox_provider_kind.is_ok());
        let sandbox_provider_descriptor = SandboxProviderDescriptor::new(
            sandbox_provider_id.unwrap_or_else(|error| panic!("valid test provider id: {error}")),
            sandbox_provider_kind
                .unwrap_or_else(|error| panic!("valid test provider kind: {error}")),
            [RuntimeCapability::Filesystem],
            IsolationAssurance::HostUser,
        );

        let filesystem = BTreeSet::from([RuntimeCapability::Filesystem]);
        assert!(sandbox_provider_descriptor
            .satisfies_sandbox_requirements(&filesystem, IsolationAssurance::HostUser));
        assert!(!sandbox_provider_descriptor
            .satisfies_sandbox_requirements(&filesystem, IsolationAssurance::Container));

        let terminal = BTreeSet::from([RuntimeCapability::Terminal]);
        assert!(!sandbox_provider_descriptor
            .satisfies_sandbox_requirements(&terminal, IsolationAssurance::HostUser));
    }

    #[test]
    fn sandbox_provider_readiness_requires_workspace_attachment_and_policy_enforcement() {
        let ready = SandboxProviderReadiness {
            sandbox_provider_ready: true,
            sandbox_policy_enforced: true,
            sandbox_workspace_attached: true,
        };
        assert!(ready.is_sandbox_running_ready());

        for not_ready in [
            SandboxProviderReadiness {
                sandbox_provider_ready: false,
                ..ready
            },
            SandboxProviderReadiness {
                sandbox_policy_enforced: false,
                ..ready
            },
            SandboxProviderReadiness {
                sandbox_workspace_attached: false,
                ..ready
            },
        ] {
            assert!(!not_ready.is_sandbox_running_ready());
        }
    }
}
