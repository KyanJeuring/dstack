# DStack v2 decisions

This record is the authoritative high-level product direction for DStack v2.
The detailed rationale and migration mechanics live in the companion
architecture and migration documents. Implementation details explicitly marked
deferred there are not approved merely because the overall direction is.

## Product and implementation

- DStack v2 starts at version `2.0.0`. It is a major redesign, not a preview
  numbered below 1.0 and not an extension of the v1 version line.
- The crates.io package name is `dstack-compose`; the package produces the
  single executable `dstack`. The name was available when selected, but it is
  not reserved until the first approved publication.
- Rust is the implementation language. The initial project is one Cargo
  package and one `dstack` executable.
- Keep the implementation synchronous, dependency-light, and shallow. There is
  no workspace, plugin framework, daemon, background service, Docker Engine
  API, or async runtime without a later concrete requirement.
- Linux, macOS, and native Windows are product platforms. Git Bash and WSL are
  compatibility environments, not substitutes for native Windows.
- The characterized Bash implementation remains the v1 oracle during
  migration. Intentional v2 divergences are recorded separately rather than
  changing v1 expectations.

## CLI and targeting

- The permanent interface is one executable named `dstack`; 43 executable
  aliases will not be installed.
- Stack targeting is explicit with `--stack NAME` or `-s NAME`. There is no
  permanent positional `dstack STACK COMMAND` grammar.
- For stack operations, context precedence is explicit stack, current terminal
  session, exact current directory when it is a Compose project, then a
  helpful nonzero error. Parent directories are not searched implicitly.
- DStack-owned options are accepted before or after typed subcommands where
  unambiguous. A command's `--` is a hard passthrough boundary; all later argv
  are preserved exactly.
- `dstack compose [ARGS...]` is raw Compose passthrough after DStack resolves
  the project and files. Its optional leading `--` is consumed as the hard
  boundary. Typed-command defaults are never added to raw Compose argv.

## Typed operations

- Core typed operations include `up`, `start`, `stop`, `restart`, `pull`,
  `update`, `rebuild`, `down`, and `recreate`. Service operands narrow an
  operation; no service means the whole stack where Compose supports it.
- `start`, `stop`, and `up` remain distinct Compose operations.
- Typed `up` defaults to detached build behavior, approximately
  `up -d --build`. Orphan removal is opt-in with `--remove-orphans`.
- Permanent duplicate families collapse into modifiers: `down --volumes`,
  `rebuild --no-cache`, and `recreate [--volumes]`.
- `restart` maps to Compose restart. `recreate` performs down followed by the
  typed up workflow. Multi-step commands stop on first failure.
- Explicit destructive flags express user intent. `down --volumes` and
  `recreate --volumes` do not add a redundant DStack confirmation. Stack-local
  and host-wide scopes are never mixed.
- Historical generic Docker helpers and host-wide stop/prune commands are not
  permanent v2 core commands.
- Logs consolidate under `dstack logs [SERVICE]` with modifiers.
- Explicit exec form is `dstack exec SERVICE -- COMMAND...`. When COMMAND is
  omitted, a later Docker phase will define and test a compatible-container-
  shell probe; Phase 1 does not choose that policy.
- `dstack update` updates the selected Compose stack. `dstack self-update`
  updates DStack through its owning installation channel; implementation is a
  later phase.

## Listing, registry, Compose files, and session

- `dstack list` shows the effective registered and discovered stacks and their
  source. `dstack registry list|add|remove` changes persistent registration
  only.
- `dstack use NAME`, `dstack unset`, and `dstack context show` operate on the
  current terminal session. `unset` does not unregister a stack, and the
  interface does not use the name `unuse`.
- Registered names are profiles. Multiple names may point to the same project,
  and each profile may store ordered default Compose files.
- Compose selection precedence is explicit CLI `--file`/`--all-files`, then
  session selection, then registered-profile defaults, then automatic
  discovery. An explicit CLI selection replaces rather than appends to lower
  precedence selections.
- Docker Compose configuration is `dstack compose config`. Top-level
  `dstack config` is reserved for future DStack configuration and is not added
  until it has a concrete interface.
- The per-shell private session-file design remains the approved direction,
  but its transport and schema are deferred until real Bash, Zsh, Fish, and
  PowerShell adapter tests exist.
- The registry will use structured versioned storage in native platform
  locations with safe migration from v1. Its exact TOML schema and platform
  identifiers remain Phase 2 decisions.

## Errors, output, distribution, and migration

- Usage, validation, target-resolution, and Compose-resolution failures are
  nonzero. Docker failures propagate meaningful failure status.
- DStack diagnostics use stderr; command data and Docker stdout use stdout;
  Docker stderr remains stderr. Errors can never become file selections.
- Docker is always invoked directly with an argv vector, never a reconstructed
  shell command.
- Repository-owned POSIX and PowerShell installers, immutable checksummed
  GitHub Release artifacts, native CI runners, staged replacement, rollback,
  and an installation-channel-aware self-update are the approved distribution
  direction. No installer or release is part of Phase 1.
- The stable `kyanjeuring.com` URL should become a bootstrap/redirect to
  repository-produced immutable installer content rather than remain an
  independent mutable implementation.
- Legacy stack-aware `d*` shell wrappers may exist for a bounded migration
  period. Their exact removal release and optional generic-Docker compatibility
  pack remain later maintainer gates.

## Phase 1 boundary

Phase 1 creates the Rust `2.0.0` package, typed help/version foundation,
quality tests, native build checks, and dormant CI/CD scaffolding. It does not
implement Docker, registry/configuration, discovery/resolution, Compose
selection, shell/session adapters, installers, releases, or self-update, and
it creates no tag or published artifact.
