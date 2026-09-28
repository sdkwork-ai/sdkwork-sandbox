//! Drain-aware readiness for the Sandbox API assembly.
//!
//! A gracefully draining instance must stop receiving new work before its
//! last in-flight request finishes: a load balancer that keeps polling
//! `/readyz` during drain otherwise routes new connections to a process that
//! is about to exit. The framework owns the readiness plumbing
//! (`sdkwork-web-bootstrap`); this module supplies the Sandbox-side probe
//! through its public [`ReadinessCheck`] extension point instead of forking
//! it.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use sdkwork_web_bootstrap::ReadinessCheck;

/// Shared drain flag. The gateway flips it when the graceful-shutdown signal
/// arrives; every probe composed with it fails from that moment on.
#[derive(Clone)]
pub struct SandboxDrainGate {
    sandbox_draining: Arc<AtomicBool>,
}

impl Default for SandboxDrainGate {
    fn default() -> Self {
        Self::new()
    }
}

impl SandboxDrainGate {
    #[must_use]
    pub fn new() -> Self {
        Self {
            sandbox_draining: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Marks the process as draining; readiness fails from this point.
    pub fn begin_drain(&self) {
        self.sandbox_draining.store(true, Ordering::SeqCst);
    }

    #[must_use]
    pub fn is_draining(&self) -> bool {
        self.sandbox_draining.load(Ordering::SeqCst)
    }
}

impl ReadinessCheck for SandboxDrainGate {
    fn check(&self) -> sdkwork_web_bootstrap::ReadinessFuture<'_> {
        let sandbox_draining = Arc::clone(&self.sandbox_draining);
        Box::pin(async move {
            if sandbox_draining.load(Ordering::SeqCst) {
                // Client-safe detail: the instance is shutting down and must
                // not receive new traffic (`SECURITY_SPEC.md` §3 — no
                // dependency internals in probe bodies).
                return Err("the instance is draining for shutdown".to_owned());
            }
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn sandbox_drain_gate_readiness_fails_only_after_drain_begins() {
        let sandbox_drain_gate = SandboxDrainGate::new();
        assert!(!sandbox_drain_gate.is_draining());
        sandbox_drain_gate
            .check()
            .await
            .expect("readiness before drain must pass");
        sandbox_drain_gate.begin_drain();
        assert!(sandbox_drain_gate.is_draining());
        assert!(
            sandbox_drain_gate.check().await.is_err(),
            "readiness after begin_drain must fail so load balancers stop routing"
        );
        // The flag is shared: a clone flipped in another context keeps the
        // original gate failing.
        let sandbox_cloned_gate = sandbox_drain_gate.clone();
        assert!(sandbox_cloned_gate.is_draining());
    }
}
