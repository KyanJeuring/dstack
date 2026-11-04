# DStack v2 migration plan

This is the approved incremental direction for migrating from characterized
DStack v1 to Rust-based DStack v2, beginning at version `2.0.0`. Each phase is
still gated independently; approval of the plan is not approval to skip a
phase's tests, compatibility work, or review.

## Migration principles

- `dstack.sh` remains usable and its 190 characterization cases remain the
  current-behavior oracle throughout dual implementation.
- New behavior is introduced beside Bash and selected explicitly until it has
  passed its gate.
- Approved intentional differences are recorded in a separate implementation-
  divergence ledger; current expected results are not rewritten to make a new
  implementation pass.
- Every phase is independently releasable or reversible. No phase requires a
  flag-day replacement.
- Docker tests remain fake/guarded by default. Real Docker checks, if ever
  added, are opt-in and isolated.
- CLI, state, installer, and retirement decisions are approved before users are
  asked to migrate.

## Test taxonomy during coexistence

Existing cases should be tagged conceptually, without changing their current
assertions:

1. **Cross-implementation behavior:** registry/discovery goals, explicit >
   session > local precedence where approved, Compose ordering, exact Docker
   argv, and safe destructive cancellation.
2. **Legacy Bash behavior:** sourcing, function inventory, caller shell options,
   `IFS`, `shopt`, help comment parsing, and current quirks.
3. **New CLI behavior:** explicit targets, typed operands, standard statuses,
   stderr diagnostics, registry migration, and structured output.
4. **Shell-adapter behavior:** session creation/use/unset, environment
   compatibility, wrapper translation, and real-shell quoting.
5. **Intentional divergence:** a reviewed table naming the old observation,
   approval, first release, new expectation, and compatibility treatment.

The fake Docker record format and isolated fixtures are reusable. Add an
adapter that invokes the compiled executable with equivalent fixture inputs;
do not force executable calls through Bash functions. Comparison reports must
show raw status, stdout, stderr, filesystem state, environment/session state,
and ordered Docker argv before any approved normalization.

## Phase overview

| Phase | Outcome | User-visible default |
|---|---|---|
| 0 | Decisions and acceptance matrix approved | Bash only |
| 1 | Compiled foundation beside Bash | Bash only |
| 2 | Read-only config/registry and safe migration tooling | Bash writes; core reads |
| 3 | Target and Compose planner parity | Bash executes Docker |
| 4 | Guarded Docker execution and typed core commands | Opt-in experimental binary |
| 5 | Unified CLI preview and divergence policy | Opt-in new CLI |
| 6 | Bash/Zsh/Fish/PowerShell session adapters | Opt-in per shell |
| 7 | Native Windows and cross-platform acceptance | Preview on all target OSes |
| 8 | Repository-owned release installers | New installs may opt in |
| 9 | Compatibility/deprecation release period | Binary default, wrappers available |
| 10 | Legacy Bash retirement | Binary and minimal adapters |

## Phase 0 — approved architecture and behavior decisions

**Goals:** select language, version line, CLI, state direction, installer role,
and major behavior policy.

**Scope:** reconcile the six design documents, create
`docs/architecture/v2-decisions.md`, and start the approved-only
`implementation-divergences.tsv` without changing v1 expectations.

**Tests/CI:** no changes. Keep the current baseline green.

**Compatibility/rollback:** no v1 runtime change; decision records remain
reviewable history.

**Entry:** characterization baseline is green, as it is now.

**Completion:** achieved for the v2 direction recorded in
`v2-decisions.md`. Exact registry schema, session protocol, exit codes,
container-shell probing, and compatibility removal dates remain correctly
deferred to their implementation gates.

## Phase 1 — compiled foundation alongside Bash

**Goals:** establish the Rust `2.0.0` project and release-independent quality
gate without implementing DStack behavior.

**Scope/modules introduced:** one package/one binary manifest and lockfile,
thin executable entry, typed `cli`, `app`, and error boundary. Only `--help`
and `--version` are user-supported in this phase; unsupported input fails
clearly. No placeholder `doctor` command is needed.

**Tests:** parser snapshots/unit tests, version consistency, path/process smoke
tests on Linux/macOS/Windows. Existing Bash tests still run unchanged.

