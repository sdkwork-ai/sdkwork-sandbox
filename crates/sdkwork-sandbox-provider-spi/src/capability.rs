#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeCapability {
    Terminal,
    Filesystem,
    Git,
    Build,
    Browser,
    PortForward,
    McpTransport,
    Environment,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum IsolationAssurance {
    HostUser,
    Container,
    UserSpaceKernel,
    MicroVm,
    DedicatedVm,
}

impl RuntimeCapability {
    /// The `lower_snake_case` vocabulary used on the wire, in the persisted
    /// JSONB column and in provider descriptors. One spelling, one place: a
    /// second copy of this table in a route or adapter crate is exactly how an
    /// API vocabulary and a storage vocabulary drift apart.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Terminal => "terminal",
            Self::Filesystem => "filesystem",
            Self::Git => "git",
            Self::Build => "build",
            Self::Browser => "browser",
            Self::PortForward => "port_forward",
            Self::McpTransport => "mcp_transport",
            Self::Environment => "environment",
        }
    }

    /// Parses the vocabulary emitted by [`Self::as_str`]. Unknown values return
    /// `None`; every caller decides whether that is a validation error (wire) or
    /// an unreadable row (storage).
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "terminal" => Some(Self::Terminal),
            "filesystem" => Some(Self::Filesystem),
            "git" => Some(Self::Git),
            "build" => Some(Self::Build),
            "browser" => Some(Self::Browser),
            "port_forward" => Some(Self::PortForward),
            "mcp_transport" => Some(Self::McpTransport),
            "environment" => Some(Self::Environment),
            _ => None,
        }
    }
}

impl IsolationAssurance {
    /// The `lower_snake_case` vocabulary used on the wire, in the persisted
    /// column and in provider descriptors.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::HostUser => "host_user",
            Self::Container => "container",
            Self::UserSpaceKernel => "user_space_kernel",
            Self::MicroVm => "micro_vm",
            Self::DedicatedVm => "dedicated_vm",
        }
    }

    /// Parses the vocabulary emitted by [`Self::as_str`].
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "host_user" => Some(Self::HostUser),
            "container" => Some(Self::Container),
            "user_space_kernel" => Some(Self::UserSpaceKernel),
            "micro_vm" => Some(Self::MicroVm),
            "dedicated_vm" => Some(Self::DedicatedVm),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{IsolationAssurance, RuntimeCapability};

    /// F-02 lock-in: the isolation ladder is the derived `Ord` on this enum,
    /// so the declaration order above IS the security semantics that
    /// `satisfies_sandbox_requirements` fails closed against. The negotiation
    /// test in `provider.rs` only exercises the HostUser/Container pair;
    /// every adjacent pair is locked here so an accidental reorder — which
    /// would silently re-rank isolation strength for every provider — cannot
    /// pass.
    #[test]
    fn isolation_assurance_declaration_order_is_the_security_ladder() {
        let isolation_ladder = [
            IsolationAssurance::HostUser,
            IsolationAssurance::Container,
            IsolationAssurance::UserSpaceKernel,
            IsolationAssurance::MicroVm,
            IsolationAssurance::DedicatedVm,
        ];
        for weaker_position in 0..isolation_ladder.len() {
            for stronger_position in (weaker_position + 1)..isolation_ladder.len() {
                let weaker = isolation_ladder[weaker_position];
                let stronger = isolation_ladder[stronger_position];
                assert!(
                    weaker < stronger,
                    "isolation ladder regression: {weaker:?} must order strictly below {stronger:?}"
                );
            }
        }
        for assurance in isolation_ladder {
            assert!(!(assurance < assurance));
        }
    }

    /// The wire/storage vocabulary is one table. A route crate that accepts a
    /// spelling this table does not emit, or a column value this table cannot
    /// read back, is a silent contract split — so both directions are locked
    /// here over the whole vocabulary, not over a sample.
    #[test]
    fn capability_and_assurance_vocabularies_round_trip_and_reject_unknown_values() {
        for capability in [
            RuntimeCapability::Terminal,
            RuntimeCapability::Filesystem,
            RuntimeCapability::Git,
            RuntimeCapability::Build,
            RuntimeCapability::Browser,
            RuntimeCapability::PortForward,
            RuntimeCapability::McpTransport,
            RuntimeCapability::Environment,
        ] {
            assert_eq!(
                Some(capability),
                RuntimeCapability::parse(capability.as_str()),
                "runtime capability vocabulary must round-trip through {}",
                capability.as_str()
            );
        }
        assert_eq!(None, RuntimeCapability::parse("shell"));
        assert_eq!(None, RuntimeCapability::parse("PortForward"));

        for assurance in [
            IsolationAssurance::HostUser,
            IsolationAssurance::Container,
            IsolationAssurance::UserSpaceKernel,
            IsolationAssurance::MicroVm,
            IsolationAssurance::DedicatedVm,
        ] {
            assert_eq!(
                Some(assurance),
                IsolationAssurance::parse(assurance.as_str()),
                "isolation assurance vocabulary must round-trip through {}",
                assurance.as_str()
            );
        }
        assert_eq!(None, IsolationAssurance::parse("gvisor"));
        assert_eq!(None, IsolationAssurance::parse("MicroVM"));
    }
}
