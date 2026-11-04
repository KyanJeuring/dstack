# DStack v2 target architecture

This is the approved target direction for DStack v2. The implementation starts
at `2.0.0` and grows incrementally from one Rust package and one `dstack`
executable. Items explicitly described as deferred still require their named
migration-phase proof.

## Architectural goal

Build one small shell-independent executable for stack management, with
minimal adapters for shell-session context and temporary legacy names.

```text
      Bash adapter   Zsh adapter   Fish adapter   PowerShell adapter
             \           |           |           /
              \     session + compatibility     /
               +-------------+------------------+
                             |
                             v
                     dstack executable
          +------------------+-------------------+
          |                  |                   |
          v                  v                   v
     registry/config   target + Compose       Docker CLI
          |              resolution         process boundary
          +------------------+-------------------+
                             |
                             v
                       platform services
                 Linux / macOS / native Windows
```

The executable owns every decision that can be made in a child process. The
adapters do not discover stacks, parse Compose files, or construct Docker argv.

## Design constraints

- One package and one binary initially; no workspace, daemon, plugin system,
  background service, or Docker Engine API.
- Docker is launched directly with an argv vector. No `sh -c`, `eval`, or
  reconstructed command string.
- Paths stay in native path types until displayed or passed to Docker.
- Resolver results and errors are typed; stdout is never an internal data bus.
- Current Bash characterization remains unchanged during dual operation.
- Platform branches exist only for real path, directory, locking, replacement,
  signal, terminal, or shell-integration differences.

## Proposed component responsibilities

The names are conceptual Rust modules. They are not a requirement to create
one file or abstraction per bullet.

### `cli`

- Defines the command tree in `cli-design.md`, global target/file options,
  typed operands, raw Compose trailing argv, help, version, and completions.
- Accepts DStack-owned options around typed subcommands where unambiguous;
  `compose`, `exec`, and `run` honor `--` as an authoritative passthrough
  boundary.
- Rejects conflicting or incomplete syntax before application work.
- Contains an isolated, removable legacy grammar during deprecation.

### `app`

- Routes parsed commands and coordinates use cases.
- Applies the error/status and output policies.
- Contains orchestration such as pull-then-up, while delegating resolution and
  process execution.
- Stops multi-step commands on failure unless an approved option says
  otherwise.

### `model`

Small concrete types prevent accidental mixing:

- `StackName`, `StackRecord`, `ProjectTarget`;
- `TargetSource` (`ExplicitName`, `ExplicitDirectory`, `Session`, `CurrentDir`);
- `ComposeCandidate`, `ComposeSelection`;
- `DockerInvocation` with executable, argv, cwd, environment policy, and I/O
  attachment;
- `Outcome`/error categories and user cancellation.

These are data types, not a large domain framework.

### `config` and `registry`

- Find native user directories and load versioned TOML.
- Keep configuration separate from mutable stack registrations.
- Validate stack names and paths without regular expressions or word splitting.
- Lock read-modify-write operations, use unique same-directory temporary files,
  flush, and replace atomically where the OS permits.
- Expose a read-only legacy registry parser and an explicit migration command.
- Never mutate the legacy file as a side effect of listing or help.

### `discovery`

- Walk configured bases to a documented bounded depth with native filesystem
  APIs.
- Return stable, sorted candidates and enough origin information for useful
  diagnostics.
- Do not follow symlinks recursively by default; avoid cycles. Whether a stack
  root itself is resolved through a symlink is an approved path policy.

### `resolution`

- Apply exactly one target policy for every command: explicit `--stack`/`-s`
  or `--project-dir`, then session context, then exact cwd.
- Resolve registered names before discovered names when an explicit name is
  used, preserving the useful current precedence.
- Treat invalid explicit or session context as an error. Never silently clear
  part of the context and run against another project.
- Return a `ResolvedProject`; never print as part of resolution.

### `compose`

- Discover standard candidates in fixed order and additional matches in a
  stable filename order.
- Parse repeated `--file`, `--all-files`, configured candidate arrays, and TTY
  selection into a validated `ComposeSelection`.
- Apply selection precedence: explicit CLI selection, then session selection,
  then registered-profile defaults, then automatic discovery. Explicit CLI
  files replace lower-priority defaults.
- One file is automatic. Multiple files prompt only when interactive; scripts
  must select explicitly.
- Produce repeated `-f`, path pairs without using output text as an internal
  protocol.

### `docker`

- Locate `docker`/`docker.exe` through native PATH rules.
- Optionally probe `docker compose version` for diagnostics, not at startup for
  unrelated commands.