**CI:** add format/lint/test/build jobs and native OS smoke builds. Keep the
existing Bash matrix and Linux PTY job required. Repository-owned release and
package workflows may be scaffolded from proven IRIS patterns, but remain
dormant until the distribution phase and accept only v2 tags.

**Compatibility:** binary uses a development name/path or explicit feature
selection; it does not shadow installed `dstack.sh` commands.

**Rollback:** remove the new package; Bash remains untouched.

**Entry:** Phase 0 language and naming decisions approved.

**Completion:** reproducible locked builds pass on three OSes, the binary has no
Docker side effects, and the project remains deliberately one package/binary.
No tag, release, crate publication, or installer activation occurs.

## Phase 2 — config, registry, and migration primitives

**Goals:** implement safe native storage without making it authoritative.

**Scope/modules introduced:** platform directories, typed config, structured
registry model, exact name/path validation, lock/atomic writer, legacy line-file
reader, `registry list`, and `migrate registry --dry-run`.

**Tests:** reuse isolated HOME/registry fixtures for valid, stale, duplicate,
space, Unicode, `=`, regex-like, nested, and symlink paths. Add concurrent
new-writer tests and crash/partial-temp recovery. Add Windows UNC/drive and
case-folding expectations before Windows writes are enabled.

**CI:** storage tests on native Linux/macOS/Windows; fault-injection tests where
deterministic. Bash registry characterization remains unchanged.

**Compatibility:** Bash remains the only writer. Core may read both formats
with explicit precedence. No automatic import or dual-write.

**Rollback:** new files are optional and legacy data is never deleted. Migration
dry run makes no changes.

**Entry:** registry format/location and malformed-line policies approved.

**Completion:** lock/atomic behavior is tested; migration reports every legacy
line deterministically; a reviewed backup/commit algorithm exists.

## Phase 3 — discovery, target, and Compose planning

**Goals:** make the native core able to produce a complete Docker invocation
plan without starting Docker.

**Scope/modules introduced:** bounded discovery, registry/discovery lookup,
session-input model, exact cwd fallback, Compose candidate discovery, explicit
file selection, noninteractive errors, and a serializable development-only
plan output.

**Tests:** adapt resolution, Compose discovery/selector/precedence, hostile path,
and argv fixtures. Compare approved common behavior with Bash; record approved
fixes such as errors-as-`-f` as intentional divergences. Add
stable locale-independent ordering and native Windows path tests.

**CI:** differential planner tests on all OSes. No real or fake Docker process
is needed for planner units.

**Compatibility:** current environment variables can feed a legacy config
adapter; new config arrays have explicit precedence and warnings.

**Rollback:** planner is unused by default.

**Entry:** Phase 2 read path stable and CLI targeting grammar approved.

**Completion:** every supported target/file scenario returns either a typed
plan or typed error, with no text parsing and no ambiguous first-token lookup.

## Phase 4 — Docker process boundary and typed stack commands

**Goals:** execute approved plans through the guarded fake Docker and implement
the non-destructive command core.

**Scope/modules introduced:** Docker executable lookup/probe, direct process
runner, stream attachment/capture policy, signal/cancellation handling, and
`services`, `start`, `stop`, `restart`, `pull`, `logs`, `exec`, `run`, and raw
`compose` in experimental form. Compose configuration is exercised as raw
`dstack compose config`; top-level `config` remains reserved.

**Tests:** reuse NUL argv records and sequential fake responses. Add compiled-
adapter exact argv cases, Docker status propagation, stream separation,
long-running stub cancellation, and Windows console/process tests. No daemon.

**CI:** fake-Docker tests on all native platforms. Add targeted signal tests by
platform; keep flaky timing out of required core gates.

**Compatibility:** explicit experimental binary/feature; Bash remains default.

**Rollback:** users continue using Bash. No registry writes are required.

**Entry:** planner is complete and error/status/output policies approved.

**Completion:** all non-destructive typed commands and raw passthrough preserve
argv, cwd, streams, and status on all target OSes.

## Phase 5 — unified CLI preview and multi-step behavior

**Goals:** deliver the complete approved command tree as an opt-in preview and
exercise approved divergences.

