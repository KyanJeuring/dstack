<img src="./logo/dstack-logo.png" alt="DStack">

![CI](https://github.com/KyanJeuring/dstack/actions/workflows/ci.yml/badge.svg)

# DStack

Docker Compose stack management from anywhere.

DStack v2 is the current supported release. It is implemented in Rust, the
executable is `dstack`, and its Cargo/system package name is
`dstack-compose`. DStack v1.11.0 is the final legacy Bash release; v2 is an
intentionally breaking CLI redesign, not a drop-in replacement.

## Install

### Linux and macOS

Install the latest stable standalone release from the GitHub Release asset:

```bash
curl -fsSL \
  https://github.com/KyanJeuring/dstack/releases/latest/download/install-dstack.sh \
  | sh
```

The installer verifies the archive's SHA-256 checksum and installs to
`~/.local/bin` by default. Follow its printed PATH guidance if that directory
is not already on PATH, then install terminal integration:

```bash
dstack shell install
```

Pin the first v2 release through the installer's supported environment
contract:

```bash
curl -fsSL \
  https://github.com/KyanJeuring/dstack/releases/latest/download/install-dstack.sh \
  | DSTACK_VERSION=v2.0.0 sh
```

### Windows

Run the published PowerShell installer from a native PowerShell session:

```powershell
$installer = Join-Path $env:TEMP 'install-dstack.ps1'
Invoke-WebRequest `
  -Uri 'https://github.com/KyanJeuring/dstack/releases/latest/download/install-dstack.ps1' `
  -OutFile $installer
& $installer
Remove-Item $installer
dstack shell install
```

The installer verifies the archive checksum and installs
`$HOME\.local\bin\dstack.exe` by default. Add the exact directory it prints to
PATH first when necessary.

### Packages and archives

Each GitHub Release provides:

- standalone archives for Linux x86_64/ARM64, macOS Intel/Apple Silicon, and
  Windows x86_64;
- direct `.deb` packages for amd64 and arm64;
- direct `.rpm` packages for x86_64 and aarch64;
- direct Arch Linux packages for x86_64 and aarch64;
- `SHA256SUMS` plus a checksum sidecar for every asset.

These are release files, not hosted APT, DNF, Homebrew, Chocolatey, Winget, or
Scoop repositories. The crate package is named `dstack-compose`; its Cargo
installation command will be documented after version 2.0.0 is visible on
crates.io.

## Quick start

```bash
# Discover projects and show registered profiles.
dstack list

# Register a project explicitly.
dstack registry add media ~/projects/media

# Select it for this terminal (requires shell integration).
dstack use media

# Inspect the exact resolved project and Compose files.
dstack context show

# Start the stack, inspect logs, then clear the terminal selection.
dstack up
dstack logs --follow
dstack unset
```

Without `--stack` or an active session, stack commands use the exact current
directory when it contains a configured Compose file. DStack never walks up to
a parent directory.

## Stacks, discovery, and registry

`dstack list` combines persistent registry profiles with discovered projects.
Discovery checks direct children of the configured roots; it does not recurse
arbitrarily. Exact registered names are authoritative, and ambiguous discovered
names must be resolved by registering a unique name.

```bash
dstack registry list
dstack registry show media
dstack registry add media /srv/media --file compose.yml
dstack registry remove media
```

Removing a profile does not touch the project. Repeated `--file` options store
an ordered multi-file profile.

## Terminal context

`dstack use NAME` selects a resolved snapshot for the current terminal only.
`dstack unset` clears it. The shell adapter stores this state in the versioned
`DSTACK_SESSION` environment variable; it does not write a session file or
change the registry.

Resolution order for stack-scoped commands is:

1. explicit `--stack NAME`;
2. the current terminal's `DSTACK_SESSION`;
3. the exact current directory.

Use `dstack context show` to see the effective source, project path, and
ordered Compose files.

## Compose files and lifecycle

Automatic Compose-file selection checks the configured filenames in order and
uses the first existing file. Explicit repeated `--file FILE` options replace
profile/default selection and preserve their order.

Typed lifecycle commands are the preferred interface:

```text
dstack up [--remove-orphans] [SERVICE...]
dstack start|stop|restart|pull [SERVICE...]
dstack down [--volumes]
dstack recreate [--volumes]
dstack update [--remove-orphans] [SERVICE...]
dstack rebuild [--no-cache] [SERVICE...]
```

`up` uses detached, build-aware defaults. Multi-step operations are fail-fast,
and Docker Compose remains authoritative for output and exit status.
`update` pulls the requested service images, then runs Compose `up -d`; Compose
recreates services whose image or configuration changed. `--remove-orphans`
also removes project containers for services no longer defined by the current
Compose model. It does not prune images or unrelated host resources.

For Compose features outside the typed surface, use raw forwarding:

```bash
dstack compose ps
dstack --stack media compose config
dstack compose -- --file special.yaml up
```

The hard `--` boundary protects raw arguments that look like DStack options.
DStack forwards exact OS arguments and never reconstructs a shell command.

## Services, logs, exec, and run

```bash
dstack services
dstack logs --tail 50 web
dstack logs --follow web worker
dstack exec web
dstack exec web -- env
dstack run worker
dstack run app -- sh -c 'echo hello'
```

Logs show 100 lines by default and do not follow unless requested. With no
explicit command, `exec` probes common interactive shells. An explicit command
for `exec` or `run` requires `--`; `run` always uses Compose `--rm`.

## Host Docker commands

The `container`, `image`, `volume`, `network`, and `system` namespaces call the
Docker CLI directly and do not resolve a DStack project:

```text
dstack container list|ports|ip|stats|inspect|remove|stop|prune
dstack image list|inspect|remove|prune
dstack volume list|inspect|remove|prune
dstack network list|inspect|remove|prune
dstack system df|prune
```

Use each command's `--help` for its typed options. `inspect --all` is explicit,
and removal commands act only on named resources.

## Maintenance safety

Prune commands delegate eligibility, confirmation, output, and exit status to
Docker. They do not add `--force` automatically. Use long-form `--force` only
when intentionally skipping Docker's confirmation.

```bash
dstack container prune
dstack image prune --all
dstack volume prune --all --force
dstack network prune --filter until=24h
dstack system df --verbose
dstack system prune --all --volumes
```

`dstack container stop --all` is host-wide and stops every currently running
container visible to the active Docker daemon/context. `dstack system prune`
also affects Docker data unrelated to registered DStack projects.

## Shell integration and completion

One managed profile block enables both terminal session handling and static
completion for Bash, Zsh, Fish, or PowerShell:

```bash
dstack shell install
dstack shell status
dstack shell uninstall
```

Use `--shell bash|zsh|fish|powershell` when automatic detection is ambiguous,
and `--profile PATH` for a custom profile. `--dry-run` previews profile changes.
Installation preserves unrelated content; uninstall removes only DStack's
managed block and never removes a user-authored manual loader.

## Configuration

```bash
dstack config path
dstack config show
```

The default configuration root is `~/.config/dstack`. Set
`DSTACK_CONFIG_DIR` to replace it completely, which is useful for isolated
automation and tests. `dstack.conf` is strict TOML with ordered discovery
roots and Compose filenames. The default roots are `~/projects`, `~/src`,
`~/code`, and `/opt/services` on Unix; the default filenames are
`docker-compose.yml`, `docker-compose.yaml`, `compose.yml`, and `compose.yaml`.

## Self-update and installation ownership

```bash
dstack self-update --check
dstack self-update
```

Self-update accepts only compatible stable v2 GitHub releases and verifies
checksums before replacing a standalone binary. Installation ownership is
deliberate:

- standalone install: use `dstack self-update`;
- `.deb`, `.rpm`, or Arch package: update through the system package manager;
- Cargo-managed install: update through Cargo, not `dstack self-update`.

No update check invokes Docker or changes shell profiles.

## Migrating from v1

DStack v1.11.0 is the final Bash release. Installing the v2 executable does
not disable the old sourced `dstack()` function, which may continue to shadow
the binary. Follow [MIGRATING_V1_TO_V2.md](./MIGRATING_V1_TO_V2.md) before
removing legacy files. The supported registry import is:

```text
dstack migrate registry --dry-run
dstack migrate registry
```

The historical [v1.11.0 release](https://github.com/KyanJeuring/dstack/releases/tag/v1.11.0)
remains available for migration and rollback reference.

## Platform support

DStack v2 supports native Linux, macOS, and Windows builds. Shell integration
and completion support Bash, Zsh, Fish, and PowerShell. WSL and Git Bash may be
useful compatibility environments, but native Windows does not require them.
Docker CLI with Compose v2 is required for Docker-facing commands.

## Uninstall

First remove only the managed shell integration:

```bash
dstack shell uninstall
```

Then remove the binary through the channel that installed it: delete the
standalone `dstack`/`dstack.exe`, uninstall `dstack-compose` with the system
package manager, or use `cargo uninstall dstack-compose`. Configuration and
registry data are retained unless you explicitly choose to remove them.

## Development

See [CONTRIBUTING.md](./CONTRIBUTING.md). The main local quality gate is:

```bash
make ci
```

Release CI additionally validates native Linux, macOS, and Windows builds,
standalone installers, packages, checksums, and draft-first release assets.