- Construct and launch direct Docker/Compose invocations.
- Attach stdin/stdout/stderr for interactive and streaming commands; capture
  only bounded probes.
- Forward cancellation/signals using platform-appropriate process behavior.
- Return actual child status and do not suppress Docker stderr.

This boundary is intentionally the Docker CLI rather than the Engine API. It
respects Docker contexts, Desktop, credentials, plugins, and host behavior.

### `terminal`

- Detect terminal input/output, color capability, and noninteractive mode.
- Render selection prompts and any genuinely necessary confirmations to
  stderr. Explicit stack-local destructive flags such as `--volumes` are
  sufficient intent and do not add a redundant confirmation.
- Support optional paging only when appropriate and without making pager status
  replace Docker status.

### `shell`

- Embed versioned templates for Bash, Zsh, Fish, and PowerShell initialization,
  completion, and legacy wrappers.
- Create/delete a per-shell session through a narrow machine interface.
- Optionally export legacy `DSTACK` variables through a fixed structured
  protocol during migration.
- Never contain target, registry, or Compose business logic.

### `platform`

Keep this boundary concrete and narrow:

- config/data/cache/runtime directories;
- user-private session/temp files;
- atomic replacement and file locking;
- executable naming and PATH lookup;
- Unix signal groups versus Windows console/process groups;
- path display/normalization rules;
- platform identity used by diagnostics and release selection.

Do not hide all filesystem operations behind a universal virtual filesystem.
Unit tests can use temporary directories; only OS-different operations need a
platform interface.

## End-to-end command flow

For `dstack --stack web --file compose.prod.yml logs api --follow`:

1. CLI parses target, files, command, service, and flags without probing the
   filesystem to decide token meaning.
2. Resolver obtains the registered/discovered `web` project.
3. Compose resolver validates `compose.prod.yml` relative to that project.
4. Application creates the exact invocation:
   `docker compose -f <native-path> logs -f --tail=100 api`.
5. Process layer starts Docker in the caller's cwd with attached streams.
6. DStack returns Docker's status. Ctrl+C reaches Docker and DStack exits
   consistently after the child.

Raw `dstack ... compose ARGS...` follows the same first three steps, then
appends the trailing argv unchanged.

## Parent-shell state

A child executable cannot mutate its parent. The target design avoids pretending
otherwise.

### Session mechanism

1. A versioned adapter is sourced/dot-sourced at shell startup.
2. It asks the trusted executable to create a unique user-private session file
   in a runtime/temp directory and exports only its controlled pointer, for
   example `DSTACK_SESSION_FILE`.
3. `dstack use` validates the selected stack/files and atomically writes the
   session record. Because the executable can write the referenced file, the
   parent environment need not change for each selection.
4. Subsequent DStack processes read and revalidate the record.
5. `dstack unset` removes/clears it. The adapter attempts cleanup on shell exit;
   stale files also expire safely.

The session record will contain a format version, stack/profile reference,
resolved display path, optional file selectors, and creation/last-use metadata.
Its exact transport and schema are deferred until real shell-adapter tests. It
is never a global default shared by unrelated terminals.

Without an initialized session, `use` reports how to initialize the shell and
suggests one-shot `--stack`. Listing and explicit/local commands still work.

### Legacy environment compatibility

During the compatibility window, the core continues reading user-provided
`DSTACK`, `DSTACK_COMPOSE_FILE`, `DSTACK_BASES`, and
`DSTACK_COMPOSE_FILES` when no new equivalent overrides them. Each use emits a
targeted deprecation warning unless quieted.

If maintainers require `dstack use` to mirror `DSTACK` for arbitrary child
programs, the adapter can intercept only `use`/`unset` and parse a fixed
machine response written to a temporary file:

- Bash/Zsh: NUL-delimited fixed fields with shell-native `read`;
- Fish: NUL splitting with native Fish primitives;
- PowerShell: UTF-8 JSON and `ConvertFrom-Json`.

The adapter assigns only known variable names. It must not `eval` values or
execute registry/path text. This mirror is compatibility behavior; the session
file remains the new core's authority.

## Shell adapters

### Bash

- Installed as a static generated `.bash` file and sourced from `.bashrc`.
- Preserves `set` options, `IFS`, and `shopt`.
- Exports the session pointer and defines bounded legacy functions that call
  `command dstack` with arrays.
- Uses no discovery or Docker logic.

### Zsh

- Native Zsh file sourced from `.zshrc`; never sources Bash implementation.
- Uses Zsh arrays/quoting only for wrapper forwarding and completion.
- Shares the same session protocol and legacy translation table generated by
  the core schema.

### Fish

- Native Fish initialization under the user config/function conventions.
- Uses `set -gx` for the session pointer and Fish-native wrapper/completion
  syntax. No Bash source line or `eval` of user values.