**Scope/modules introduced:** effective `list`, `use` contract (without adapters
it may require explicit session input), `unset`, profile-aware `registry`,
typed `up`, `down`, `recreate`, `update`, `rebuild`, output modes, and the
isolated legacy grammar parser. Explicit `--volumes` is sufficient destructive
intent; there is no redundant confirmation layer.

**Tests:** multi-step fail-first/fail-second matrices using the new stop-on-first
policy, explicit destructive-flag behavior, no-Docker-on-validation,
stdout/stderr contract, help/completion checks, and all 43 mapping decisions.
Maintain an approved divergence row for every intentional difference.

**CI:** full native core suite plus unchanged Bash suite. Publish no canonical
installer yet.

**Compatibility:** preview docs show both old and new forms. Legacy parser is
feature/version gated and produces warnings; generic Docker shortcuts are not
core commands.

**Rollback:** remove/disable preview artifact; no installed runtime is replaced.

**Entry:** Phase 4 process boundary green; exact CLI and destructive policy
approved.

**Completion:** approved CLI help is coherent, every mapped command has a
migration path, and no undocumented divergence exists.

## Phase 6 — shell adapters and session context

**Goals:** preserve the active-stack workflow in Bash, Zsh, Fish, and
PowerShell without duplicating DStack.

**Scope/modules introduced:** versioned session-file protocol, adapter templates,
`shell init`, completions, cleanup/expiry, optional legacy env mirror, and the
default stack-aware legacy wrappers listed in `cli-design.md`.

**Tests:** hostile names/paths through every template; source/dot-source smoke
tests in real shells; use/show/unset isolation across two simultaneous shells;
stale/permission/corrupt session handling; adapter option-state preservation;
wrapper exact argv. Add real PowerShell tests on Windows.

**CI:** dedicated adapter jobs using supported shell versions. Linux Bash/Fish,
macOS Bash/Zsh/Fish as available, and Windows PowerShell are required before
claiming support. Git Bash remains a compatibility job, not the Windows oracle.

**Compatibility:** adapters can read legacy environment values. Bash users may
continue sourcing `dstack.sh`; they do not source both old and new default
adapters without an explicit coexistence mode.

**Rollback:** remove the new profile marker block and resume sourcing
`dstack.sh`; registry remains readable and legacy data remains intact.

**Entry:** session semantics and legacy-variable policy approved.

**Completion:** four shell adapters contain only session/wrapper/completion
logic, two terminals do not share context, and quoting tests cover spaces,
Unicode, metacharacters, and newlines according to approved input policy.

## Phase 7 — native Windows acceptance

**Goals:** prove native Windows as a product platform rather than infer it from
Git Bash.

**Scope:** MSVC binary, PowerShell adapter, LocalAppData/config behavior,
drive/UNC/Unicode paths, file locking/replacement, Docker Desktop CLI,
console cancellation, and Windows archive layout. Add ARM64 only when an
executable smoke-test route is available.

**Tests:** native PowerShell workflows, fake `docker.exe`, path/case/symlink or
junction policy, concurrent registry access, Ctrl+C/child cleanup, installer
staging/rollback, and profile idempotency. A small opt-in Docker Desktop smoke
suite may be manual/release-only.

**CI:** required Windows x86_64 native jobs. Git Bash remains separate. Windows
ARM64 can begin build-only but must not be advertised as supported until run.

**Compatibility:** WSL uses the Linux artifact; Git Bash support is explicitly
documented and may use the Windows binary with adapter limitations.

**Rollback:** Windows preview users reinstall a previous version; no legacy
native runtime exists to replace.

**Entry:** Phase 6 PowerShell/session protocol stable and Windows policies
approved.

**Completion:** native Windows acceptance checklist passes without Bash,
MSYS, Unix paths, or Unix permissions.

## Phase 8 — repository-owned release and installers

**Goals:** make binary installation reproducible, verified, reversible, and
reviewable.

**Scope/files introduced:** future `scripts/install.sh`,
`scripts/install.ps1`, release workflow, checksum manifest, artifact naming,
profile marker management, version selection, staged replacement, previous-
version rollback, uninstall, and installation-channel-aware `self-update`.

**Tests:** OS/architecture detection tables, explicit/latest version resolution,
checksum mismatch, network/partial archive failure, existing install,
reinstall, update, rollback, uninstall with/without config deletion, PATH/profile
idempotency, and unsupported targets. Use local fixture servers/artifacts in CI.

