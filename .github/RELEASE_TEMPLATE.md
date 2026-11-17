# DStack v2.0.0

DStack v2.0.0 makes the Rust implementation the supported production DStack.
This is an intentionally breaking CLI redesign, not a drop-in-compatible
replacement for the v1 Bash functions.

## Highlights

- Native Rust executable for Linux, macOS, and Windows.
- Persistent stack registry plus deterministic project discovery.
- Terminal-local context through `DSTACK_SESSION`, `dstack use`, and
  `dstack unset`.
- Managed Bash, Zsh, Fish, and PowerShell integration with completion.
- Typed Compose lifecycle, services, logs, exec, and removable one-off run
  workflows, plus exact raw Compose forwarding.
- Host-scoped container, image, volume, network, disk-usage, prune, and
  graceful stop commands with explicit destructive scope.
- Checksum-verified standalone self-update for compatible stable v2 releases.

## Breaking migration

DStack v1.11.0 is the final legacy Bash release. Installing v2 does not remove
v1, and a sourced v1 `dstack()` function may shadow the new executable.

Follow the complete
[v1-to-v2 migration guide](https://github.com/KyanJeuring/dstack/blob/v2.0.0/MIGRATING_V1_TO_V2.md)
before removing legacy files. The supported registry import is:

```text
dstack migrate registry --dry-run
dstack migrate registry
```

Old helper names are not retained as compatibility aliases. The migration
guide maps common workflows, explains Compose/session changes, and documents
rollback.

## Installation and release assets

This release contains five standalone archives:

- Linux x86_64 and ARM64
- macOS Intel and Apple Silicon
- Windows x86_64

It also includes stable Unix and PowerShell installer assets, direct `.deb`,
`.rpm`, and Arch Linux packages for both Linux architectures, checksum
sidecars for every asset, and the aggregate `SHA256SUMS` manifest.

Standalone installations can use `dstack self-update`; system-package and
Cargo-managed installations remain owned by their respective package manager.

## Safety and integrity

Installers and self-update use HTTPS release downloads and mandatory SHA-256
verification. Installers do not edit shell profiles, invoke Docker, or delete
legacy v1 files. Run `dstack shell install` explicitly after binary
installation.