### PowerShell

- Native `.ps1` initialization, installed under an appropriate user data path
  and dot-sourced from the selected PowerShell profile only with consent.
- Uses `$env:DSTACK_SESSION_FILE`, native JSON if a legacy environment mirror
  is enabled, and argument arrays when calling `dstack.exe`.
- Operates on native Windows paths and `docker.exe`; no MSYS conversion.

The adapter templates should be snapshot-tested with hostile names/paths and
smoke-tested in their real shells. Business behavior is tested once through
the executable.

## Registry and configuration design

### Locations

Use a platform directory library rather than hand-building `$HOME`:

- Linux: honor XDG config/data/runtime variables with documented fallbacks;
- macOS: Application Support/config conventions selected by the library;
- Windows: LocalAppData for machine-specific registry paths and state;
- session state: runtime directory or protected temp directory, never the
  durable registry directory.

Exact application/vendor identifiers require approval before code is written.
Registry entries contain machine paths and should not roam between machines by
default.

### Formats

Proposed files:

```text
config.toml
registry.toml
sessions/<random-id>.json
```

`config.toml` owns discovery bases, candidate filenames, color/pager defaults,
and other user policy. Top-level `dstack config` remains reserved until a real
user-facing configuration interface is designed. `registry.toml` is versioned
and contains profile names, native project paths, and optional ordered default
Compose files. Multiple profiles may reference the same project. The exact
schema is a Phase 2 decision. Session state is private machine state, not a
user-edited config.

The registry API validates duplicate names and preserves deterministic listing.
Unknown future fields are either retained or rejected according to an explicit
format-version policy; silently discarding them is unsafe for downgrade.

### Legacy migration

- Detect `$HOME/.config/dstack/registry` on applicable environments.
- `dstack migrate registry --dry-run` reports valid, duplicate, stale, and
  ambiguous entries without writing.
- Approved migration creates a timestamped backup, writes/validates the new
  file under lock, then marks migration completion. It does not delete legacy
  data automatically.
- During coexistence, reads can merge with clear precedence, but only one
  format is write-authoritative. Never dual-write without a transaction.
- Lines with ambiguous extra `=` or control characters require user action;
  do not invent a path/name split.

## Error and status model

Use typed categories internally and stable documented process behavior:

- success;
- user cancellation;
- usage/configuration error;
- target/Compose resolution error;
- Docker child failure, preserving its exit status when representable;
- system/internal error.

Exact DStack-specific numeric codes remain a maintainer decision. No usage or
resolution error may invoke Docker. One-step Docker commands return the child
status. Multi-step commands stop on first failure and return it. A future
`--continue-on-error` must define which status wins before being introduced.

## Output model

- stdout: command data and Docker stdout;
- stderr: DStack diagnostics, warnings, errors, prompts, and Docker stderr;
- terminal color: auto by destination, overridable;
- `--quiet`: hides DStack informational messages, never Docker output/errors;
- machine-readable listing: add only with a defined schema/version, preferably
  `--output json`, not by parsing decorated human tables;
- interactive menus: stderr, with decisions represented internally rather than
  captured from their rendered text.

Errors never become selected file paths. Pager integration is optional and
disabled when stdout is not a terminal.

## Destructive operations

Stack-local removal (`down --volumes`, `recreate --volumes`) is explicitly
requested by the destructive flag and does not require a second confirmation.
The resolved target/files may still be displayed as diagnostics where useful.

Host-wide stop/prune commands are not part of the permanent CLI. The raw
Compose escape hatch is not safety-wrapped beyond normal target validation;
documentation must state that arbitrary Compose argv can be destructive.

## Installation and distribution architecture

Canonical future sources should live in this repository:

```text
scripts/install.sh
scripts/install.ps1
```

They are release inputs, linted and tested in CI. Do not add them before the
installer/release migration phase.

### Release artifacts

Proposed minimum:

```text
dstack-vX.Y.Z-linux-x86_64.tar.gz
dstack-vX.Y.Z-linux-aarch64.tar.gz
dstack-vX.Y.Z-macos-x86_64.tar.gz
dstack-vX.Y.Z-macos-aarch64.tar.gz
dstack-vX.Y.Z-windows-x86_64.zip
dstack-vX.Y.Z-windows-aarch64.zip      # when build and smoke test are reliable
SHA256SUMS
install-dstack.sh
install-dstack.ps1
```

Tags must match the package version. Native CI runners build/test artifacts;
the release remains draft until artifacts/checksums are reviewed. Checksums are
necessary integrity checks, though signatures/provenance can be added later.

