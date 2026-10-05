//! Real-process end-to-end test of the template-resolution flow: the start
//! command resolves from the plan's version reference through the declared
//! resolver port, then runs as a real host process through the boundary
//! allowlist, the tokio runner, the executor, the runtime bridge and the
//! port — reaching `Started` on a zero exit.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;

use sdkwork_intelligence_sandbox_template_authority::BoundedSandboxTemplateRegistry;
use sdkwork_intelligence_sandbox_worker_local::SandboxLaunchStartCommandPort;
use sdkwork_intelligence_sandbox_worker_local_exec::{
    SandboxLocalStartCommandExecutor, SandboxResolvedStartCommand,
    SandboxStartCommandResolutionError, SandboxStartCommandScope, SandboxTemplateRegistryResolver,
};
use sdkwork_sandbox_provider_local::command_executor::SandboxLocalCommandExecutor;
use sdkwork_sandbox_provider_local::host_boundary::SandboxLocalHostBoundary;
use sdkwork_sandbox_provider_local::process_runner::{
    SandboxLocalProcessRunnerConfig, SandboxLocalTokioProcessRunner,
};
use sdkwork_sandbox_provider_spi::SandboxCommandLimits;

fn sandbox_executable_roots() -> Vec<PathBuf> {
    if cfg!(windows) {
        vec![PathBuf::from("C:\\Windows\\System32")]
    } else {
        vec![PathBuf::from("/usr/bin"), PathBuf::from("/bin")]
    }
}

/// The scripted resolver: resolves the known version to the host-appropriate
/// echo invocation and refuses everything else.
struct ScriptedResolver;

impl sdkwork_intelligence_sandbox_worker_local_exec::SandboxStartCommandResolverPort
    for ScriptedResolver
{
    fn sandbox_resolve(
        &self,
        sandbox_template_version_ref: &str,
    ) -> Result<SandboxResolvedStartCommand, SandboxStartCommandResolutionError> {
        if sandbox_template_version_ref != "version-1" {
            return Err(SandboxStartCommandResolutionError::SandboxStartCommandNotFound);
        }
        if cfg!(windows) {
            // `echo` is a cmd builtin on Windows; the fixture resolves `cmd`
            // from System32 and drives the builtin through /C.
            Ok(SandboxResolvedStartCommand {
                sandbox_executable: "cmd".to_owned(),
                sandbox_arguments: vec![
                    "/C".to_owned(),
                    "echo".to_owned(),
                    "fast-start".to_owned(),
                ],
            })
        } else {
            Ok(SandboxResolvedStartCommand {
                sandbox_executable: "echo".to_owned(),
                sandbox_arguments: vec!["fast-start".to_owned()],
            })
        }
    }
}

fn sandbox_limits() -> SandboxCommandLimits {
    SandboxCommandLimits {
        sandbox_timeout_ms: 10_000,
        sandbox_stdout_byte_limit: 64 * 1024,
        sandbox_stderr_byte_limit: 64 * 1024,
        sandbox_cleanup_timeout_ms: 2_000,
        sandbox_max_process_count: 8,
    }
}

fn sandbox_scope() -> SandboxStartCommandScope {
    SandboxStartCommandScope {
        sandbox_tenant_id: "tenant-1".to_owned(),
        sandbox_provider_id: "provider-local".to_owned(),
        sandbox_workspace_id: "workspace-1".to_owned(),
        sandbox_session_id: "session-1".to_owned(),
        sandbox_id: "sandbox-1".to_owned(),
        sandbox_runtime_binding_id: "binding-1".to_owned(),
        sandbox_fencing_token: 1,
        sandbox_working_directory: ".".to_owned(),
        sandbox_environment: BTreeMap::new(),
        sandbox_command_limits: sandbox_limits(),
    }
}

fn sandbox_executor() -> Arc<SandboxLocalCommandExecutor> {
    Arc::new(SandboxLocalCommandExecutor::new(
        Arc::new(SandboxLocalHostBoundary::new(
            ["cmd", "echo"].into_iter().map(str::to_owned).collect(),
            [].into_iter().map(str::to_owned).collect(),
        )),
        Arc::new(SandboxLocalTokioProcessRunner::new(
            SandboxLocalProcessRunnerConfig {
                sandbox_executable_roots: sandbox_executable_roots(),
                sandbox_workspace_root: std::env::temp_dir(),
            },
        )),
    ))
}

