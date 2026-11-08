# Contributing to DStack

Thanks for your interest in contributing to **DStack**

## Philosophy
DStack is designed to be:
- Simple
- Predictable
- Shell-first
- Zero dependencies (besides Docker)

Please keep changes aligned with these goals.

---

## How to contribute

### 1. Fork the repository

Create a fork and clone it locally.

### 2. Create a feature branch

```bash
git checkout -b feature/my-feature
```

### 3. Make your changes

Keep functions POSIX-friendly where possible

Avoid unnecessary external dependencies

Prefer readability over cleverness

### 4. Test manually

Please test:

- Inside a project directory
- Outside a project directory
- With and without registered stacks

### 5. Submit a Pull Request

Use the PR template and describe:

What you changed

Why it’s useful

Any breaking changes

---

## Code style

Bash (not zsh-specific)

2 spaces indentation

Functions prefixed with _ are internal

Public commands must be documented in README

---

## Versioning

This project uses semantic versioning:

MAJOR – breaking changes

MINOR – new features

PATCH – bug fixes

## DStack v2 development

The supported runtime is still the characterized Bash v1 implementation.
DStack v2 is an in-progress Rust migration that starts at version `2.0.0`.
Changes must keep the v1 characterization suite green and record approved v2
divergences separately rather than rewriting v1 expectations.