**CI/release:** tags must equal package version. Quality gates precede native
builds. Archives and installers are checksummed and uploaded to a draft GitHub
Release for review. Start with Linux x86_64/ARM64, macOS x86_64/ARM64, and
Windows x86_64; gate Windows ARM64 separately.

**Compatibility:** current website installer remains available during preview
and must be clearly labeled as Bash DStack. New installer uses a preview URL or
flag until promotion.

**Rollback:** installer retains one previous verified version and restores it;
profile changes are marker-delimited and reversible. User registry/config is
not deleted by binary rollback.

**Entry:** binary and adapters meet platform acceptance; installer ownership and
website role approved.

**Completion:** a clean machine can install, update, roll back, and uninstall
without Git, Bash on Windows, mutable branches, or unverified binaries.

## Phase 9 — default transition and deprecation window

**Goals:** make the unified binary the recommended install while preserving
common Bash workflows for a bounded period.

**Scope:** website stable URL points to the repository-produced release
installer; README leads with new CLI; adapters install default stack-aware
wrappers; telemetry-free warnings explain replacements.

**Legacy wrappers:** default temporarily for:

```text
dhelp dversion dstackunset dsvc dstart dstop dcompose ddown ddownv
drecompose drebootstack drestart dlogs dlog dllogs dllog dexec drun
dpull dupdate drebuild drebuildnocache dconfig
```

Generic Docker aliases may be opt-in for a shorter window. Do not install
`dstopall`, `dstackpurge`, or `dcleanall` by default. The executable's narrow
legacy `dstack add|ls|NAME` parser is warned and time-bounded.

**Tests/CI:** install/upgrade from the last Bash release, registry dry-run and
migration, old wrapper to new Docker argv, rollback to Bash, and clean install
on all platforms. Both Bash and compiled suites remain required.

**Rollback:** installer restores previous Bash installation/profile block and
keeps the legacy registry backup. Release docs publish exact rollback steps.

**Entry:** at least one preview release has real-world feedback and no unresolved
data-loss, argv, status, installer, or session issue.

**Completion:** two approved minor releases (or the approved alternative) have
shipped, warning metrics are not required, migration docs are complete, and
maintainer explicitly approves the major removal gate.

## Phase 10 — legacy Bash retirement

**Goals:** remove sourced business logic and permanent compatibility baggage.

**Scope:** stop installing `dstack.sh`; remove default wrappers and legacy CLI
grammar according to the approved schedule. Retain a tagged final Bash release,
migration documentation, and perhaps an opt-in separately versioned legacy
adapter for one final support window.

**Tests/CI:** compiled core, adapter, installer, and native platform suites
remain required. Freeze legacy Bash characterization against the final Bash
tag rather than deleting it immediately. Retire it only after supported upgrade
paths no longer execute Bash code.

**Rollback:** install the final Bash tag through documented archival steps; the
new structured registry retains its backup/export path.

**Entry:** explicit major-release approval and completion of Phase 9 criteria.

**Completion:** one executable plus minimal adapters provides the supported
surface, and no supported command depends on Bash business logic.

## Backward compatibility details

- **`source dstack.sh`:** remains valid during dual implementation. New installs
  source/dot-source only the native shell adapter.
- **Current command names:** stack-aware commands receive bounded wrappers;
  generic Docker helpers are opt-in/shorter-lived; dangerous host-wide helpers
  are not default wrappers.
- **Bash:** first adapter and strongest legacy support.
- **Zsh/Fish/PowerShell:** native adapters; never source Bash syntax.
- **Active stack:** new per-shell session file. Optional adapter mirror supports
  old exported variables during the approved window.
- **Registry:** legacy line file remains untouched and backed up; explicit
  migration creates structured storage.
- **Environment variables:** read with warnings during coexistence. Config arrays
  replace whitespace-split bases/files. Exact removal release is documented.
- **Install location:** installer detects and can migrate
  `~/.local/share/dstack`; binaries use user bin conventions. Existing source
  clone installs remain a manual channel.
- **Update/uninstall:** old reinstall/manual removal remains documented until
  the new installer owns only its marker block and version directories.

## CI migration

