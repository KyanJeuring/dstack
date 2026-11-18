use std::fs;

fn source(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|error| panic!("could not read {path}: {error}"))
}

#[test]
fn release_workflow_contains_the_complete_standalone_and_installer_contract() {
    let workflow = source(".github/workflows/release.yml");
    for required in [
        "linux-x86_64",
        "linux-aarch64",
        "macos-x86_64",
        "macos-aarch64",
        "windows-x86_64",
        "install-dstack.sh",
        "install-dstack.sh.sha256",
        "install-dstack.ps1",
        "install-dstack.ps1.sha256",
        "SHA256SUMS",
        "sha256sum --check",
        "draft: true",
    ] {
        assert!(
            workflow.contains(required),
            "release workflow omitted {required}"
        );
    }

    let validate = workflow.find("Validate release asset contract").unwrap();
    let aggregate = workflow.find("Create aggregate checksum manifest").unwrap();
    let draft = workflow.find("Create draft release").unwrap();
    assert!(validate < aggregate && aggregate < draft);
}

#[test]
fn all_official_linux_packages_own_the_same_marker() {
    let workflow = source(".github/workflows/release.yml");
    let nfpm = source("packaging/nfpm.yaml");
    let marker = source("packaging/install-method");

    assert_eq!(marker, "system-package\n");
    assert!(workflow.contains("$PACKAGE_ROOT/usr/share/dstack-compose/install-method"));
    assert!(workflow.contains("install -Dm644 packaging/install-method"));
    assert!(nfpm.contains("src: packaging/install-method"));
    assert!(nfpm.contains("dst: /usr/share/dstack-compose/install-method"));
}

#[test]
fn release_stays_draft_first_and_crate_publish_stays_publication_triggered() {
    let release = source(".github/workflows/release.yml");
    let publish = source(".github/workflows/publish-crate.yml");
    assert!(release.contains("draft: true"));
    assert!(!release.contains("draft: false"));
    assert!(publish.contains("types:\n      - published"));
    assert!(publish.contains("startsWith(github.event.release.tag_name, 'v2.')"));
    assert!(publish.contains("cargo publish --locked --package dstack-compose"));
}

#[test]
fn standalone_archives_remain_binary_only() {
    let workflow = source(".github/workflows/release.yml");
    let unix_archive =
        "tar -C \"target/${{ matrix.target }}/release\" -czf \"dist/$ARCHIVE\" dstack";
    assert!(workflow.contains(unix_archive));
    assert!(!workflow.contains("tar -C packaging"));
}

#[test]
fn updater_configuration_keeps_mandatory_integrity_and_unattended_gates() {
    let cargo = source("Cargo.toml");
    let backend = source("src/update/backend.rs");

    assert!(cargo.contains("\"checksums\""));
    assert!(backend.contains(".checksum_from_asset(CHECKSUM_ASSET)"));
    assert!(backend.contains(".check_install_path_writable(true)"));
    assert!(backend.contains(".unattended()"));
    assert!(!backend.contains(".verify_release_digest(false)"));
}

#[test]
fn production_readme_presents_v2_as_current() {
    let readme = source("README.md");

    assert!(readme.contains("DStack v2 is the current supported release"));
    assert!(readme.contains("DStack v1.11.0 is the final legacy Bash release"));
    assert!(readme.contains("Linux, macOS, and Windows"));
    assert!(readme.contains("Bash, Zsh, Fish, and PowerShell"));
    assert!(!readme.contains("v1 is the supported production release"));
    assert!(!readme.contains("v2 is in development"));
    assert!(!readme.contains("do not replace v1"));
}

#[test]
fn release_template_remains_generic_and_reusable() {
    let template = source(".github/RELEASE_TEMPLATE.md");

    for required in [
        "## What's Changed",
        "## Details",
        "### Added",
        "### Changed",
        "### Fixed",
        "## Why this matters",
        "Before publishing:",
    ] {
        assert!(
            template.contains(required),
            "release template omitted reusable section {required}"
        );
    }

    for release_specific in ["# DStack v2.0.0", "DStack v2.0.0 makes"] {
        assert!(
            !template.contains(release_specific),
            "release template contains release-specific text: {release_specific}"
        );
    }
}

#[test]
fn tracked_migration_guide_handles_registry_shadowing_and_session_changes() {
    let guide = source("MIGRATING_V1_TO_V2.md");

    for required in [
        "dstack migrate registry --dry-run",
        "dstack migrate registry",
        "function named `dstack`",
        "may still be active",
        "DSTACK_COMPOSE_FILE",
        "DSTACK_SESSION",
        "dstack use NAME",
        "dstack container stop --all",
        "v1.11.0 tag and release",
    ] {
        assert!(
            guide.contains(required),
            "migration guide omitted {required}"
        );
    }
}

#[test]
fn installers_warn_about_v1_shadowing_without_deleting_legacy_files() {
    let unix = source("install/install.sh");
    let powershell = source("install/install.ps1");

    for installer in [&unix, &powershell] {
        assert!(installer.contains("legacy v1 Bash integration may still shadow"));
        assert!(installer.contains("MIGRATING_V1_TO_V2.md"));
        assert!(installer.contains("migrate registry --dry-run"));
        assert!(installer.contains("Legacy v1 files and shell-profile lines were left unchanged"));
    }

    assert!(!unix.contains("rm -rf \"$HOME/.local/share/dstack\""));
    assert!(!powershell.contains("Remove-Item -LiteralPath $Legacy"));
}

#[test]
fn legacy_registry_migration_remains_a_supported_rust_path() {
    let paths = source("src/config/paths.rs");
    let migration = source("src/migration/registry.rs");

    assert!(paths.contains("application_dir.join(\"registry\")"));
    assert!(migration.contains("pub fn migrate_registry"));
    assert!(migration.contains("The source is never modified"));
}
