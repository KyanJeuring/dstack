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