Do not replace the current jobs initially. Add:

1. compiled quality (format, lint, unit/integration tests);
2. native Linux/macOS/Windows build/test matrix;
3. cross-implementation fixture/fake-Docker comparisons;
4. shell-adapter jobs as adapters appear;
5. installer fixture tests;
6. tag-triggered release verification/build/package/draft jobs.

Legacy Bash CI can retire only when the corresponding runtime is no longer
installed or invoked by any supported upgrade/compatibility path. The final
Bash tag should retain its own frozen CI evidence.

## Release and update architecture

- Semantic version in the package manifest is the release source; tag
  `vX.Y.Z` must match.
- Native runner artifacts use stable names:
  `dstack-vX.Y.Z-<os>-<arch>.<archive>`.
- Each artifact is covered by `SHA256SUMS`; draft GitHub Release review occurs
  before publication.
- Installers are versioned release assets from repository source and verify the
  binary archive before activation.
- User installs retain current and one previous version. Update stages the new
  version, runs `--version`, switches atomically, and can roll back.
- The website URL redirects to the release installer. It does not independently
  generate runtime/install logic or download mutable `main`.
- Package managers are later channels. Each channel documents its own update
  ownership, as IRIS does.

## Deferred characterization review

| Deferred area | Classification for migration | Reason/gate |
|---|---|---|
| Actual Bash 3 guard | **Unnecessary for compiled implementation; optional for final legacy tag** | Current code clearly rejects it; no need to acquire Bash 3 to design the new core |
| Concurrent legacy registry writes | **Optional for Bash; required new-core test before Phase 2 write authority** | Do not spend effort stabilizing a known unsafe legacy format; prove the replacement |
| Embedded-newline registry names | **Optional legacy; required validation decision before Phase 2** | New structured format can support or explicitly reject controls; no fragile Bash test needed |
| Filesystem case folding | **Required before Phase 7** | Native Windows name/path identity and duplicate handling depend on it |
| macOS PTY parity | **Required before claiming full interactive macOS behavior in Phase 6** | New native terminal layer should be tested on macOS; exact legacy transcript is not required |
| Windows/Git Bash PTY | **Optional** | It is compatibility coverage, not native PowerShell/Windows acceptance |
| Exact PTY stream separation | **Required for new output contract by Phase 4/6, not as a legacy PTY transcript** | New core can test attached/captured streams directly |
| Real pager behavior | **Unnecessary as a blocker** | Respect/disable pager and test the boundary; full-screen automation is low value |
| Long-running streams | **Required with deterministic stub before Phase 4 completion** | Signal/cancellation must work without a real daemon |
| Real Docker integration | **Optional release/manual smoke before default transition** | Fake Docker remains the safe regression oracle; daemon state is unsuitable for routine CI |

## Lessons from IRIS

Useful patterns for migration:

- introduce one Cargo package with a thin entry point rather than a workspace;
- keep quality and native build matrices separate;
- use locked dependencies and tag/version verification;
- build release artifacts on native OS runners, checksum them, and create a
  draft release for human review;
- document updates according to installation channel.

Patterns not to copy mechanically:

- IRIS's broad renderer/module/dependency structure;
- crates.io publication or multiple native Linux packages as an early gate;
- Unix Makefile install/update as the primary cross-platform path;
- manual prebuilt replacement without rollback;
- flat CLI flags for DStack's multi-action product;
- lack of Windows ARM64 execution evidence.

IRIS proves the maintainer can operate Rust and release infrastructure. DStack
still needs its own shell-session, installer, data migration, Docker process,
and compatibility plan.

## Migration risks requiring explicit gates

- Ambiguous/malformed legacy registry rows could be imported incorrectly.
- Two implementations could both write state unless one authority is enforced.
- Shell adapters could become business-logic forks or execute unsafe generated
  code.
- MSYS behavior could be mistaken for native Windows acceptance.
- Status/output fixes could break scripts without visible deprecation.
- Compatibility wrappers could remain indefinitely and block CLI cleanup.
- Installer promotion could strand users without rollback.
- Release checksums do not by themselves secure a compromised bootstrap URL.

The phased authority changes, divergence ledger, native acceptance jobs,
bounded wrapper schedule, backups, and rollback requirements above are the
controls for those risks.
