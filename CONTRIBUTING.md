# Contributing to DStack

DStack v2 is the supported Rust implementation. Changes should keep the CLI
predictable, preserve exact process arguments, and make destructive Docker
scope explicit.

## Workflow

1. Fork and clone the repository.
2. Create a focused branch.
3. Add tests for behavior changes.
4. Run the validation gates below.
5. Open a pull request describing the change, motivation, and any breaking or
   destructive behavior.

Do not test cleanup or mutation commands against a real Docker daemon for
routine validation. Use isolated homes/config roots and guarded fake processes.

## Rust conventions

- Format with `cargo fmt`.
- Keep Clippy warning-free across all targets and features.
- Prefer typed command models and exact native argv over shell construction.
- Keep stack-scoped Compose commands separate from host-scoped Docker
  namespaces.
- Preserve `DSTACK_SESSION` as terminal-only adapter state; do not persist a
  session file or interpret v1 `DSTACK`/`DSTACK_COMPOSE_FILE` as v2 state.
- Do not add legacy v1 command aliases. Migration documentation is the
  compatibility path.
- Shell-profile tests must use temporary explicit profiles and never touch a
  developer's real profile.

## Validation

Run the production Rust gates:

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --all-features --locked
cargo build --release --locked
cargo package --locked
make ci
```

Until the production v2 cutover and external installer transition have been
proven, the tracked v1.11.0 runtime remains a release oracle. Also run:

```bash
bash tests/characterization/run.sh all
bash tests/characterization/run.sh interactive
```

The suite must report 179/179 noninteractive and 11/11 PTY cases. This
temporary oracle is retired only after the public v2 release, package channels,
latest installer, self-update, and historical production installer endpoint
are verified.

Installer changes also require:

```bash
shellcheck -x install/install.sh
sh -n install/install.sh
install/install.sh --version v2.0.0 --install-dir /tmp/dstack-bin --dry-run
```

Use native Windows CI for PowerShell parsing, dry-run, shell integration, and
completion. The binary installers must not edit profiles or PATH; profile
integration belongs to `dstack shell install`.

## Documentation

Document public commands in `README.md`. Breaking v1 migration details belong
in `MIGRATING_V1_TO_V2.md`. Private architecture and phase records under
`docs/` remain ignored and must not be committed.

## Releases

Versions follow semantic versioning. Release tags must match `Cargo.toml`.
The release workflow must remain draft-first, build all supported native
archives and Linux packages, attach checksum sidecars plus `SHA256SUMS`, and
pass independent artifact validation before publication.
