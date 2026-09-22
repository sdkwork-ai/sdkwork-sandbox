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

#[cfg(test)]
mod tests {
    use super::IsolationAssurance;

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
}
