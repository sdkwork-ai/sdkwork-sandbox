#![forbid(unsafe_code)]
//! PostgreSQL persistence adapter for SDKWork Sandbox lifecycle state.

mod codec;
mod encryption;
mod instance;
mod reencryption;
mod repository;

pub use encryption::{
    SandboxProviderAllocationKey, SandboxProviderAllocationKeySource,
    SdkworkUtilsSandboxProviderAllocationProtector,
};
pub use instance::SqlxSandboxInstanceRepository;
pub use reencryption::SandboxProviderAllocationReencryptionPage;
pub use repository::SqlxSandboxSessionRepository;
