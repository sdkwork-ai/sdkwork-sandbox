use std::collections::{BTreeMap, BTreeSet};

use super::{
    SandboxLocalHostBoundary, SandboxLocalHostBoundaryError, SandboxLocalHostBoundaryLimits,
};

fn sandbox_boundary() -> SandboxLocalHostBoundary {
    SandboxLocalHostBoundary::new(
        BTreeSet::from(["toybox".to_owned(), "python3".to_owned()]),
        BTreeSet::from(["SANDBOX_MODE".to_owned(), "LANG".to_owned()]),
    )
}

fn sandbox_environment() -> BTreeMap<&'static str, &'static str> {
    BTreeMap::from([("SANDBOX_MODE", "strict")])
}

fn sandbox_environment_entry(
    name: &'static str,
    value: &'static str,
) -> BTreeMap<&'static str, &'static str> {
    BTreeMap::from([(name, value)])
}

#[test]
fn accepts_a_well_formed_sandbox_command_request() {
    let boundary = sandbox_boundary();
    assert_eq!(
        boundary.validate_sandbox_command(
            "toybox",
            &["echo", "hello"],
            "workspace/out",
            &sandbox_environment(),
        ),
        Ok(())
    );
}

#[test]
fn rejects_an_executable_that_is_not_a_bare_name() {
    let boundary = sandbox_boundary();
    assert_eq!(
        boundary.validate_sandbox_command(
            "",
            &[],
            "workspace",
            &sandbox_environment(),
        ),
        Err(SandboxLocalHostBoundaryError::ExecutableInvalid)
    );
    assert_eq!(
        boundary.validate_sandbox_command(
            "/bin/toybox",
            &[],
            "workspace",
            &sandbox_environment(),
        ),
        Err(SandboxLocalHostBoundaryError::ExecutableInvalid)
    );
}

#[test]
fn rejects_an_executable_outside_the_allowlist() {
    let boundary = sandbox_boundary();
    assert_eq!(
        boundary.validate_sandbox_command(
            "curl",
            &[],
            "workspace",
            &sandbox_environment(),
        ),
        Err(SandboxLocalHostBoundaryError::ExecutableDenied)
    );
}

#[test]
fn rejects_argument_overruns_and_forbidden_bytes() {
    let boundary = sandbox_boundary();
    let long_argument = "a".repeat(4_097);
    assert_eq!(
        boundary.validate_sandbox_command(
            "toybox",
            &[long_argument.as_str()],
            "workspace",
            &sandbox_environment(),
        ),
        Err(SandboxLocalHostBoundaryError::ArgumentInvalid)
    );
    assert_eq!(
        boundary.validate_sandbox_command(
            "toybox",
            &["bad\nline"],
            "workspace",
            &sandbox_environment(),
        ),
        Err(SandboxLocalHostBoundaryError::ArgumentInvalid)
    );

    let limits = SandboxLocalHostBoundaryLimits {
        sandbox_max_arguments: 1,
        ..SandboxLocalHostBoundaryLimits::default()
    };
    let bounded = sandbox_boundary().with_limits(limits);
    assert_eq!(
        bounded.validate_sandbox_command(
            "toybox",
            &["one", "two"],
            "workspace",
            &sandbox_environment(),
        ),
        Err(SandboxLocalHostBoundaryError::ArgumentCountExceeded)
    );
}

#[test]
fn rejects_working_directory_escapes_and_windows_hazards() {
    let boundary = sandbox_boundary();
    for working_directory in [
        "/etc",
        "\\windows",
        "c:/windows",
        "a/..",
        "a/./b",
        "con.txt",
        "lpt9",
        "trailing.",
        "co:lon",
    ] {
        assert_eq!(
            boundary.validate_sandbox_command(
                "toybox",
                &[],
                working_directory,
                &sandbox_environment(),
            ),
            Err(SandboxLocalHostBoundaryError::WorkingDirectoryInvalid),
            "working directory `{working_directory}` must be rejected"
        );
    }
}

#[test]
fn rejects_environment_entries_that_break_the_boundary() {
    let boundary = sandbox_boundary();
    assert_eq!(
        boundary.validate_sandbox_command(
            "toybox",
            &[],
            "workspace",
            &sandbox_environment_entry("lowercase", "x"),
        ),
        Err(SandboxLocalHostBoundaryError::EnvironmentNameInvalid)
    );
    assert_eq!(
        boundary.validate_sandbox_command(
            "toybox",
            &[],
            "workspace",
            &sandbox_environment_entry("PATH", "/bin"),
        ),
        Err(SandboxLocalHostBoundaryError::EnvironmentProtected)
    );
    assert_eq!(
        boundary.validate_sandbox_command(
            "toybox",
            &[],
            "workspace",
            &sandbox_environment_entry("GITHUB_TOKEN", "x"),
        ),
        Err(SandboxLocalHostBoundaryError::EnvironmentSensitive)
    );
    assert_eq!(
        boundary.validate_sandbox_command(
            "toybox",
            &[],
            "workspace",
            &sandbox_environment_entry("UNLISTED_NAME", "x"),
        ),
        Err(SandboxLocalHostBoundaryError::EnvironmentDenied)
    );
    assert_eq!(
        boundary.validate_sandbox_command(
            "toybox",
            &[],
            "workspace",
            &sandbox_environment_entry("LANG", "bad\nvalue"),
        ),
        Err(SandboxLocalHostBoundaryError::EnvironmentValueInvalid)
    );
}

#[test]
fn rejects_environment_count_overruns() {
    let limits = SandboxLocalHostBoundaryLimits {
        sandbox_max_environment_entries: 1,
        ..SandboxLocalHostBoundaryLimits::default()
    };
    let bounded = sandbox_boundary().with_limits(limits);
    let environment = BTreeMap::from([("LANG", "c"), ("SANDBOX_MODE", "strict")]);
    assert_eq!(
        bounded.validate_sandbox_command("toybox", &[], "workspace", &environment),
        Err(SandboxLocalHostBoundaryError::EnvironmentCountExceeded)
    );
}

#[test]
fn displays_every_error_variant_without_panicking() {
    let variants = [
        SandboxLocalHostBoundaryError::ExecutableInvalid,
        SandboxLocalHostBoundaryError::ExecutableDenied,
        SandboxLocalHostBoundaryError::ArgumentCountExceeded,
        SandboxLocalHostBoundaryError::ArgumentInvalid,
        SandboxLocalHostBoundaryError::WorkingDirectoryInvalid,
        SandboxLocalHostBoundaryError::EnvironmentCountExceeded,
        SandboxLocalHostBoundaryError::EnvironmentNameInvalid,
        SandboxLocalHostBoundaryError::EnvironmentProtected,
        SandboxLocalHostBoundaryError::EnvironmentSensitive,
        SandboxLocalHostBoundaryError::EnvironmentDenied,
        SandboxLocalHostBoundaryError::EnvironmentValueInvalid,
    ];
    for variant in variants {
        assert!(!variant.to_string().is_empty());
    }
}