Rust validation uses:

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --all-features --locked
cargo build --locked
```

Phase 6 adds namespaced host-scoped Docker container, image, volume, and network
utilities. They must stay independent from stack discovery, registry lookup,
Compose invocation construction, and `ResolvedContext`. Bounded metadata
commands use the generic captured-output process interface; live stats,
inspection, one-container port lookup, and explicit volume removal retain the
streaming executor and inherited stdio. Do not add shell pipelines, legacy
abbreviated aliases, a generic `dstack docker` passthrough, or host-wide prune
and stop-all operations as part of this phase.

Phase 7 adds terminal-only active context through the versioned
`DSTACK_SESSION` environment payload and thin Bash, Zsh, Fish, and PowerShell
adapters. Registry persistence, terminal session state, and shell rendering
must remain separate. Stack resolution is explicit `--stack`, then session,
then exact cwd. Session parsing must stay lazy so malformed state cannot affect
explicit targets, effective listing, or host-scoped namespaces. Shell mutation
stdout is machine-consumable code; diagnostics belong on stderr. Never write a
session file, inspect legacy `DSTACK`/`DSTACK_COMPOSE_FILE` as v2 state, or put
registry/discovery/Compose logic into an adapter.

Phase 7.1 makes adapter setup practical through public `shell install`,
`shell status`, and `shell uninstall` commands. The adapter generator and
mutation endpoints remain hidden as `shell __init`, `shell __activate`, and
`shell __clear`. Shell detection belongs in the shell subsystem and must keep
explicit `--shell` authoritative; profile resolution and strict managed-block
editing must preserve unrelated content, symlinks, permissions, and practical
line endings. Unknown detection and malformed markers are safe failures.
Generated adapters scope `DSTACK_SHELL_ADAPTER` to delegated child processes
so status can distinguish an installed profile from a loaded adapter.

Phase 7.2 treats shell-profile integration ownership as part of the domain.
The profile analyzer must distinguish DStack-managed blocks, canonical manual
loaders, legacy `shell init` manual loaders, duplicates, and corrupt markers.
Manual loader scanning excludes the managed block span and recognizes only the
documented shell-specific forms, not arbitrary equivalent shell programs.
DStack may replace or remove its marked managed block, but manually authored
loaders are user-owned: install/status/uninstall may detect and explain them
but must never rewrite, remove, adopt, or normalize them automatically.

Shell management routes before DStack configuration bootstrap. Tests must use
temporary `--profile` paths and must never edit a developer or CI runner's real
shell profile. Hidden adapter stdout remains machine-consumable source. Profile
installation is not the Phase 9 DStack binary installer and must not alter the
external production installer or implement release migration.

Phase 7.1 does not add host lifecycle/prune expansion, self-update, production
installer changes, completion frameworks, or final cutover.

Phase 8.1 completes the targeted host-resource lifecycle surface with explicit
container, image, and network removal plus image inspection. These commands
remain context-, session-, registry-, discovery-, Compose-, and configuration-
independent. Explicit names are sufficient intent: handlers preserve operand
order, inherit Docker stdio, return Docker status, and never add force, stop,
disconnect, volume-removal, or prune behavior.

Phase 8.1 also adds one shared Compose state inspector for `start` and `stop`.
It uses silent captured probes against the already resolved project and file
selection, retains per-container replica state, and filters Compose one-off
containers. Probes are best-effort UX optimization only. A failed, malformed,
incomplete, unknown, or unsupported result must produce no diagnostic and must
fall back to the unchanged lifecycle invocation; Docker Compose remains the
authority. Do not copy this shortcut to `up`, `down`, or another lifecycle
command without separately defining and approving its semantics.

Phase 8.2 adds one focused system-wide inspection workflow under the Docker
module. It performs `enumerate -> ordered deduplication -> bounded inspect
batches -> JSON-array aggregation`. Container enumeration includes stopped
containers, image enumeration includes intermediate images, volume enumeration
uses the normal all-volume listing, and network enumeration includes every
network Docker reports. Identifier parsing is line-oriented, accepts LF/CRLF
and a final unterminated line, rejects malformed identifiers, and never shell
splits captured text. Duplicate image IDs—and any duplicate identifiers from
another list—are inspected once at their first-seen position.

All-resource inspection batches at most 64 concrete targets per Docker inspect
invocation so argv growth stays conservative on Unix and Windows. Every batch
must succeed and parse as a JSON array before DStack writes one pretty aggregate
array to stdout. An empty enumeration writes `[]`; an enumeration or inspect
child failure returns that Docker status; spawn/wait and JSON-shape failures
remain typed DStack errors. Docker stderr is inherited because these are
requested primary operations, unlike Phase 8.1's intentionally silent state
probes. The enumerate-then-inspect snapshot has an unavoidable race: if a
resource disappears, Docker's concrete inspect failure is returned without a
retry or partial JSON.

Explicit named inspect commands deliberately retain direct inherited-stdio
Docker execution. They do not enumerate, capture, reformat, or aggregate native
output. `container ip --all` reuses the shared all-container inspection data,
then validates a typed inspect shape and renders one row per container/network
membership. It removes only Docker's leading name slash, renders missing IPv4
or network membership as `-`, and sorts by container name then network name.
The one-container IP command keeps its existing lightweight formatted inspect.

The Phase 8.2 read-only host-command audit is:

| Command | `--all` decision |
|---|---|
| `container list` | Already supports all container states |
| `container inspect` | Added; inspect every container |
| `container ip` | Added; aggregate every container/network membership |
| `container ports` | Not added; no operand is already the running-container overview |
| `container stats` | Already supports Docker's native all-container mode |
| `image list` | Already supports all/intermediate images |
| `image inspect` | Added; inspect every unique image ID |
| `volume list` | Inherently lists all volumes |
| `volume inspect` | Added; inspect every volume |
| `network list` | Inherently lists all networks |
| `network inspect` | Added; inspect every network Docker reports |

Do not infer further host commands from that matrix. Phase 8.2 does not add
resource prune, system maintenance, stop-all, completion generation, manpages,
or update/installer behavior.

Rust tests and manual development commands must set
`DSTACK_CONFIG_DIR` to a temporary directory so they never read or write a
developer's real v1 or v2 state. The override contains `dstack.conf` and
`registry.toml`; its `legacy/registry` child remains the isolated v1 migration
fixture. Automated Rust tests inject a recording process executor and must
never require a Docker daemon, Docker Desktop, containers, or network pulls.
Interactive shell probes use null stdio; final Compose and direct Docker
streaming operations inherit the user's terminal and preserve exact native
arguments. Captured-process fakes must cover stdout bytes, numeric status,
spawn errors, the normal null-stdin/captured-stdout/inherited-stderr policy,
and the silent null-stdin/captured-stdout/null-stderr probe policy. No automated
test may require a Docker daemon.

The same developer workflows are available through `make fmt-check`,
`make lint`, `make test`, `make build`, and `make ci` on Unix-like systems.