fn sandbox_wiring() -> SandboxLocalStartCommandExecutor {
    SandboxLocalStartCommandExecutor::sandbox_new(
        sandbox_executor(),
        Arc::new(ScriptedResolver),
        sandbox_scope(),
    )
    .expect("bridge runtime builds")
}

#[test]
fn the_start_command_resolves_from_the_version_and_a_real_process_reaches_started() {
    assert_eq!(
        sandbox_wiring().sandbox_start("plan-1", "version-1"),
        sdkwork_intelligence_sandbox_worker_local::SandboxStartCommandOutcome::Started,
        "the resolved command must run as a real process to started",
    );
}

#[test]
fn an_unresolvable_version_fails_deterministically_never_started() {
    assert_eq!(
        sandbox_wiring().sandbox_start("plan-1", "version-unknown"),
        sdkwork_intelligence_sandbox_worker_local::SandboxStartCommandOutcome::Failed,
        "a missing command is a deterministic failure, never started",
    );
}

#[test]
fn a_boundary_refusal_lands_uncertain_never_started() {
    // "version-1" resolves to the echo invocation, but the executable is not
    // on this wiring's boundary allowlist, so the executor refuses without a
    // terminal outcome.
    let boundary = SandboxLocalHostBoundary::new(
        ["ping"].into_iter().map(str::to_owned).collect(),
        [].into_iter().map(str::to_owned).collect(),
    );
    let runner = SandboxLocalTokioProcessRunner::new(SandboxLocalProcessRunnerConfig {
        sandbox_executable_roots: sandbox_executable_roots(),
        sandbox_workspace_root: std::env::temp_dir(),
    });
    let wiring = SandboxLocalStartCommandExecutor::sandbox_new(
        Arc::new(SandboxLocalCommandExecutor::new(
            Arc::new(boundary),
            Arc::new(runner),
        )),
        Arc::new(ScriptedResolver),
        sandbox_scope(),
    )
    .expect("bridge runtime builds");
    assert_eq!(
        wiring.sandbox_start("plan-1", "version-1"),
        sdkwork_intelligence_sandbox_worker_local::SandboxStartCommandOutcome::Uncertain,
        "an executor error without a terminal outcome is uncertainty",
    );
}

#[test]
fn a_registry_published_version_resolves_and_a_real_process_reaches_started() {
    // Publish the definition and version through the bounded registry.
    let mut registry = BoundedSandboxTemplateRegistry::sandbox_new();
    let definition_id =
        sdkwork_intelligence_sandbox_template_authority::SandboxTemplateDefinitionId::new(
            "definition-1",
        )
        .expect("valid id");
    registry
        .sandbox_publish_definition(
            sdkwork_intelligence_sandbox_template_authority::SandboxTemplateDefinition::sandbox_publish(
                definition_id.clone(),
                "1",
                sdkwork_intelligence_sandbox_template_authority::SandboxTemplateOpaqueRef::new(
                    "base-env-1",
                )
                .expect("valid ref"),
                Vec::new(),
                BTreeMap::new(),
                if cfg!(windows) {
                    "cmd /C echo fast-start"
                } else {
                    "echo fast-start"
                },
                900,
            )
            .expect("valid definition"),
        )
        .expect("definition published");
    registry
        .sandbox_publish_version(
            sdkwork_intelligence_sandbox_template_authority::SandboxTemplateVersion::sandbox_publish(
                sdkwork_intelligence_sandbox_template_authority::SandboxTemplateVersionId::new(
                    "version-1",
                )
                .expect("valid id"),
                definition_id,
                BTreeSet::new(),
                BTreeSet::new(),
                sdkwork_intelligence_sandbox_template_authority::SandboxTemplateOpaqueRef::new(
                    "req-0012-rootfs-sha256-0000000000000000000000000000000000000000000000000000000000000000",
                )
                .expect("valid tuple ref"),
            )
            .expect("valid version"),
        )
        .expect("version published");

    // The registry-backed resolver resolves the published start command and
    // the real process carries the plan to started.
    let wiring = SandboxLocalStartCommandExecutor::sandbox_new(
        sandbox_executor(),
        Arc::new(SandboxTemplateRegistryResolver::sandbox_new(Arc::new(
            std::sync::Mutex::new(registry),
        ))),
        sandbox_scope(),
    )
    .expect("bridge runtime builds");
    assert_eq!(
        wiring.sandbox_start("plan-1", "version-1"),
        sdkwork_intelligence_sandbox_worker_local::SandboxStartCommandOutcome::Started,
        "a registry-published version must resolve and run as a real process",
    );
}