### Installer behavior

- Detect OS and CPU with an explicit supported-target table.
- Resolve latest stable or an exact validated semantic version.
- Download a tagged release archive and checksum manifest over HTTPS.
- Verify checksum before extraction or replacement.
- Install at user scope by default (`~/.local/bin` on Unix-like hosts and a
  documented LocalAppData bin directory on Windows).
- Stage and smoke-test `--version`, then atomically switch `current`; retain one
  previous version for rollback.
- Materialize the selected shell adapter and modify a profile only after
  consent, using marker-delimited idempotent blocks and quoted paths.
- Support reinstall, explicit-version update, rollback, and uninstall including
  its own profile block. Do not remove user registry/config on uninstall unless
  explicitly requested.
- Detect Docker for a diagnostic after installation, not as a condition for
  installing help/list/migration functionality.

POSIX `install.sh` supports Linux/macOS. `install.ps1` supports native Windows
without Bash or curl. Git clone/Cargo/source installs remain developer channels.
Package managers can be added later without blocking binary distribution.

`dstack self-update` belongs to this installation boundary, not stack update.
It must respect installation ownership: repository-installer installations can
stage and verify a release; Cargo installs receive the appropriate Cargo path;
package-manager installs are directed back to their package manager. It is not
implemented in Phase 1.

### Website bootstrap role

`https://kyanjeuring.com/scripts/install-dstack` should remain a memorable
stable bootstrap URL but not an independent codebase. Preferred behavior is an
HTTPS redirect to the latest immutable, repository-produced installer asset.
An explicit version should resolve directly to that release's installer. The
README should also show the GitHub release URL and encourage inspection before
execution.

The first remote script remains a trust root. Binary checksum verification
protects the second download, not a compromised bootstrap; signed release
provenance is the later mitigation.

## Lessons from IRIS

Adopt:

- thin `main` plus testable library modules;
- typed Clap CLI/help/version and native path types;
- native config directories and explicit legacy config fallback;
- quality versus build job separation, locked dependencies, native runners;
- tag/version verification, checksummed named artifacts, draft release review;
- installation documentation organized by channel and platform.

Do not copy:

- IRIS's deep TUI/rendering hierarchy or broad dependency graph;
- flat flags suited to a one-action viewer;
- automatic theme/config materialization;
- Cargo/crates.io publication as a launch requirement;
- all Linux package formats before the archive/installer path is proven;
- Unix Makefile installation as a cross-platform installer;
- Linux `/proc` special behavior except as an example of isolating optional
  platform enhancement.

IRIS's release matrix currently demonstrates all proposed primary artifacts
except Windows ARM64. DStack should add that target only when it can be built
and smoke-tested, not merely cross-compiled and advertised.

## Adversarial architecture review

The following attacks were applied to this proposal and changed or constrained
the design:

- **Bash leakage:** legacy env parsing is compatibility-only; target parsing and
  Compose selection remain core-owned.
- **Fake native Windows:** the Windows path uses an MSVC executable,
  PowerShell, AppData, Win32 process semantics, and `docker.exe`; Git Bash is a
  separate compatibility environment.
- **Shell duplication:** adapters share a versioned protocol and generated
  command map; they contain only unavoidable syntax.
- **Unsafe generated code:** the primary session design writes a file and does
  not eval registry/path values. Optional legacy env mirroring parses a fixed
  NUL/JSON schema with known variable names.
- **Command injection:** Docker always receives argv. Raw Compose passthrough
  remains argv, even when arguments contain shell syntax.
- **Path corruption:** native path values are not converted to UTF-8 strings
  until presentation; Windows canonical prefixes are not passed blindly.
- **Registry race/downgrade:** one write-authoritative version, locking, unique
  temps, atomic replacement, backups, and unknown-version refusal are required.
- **Installer trust:** source moves into the repository; release assets are
  immutable/checksummed; the website is only a stable pointer.
- **Broken shell semantics:** session files solve child-to-parent mutation
  without global cross-terminal context. Adapter cleanup is best effort, so
  stale files also expire.
- **Over-engineering from IRIS:** one package, shallow modules, concrete types,
  no async/workspace/plugin/package matrix initially.
- **Under-engineering around statuses:** typed errors and direct child status
  replace stdout/process-substitution protocols and `|| true`.
- **Compatibility becoming permanent:** legacy grammar and wrappers are
  isolated, version-gated, warned, and assigned removal criteria in the
  migration plan.

Remaining risks are native Windows process/locking details, safe registry
migration for malformed legacy lines, shell-adapter quoting across real shell
versions, and release bootstrap trust. Each has a named migration gate rather
than being deferred to a flag-day release.
