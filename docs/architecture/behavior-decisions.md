# DStack v2 behavior decisions

This document applies the approved DStack v2 direction to the observations in
the v1 behavior ledger. The current Bash characterization suite remains
authoritative for v1. Approved v2 differences are also recorded in
`tests/characterization/decisions/implementation-divergences.tsv`.

## Decision vocabulary

- **PRESERVE**: make the observed behavior part of the future contract.
- **FIX**: treat the observation as a defect and intentionally diverge.
- **REDESIGN**: retain the user goal through a different, explicit interface.
- **REMOVE**: do not carry the behavior into the supported future surface.
- **DEFER**: gather evidence or postpone the choice to a named migration gate.

Every intentional divergence is recorded separately from the unchanged Bash
oracle. Details explicitly marked deferred still require approval at their
named migration gate.

## Loading, execution, and shell state

| Ledger rows | Proposal | Current behavior and evidence | Documentation and impact | Proposed future behavior and migration |
|---|---|---|---|---|
| `compat.direct_execution_no_dispatch`, `docs.ci_version_command` | **FIX** | `./dstack.sh` and `./dstack.sh dversion` silently succeed; load cases prove no dispatch. | The supported source model explains the cause, but the old CI name implied a working version command. Silent success is poor scripting UX. | The executable always dispatches. No args run the documented default (`list`), `--help` and `--version` are real interfaces, unknown commands exit nonzero. Keep the Bash observation only in legacy tests. |
| `compat.dhelp_disables_nullglob` | **REMOVE** | `dhelp` disables an already-enabled caller `nullglob`; `load::dhelp_disables_preexisting_nullglob`. | Undocumented shell-state damage. Portability and usability are both negative. | Generated executable help has no shell-option effects. Legacy `dhelp` wrapper calls `dstack --help`. |
| `compat.caller_options_and_ifs` (legacy row) | **REMOVE** from core | Sourced functions inherit `errexit`, `nounset`, `pipefail`, and `IFS`. | Characterized legacy behavior, not a useful product contract. | The executable owns its status and parsing. Adapters preserve caller shell options and `IFS`. |
| `state.unvalidated_selector_export` | **FIX** | `dstack NAME SELECTOR` exports selector text without validating it. | Current help presents selection as usable context; storing an invalid selector defers a confusing failure. | `dstack use NAME --file FILE` resolves and validates before changing session state. Failure leaves previous state untouched and exits nonzero. |
| `state.unregister_retains_active`, `docs.dstackunset_active_state` | **REDESIGN** | `dstackunset` removes a registered-only entry but usually leaves active stack and selector; state test proves it. Inline help says it clears matching state. | Current name conflates registration and activation. | Split `dstack registry remove NAME` from `dstack unset`. The former changes only registry data; the latter clears session context. A compatibility `dstackunset` wrapper may perform both during deprecation. |

Usability improves because a command either changes valid state or reports a
failure. Portability improves because session state is no longer coupled to
Bash exports. Compatibility impact is intentional and requires release notes.

## Registry and configuration

| Ledger rows | Proposal | Current behavior and evidence | Documentation and impact | Proposed future behavior and migration |
|---|---|---|---|---|
| `registry.fixed_home_path` (contract candidate) | **REDESIGN** | Registry is always `$HOME/.config/dstack/registry`; XDG is ignored. | README exposes the path. It is Linux-centric and wrong for native macOS/Windows. | Use platform config/data directories. Continue discovering the legacy file for migration on Unix-like installs. |
| `registry.space_path_word_splitting` | **FIX** | A real Compose project whose path contains spaces fails the registration probe. | README already warns only custom bases about spaces, not registered paths. Ordinary paths must work. | Use native path values end to end; never word-split. Add cross-platform Unicode/space tests before the new registry writes user data. |
| `registry.regex_name_rewrite` | **FIX** | Registering `app.*` removes entries matched by grep regex. | Undocumented data loss risk. | Exact key lookup in structured storage. Validate names independently of storage syntax. |
| `registry.equals_name_format` | **FIX** | `team=prod` is written unescaped and cannot round-trip as the same first field. | Undocumented and non-portable. | Structured quoted keys. Either accept `=` safely or reject it with a clear validation error; recommendation is accept it. |
| `registry.fixed_tmp_no_locking` | **REDESIGN** | All writers use adjacent `registry.tmp`; sequential replacement is tested, races are not. | No concurrency claim. Concurrent shells can lose data or move each other's temp file. | Hold an advisory lock, write a unique same-directory temp file, flush, then replace atomically with Windows-aware semantics. Add new-core concurrency tests before it becomes write authority. |

Recommendation: use a versioned TOML registry managed by the CLI, with quoted
names and native path strings. Keep the existing line file read-only during a
bounded migration period; offer a dry run and backup. Do not silently guess at
ambiguous legacy lines containing additional `=` characters.

## Stack discovery and resolution

| Ledger rows | Proposal | Current behavior and evidence | Documentation and impact | Proposed future behavior and migration |
|---|---|---|---|---|
| `compat.explicit_active_local_precedence` (contract candidate) | **PRESERVE** with explicit syntax | Explicit resolvable first token wins, then active `DSTACK`, then exact cwd. | README agrees. The first-token encoding causes collisions. | Preserve target priority as `--stack` > valid session context > exact cwd. Do not search parents unless a later feature is separately approved. |
| `compat.first_token_collisions` | **REDESIGN** | A stack named `up`, `start`, `restart`, `logs`, or a service name is consumed as the stack. | Examples rely on positional stack names but do not explain collisions. Scripts are unpredictable. | Stack targeting is only `--stack NAME`/`-s NAME` (or active/local context). Service and Compose argv never become stack names by successful lookup. Compatibility wrappers retain old parsing only during deprecation. |
| `resolution.invalid_active_partial_clear` | **FIX** | `_dcompose` clears invalid `DSTACK`, retains `DSTACK_COMPOSE_FILE`, warns, then may use cwd. | Undocumented and can select an unrelated local file. | An explicitly active but invalid context is an error and no Docker command runs. `dstack unset` clears the whole context. A user can opt into local context by unsetting it. |
| `resolution.duplicate_family_invalid_active` | **FIX** | Multi-step commands duplicate resolution and can invoke `docker compose` with no `-f` from an invalid active path. | Inconsistent with documented precedence and unsafe. | One resolver is used by every command. No command privately reimplements target or Compose selection. |
| `docs.discovery_depth`, `platform.find_depth_flags` | **REDESIGN** | Listing asks host `find` for depths one and two; unsupported flags change results. README wording says shallow/one level ambiguously. | Utility-dependent product behavior. | Native bounded traversal with a documented default depth of two directory components below each base. Make depth configurable only if a real use case is approved. |
| `platform.git_bash_paths` | **DEFER** as compatibility | Git Bash exposes POSIX-style drive paths; characterization records them. | This is not native Windows behavior. | Native Windows uses Windows paths. Retain Git Bash testing during legacy support, but do not make MSYS paths a core contract. |
| `platform.symlink_permissions` | **REDESIGN** | Registration stores `pwd -P` where symlinks are available; Windows may not permit creating them. | Physical path behavior is implicit. | Store a normalized user-facing absolute path; validate the target. Define identity/canonicalization separately and test Windows junction/symlink behavior before native Windows GA. |

## Compose discovery and selection

| Ledger rows | Proposal | Current behavior and evidence | Documentation and impact | Proposed future behavior and migration |
|---|---|---|---|---|
| Default standard/glob ordering (contract candidates) | **PRESERVE** initially | Standard names precede locale-sorted `*compose*` matches; discovery cases cover exact ordering under `LC_ALL=C`. | README lists the same candidates. | Preserve standard-name priority. Define additional-file ordering by stable filename sort rather than shell/locale glob order. |
| `compose.custom_candidates_verbatim`, `docs.custom_compose_candidates` | **REDESIGN** | `DSTACK_COMPOSE_FILES` is whitespace-split, emitted verbatim, and not joined to stack path. | README's bare-name example implies stack-relative files. Spaces fail. | Replace with a TOML array of candidate names/paths, resolved relative to each stack and existence-checked. Accept the legacy env var only in compatibility mode with a warning. |
| `compose.non_tty_multi_error_as_file` | **FIX** | Multiple files without a TTY log an error that becomes a literal `-f` value; piped selection is ignored. | README incorrectly says first file is selected. This can send nonsensical argv to Docker. | With multiple files and no explicit selector: prompt only on a TTY; otherwise exit with a dedicated nonzero resolution error. Never invoke Docker. |
| `compose.invalid_selector_error_as_file` | **FIX** | Invalid, empty, or EOF selection text is captured as a filename. | Undocumented and unsafe. | Typed resolver errors never enter the file list. Leave state unchanged and exit nonzero. |
| `compose.empty_flags_success` | **FIX** | No candidates can make flag resolution succeed with no output. | Conflicts with the command's purpose. | No Compose file is a nonzero resolution error before Docker. |
| `compose.positional_filename_command`, `docs.positional_compose_file` | **REMOVE** from new grammar | README-style `dcompose STACK FILE` sends `FILE` as a Compose command. | Documentation and runtime disagree. Positional file/command is intrinsically ambiguous. | Require repeated DStack-owned `--file FILE`. It may appear around typed commands where unambiguous, but must precede the raw `compose` passthrough boundary. Compatibility wrappers may parse the documented old form during deprecation. |
| `compose.multi_file_documentation`, `docs.multiple_compose_default` | **REDESIGN** | Selection-enabled commands prompt on a TTY and error in non-TTY; `--no-select` wrappers choose first. README says first by default. | Commands behave differently without a coherent product rule. | Apply one rule to all commands: one file is automatic; multiple require explicit files in noninteractive mode and offer a TTY menu otherwise. Provide `--all-files`; do not silently choose first. |

File selectors should accept repeated `--file` values and an explicit
`--all-files`, not numeric indexes in scripts. Numeric interactive menu choices
remain presentation details. Every selected path is validated before Docker.

## Command parsing and forwarding

| Ledger rows | Proposal | Current behavior and evidence | Documentation and impact | Proposed future behavior and migration |
|---|---|---|---|---|
| `compat.array_argument_forwarding` (contract candidate) | **PRESERVE** | Empty, spaced, wildcard, variable, and shell metacharacter arguments remain exact argv. | `drun` documentation correctly requires an explicit `sh -c` for shell syntax. | Direct process invocation with an argv vector; never `eval` or shell reconstruction. |
| `forwarding.wrapper_argument_order` | **REDESIGN** | Unresolved service-like tokens are placed before wrapper verbs (`web start`, etc.). | Surprising and caused by positional stack guessing. | Typed subcommands put their verb and service operands in documented Docker order. Targeting is a flag, not a guessed token. |
| `public.dlog`, `forwarding.dlog_fixed_tail` | **FIX** | `dlog` always uses 100 despite a description suggesting an optional count. | Help and behavior disagree. | Merge into `dstack logs [SERVICE] --follow --tail N`; default tail remains 100 unless approved otherwise. |
| `public.dllog`, `forwarding.dllog_negative_slice`, `docs.log_helper_forms` | **FIX** | The documented one-argument service form fails with a negative Bash slice. | Clear defect. | Same coherent `logs` interface; paging is an option rather than a separate parser. |
| `public.drebuild`, `forwarding.rebuild_two_token_drop` | **FIX** | An unresolved two-token call silently ignores the second and rebuilds the first; Docker errors are also masked. | Documentation implies stack/service. | `dstack --stack NAME rebuild [SERVICE...] [--no-cache]`; unresolved target and unknown syntax fail before Docker. |

## Status, output, and Docker failures

| Ledger rows | Proposal | Current behavior and evidence | Documentation and impact | Proposed future behavior and migration |
|---|---|---|---|---|
| `compat.compose_failure_masking` | **FIX** | Most Compose failures return 0 because of `|| true`. | Users and scripts cannot detect failure. | Return Docker's status for one-step commands. Multi-step commands use the policy below. Legacy wrappers may warn about changed statuses. |
| `compat.direct_failure_status` | **PRESERVE** as a general rule | Some direct commands propagate Docker/pipeline status, but caller `pipefail` changes pipelines. | Inconsistent today. | Direct child status is authoritative. Native formatting cannot hide it. Define stable DStack usage/resolution codes separately. |
| `compat.rebuild_stderr_suppression` | **FIX** | Both rebuild families discard Docker stderr. | Removes the most useful diagnostic and is undocumented. | Forward Docker stderr unchanged unless a documented quiet flag is used. |
| `compat.logging_streams` | **FIX** | Info, warning, validation errors, and resolver errors use stdout; menus use stderr. | Breaks command substitution and caused errors-as-data. | Command/data output and Docker stdout stay on stdout. DStack diagnostics, warnings, errors, and prompts go to stderr. `--quiet` suppresses DStack info only. |
| `compat.formatted_and_paged_output` | **REDESIGN** | Some direct helpers pipeline through `sed`/`column`; logs/inspect hard-code `less`. | Platform dependencies and pipeline statuses leak into behavior. | Core output is stable and scriptable. Optional paging occurs only for a TTY, honors `PAGER`, can be disabled, and never changes Docker status. Generic Docker formatters are candidates for removal. |

Recommended error policy:

- usage and DStack validation errors: nonzero, no Docker invocation;
- stack/Compose resolution errors: distinct nonzero class;
- Docker commands: propagate Docker status;
- user cancellation: success only when cancellation is an ordinary choice, or
  a documented distinct code for automation; maintainer must choose;
- internal/system errors: nonzero with stderr diagnostics.

Exact numeric codes should be approved and documented before implementation.

## Multi-step and destructive operations

| Ledger rows | Proposal | Current behavior and evidence | Documentation and impact | Proposed future behavior and migration |
|---|---|---|---|---|
| `public.drecompose` | **REDESIGN** | Always runs `down -v`, then `up -d`, even if down fails. | Destructive intent is documented, continuation is not. | `dstack recreate --volumes`; the explicit flag represents intent, and the operation stops on the first failure. |
| `public.dupdate` | **REDESIGN** | Runs pull then up even when pull fails; returns success. | Expected goal is update, not best-effort continuation. | `dstack update`; stop if pull fails. Optional `--continue-on-error` only if a concrete use case is approved. |
| `public.drebuildnocache` | **REDESIGN** | Build and up both run with suppressed errors and success status. | Diagnostic and automation harm. | `dstack rebuild --no-cache`; stop on failed build, forward stderr, propagate status. |
| `public.dstackpurge` | **REMOVE** combined operation | Stack-local `down -v` is followed by host-wide `docker system prune -f`, even after failure. | Scope jump is dangerous. | Use `dstack down --volumes` for the stack. Host-wide prune remains an explicit Docker command, not a DStack stack operation. |
| `public.dstopall`, `platform.xargs_empty_input` | **REMOVE** | Stops every running container; empty input invokes `docker stop` on GNU/Git Bash but not macOS. | Host-wide and unrelated to stack identity. | Direct users to `docker stop ...`; no future core command or default compatibility wrapper. |
| `public.dvolrm` | **REMOVE** from core | Thin wrapper for `docker volume rm`, status masked. | Generic Docker operation. | Direct Docker CLI. Optional legacy wrapper for one deprecation window. |
| `public.dclean`, `public.dcleani`, `public.dcleanall` | **REMOVE** from core | Forced container/image prune and confirmed system prune are generic host-wide operations. | Safety scope exceeds stack management. | Direct Docker CLI with Docker's own semantics. Do not create a broad `dstack clean` namespace merely to preserve shortcuts. |

Stack-local `down --volumes` remains legitimate DStack behavior and the flag
itself is explicit intent; v2 does not add a redundant confirmation. Actions
that cross from the selected stack to the entire Docker host remain outside
the permanent DStack core.

## Documentation and platform observations

| Ledger rows | Proposal | Future treatment |
|---|---|---|
| `docs.multiple_compose_default`, `docs.positional_compose_file`, `docs.custom_compose_candidates`, `docs.dstackunset_active_state`, `docs.log_helper_forms` | **FIX documentation with the approved interface** | These duplicate concrete mismatches above. Update docs only when behavior changes land; do not rewrite current docs in advance. |
| `docs.discovery_depth` | **REDESIGN** | State the exact native traversal depth and examples. |
| `docs.dependency_claim` | **FIX** | Current docs must remain honest about Bash/utilities; future docs can truthfully state binary plus Docker and optional shell integration. |
| `docs.ci_version_command` | **PRESERVE corrected CI intent** | CI now sources and checks current `dversion`. Future CI must invoke executable `dstack --version`. |
| `platform.find_depth_flags`, `platform.xargs_empty_input` | **REMOVE implementation dependence** | Native traversal/argv eliminates host-utility differences. |
| `platform.git_bash_paths` | **DEFER compatibility** | Keep legacy coverage, separate from native Windows acceptance. |
| `platform.symlink_permissions` | **REDESIGN** | Define path identity per native platform rather than capability-skipping a global rule. |

## Contract candidates retained for v2

V2 retains these current goals rather than their Bash mechanism:

- resolve explicit target, then session context, then exact current directory;
- preserve exact argv boundaries at Docker;
- discover standard Compose filenames in stable priority order;
- support one automatic file and explicit multi-file selection;
- support registry and bounded discovery stacks;
- provide active session context, logs, exec, run, lifecycle, raw Compose
  config, pull, update, restart, and rebuild workflows;
- require explicit destructive command syntax for stack-local volume removal;
- color only on a terminal, with a user override.

## Lessons from IRIS

IRIS demonstrates useful future-behavior conventions: direct executable
dispatch, standard `--help`/`--version`, typed CLI overrides, native config
paths, real nonzero top-level errors, and platform-specific enhancements that
fall back cleanly. Those support the recommendations to fix silent dispatch,
stdout diagnostics, and utility-dependent behavior.

IRIS does not establish DStack's contract. Its one-action viewer has no active
shell context, Compose ambiguity, multi-step Docker failure policy, destructive
host operations, or legacy function family. DStack therefore needs explicit
maintainer decisions rather than inheriting IRIS behavior by analogy.

## Remaining implementation decisions

The language, `2.0.0` version line, unified executable, explicit stack syntax,
typed/raw command split, status and stream direction, native platform goal,
structured registry direction, and installer/release direction are approved.
The following details remain deliberately gated:

| Detail | Required by | Primary tradeoff |
|---|---|---|
| Exact legacy-wrapper removal release | Default-transition phase | Predictable cleanup versus migration time |
| Exact DStack-owned exit codes and cancellation code | Docker/interactive phases | Script precision versus unnecessary taxonomy |
| Registry TOML schema, application identifiers, and control-character policy | Registry phase | Round-tripping versus validation simplicity |
| Session-file transport/schema and cleanup lifetime | Shell-adapter phase | Cross-shell safety versus protocol complexity |
| Container-shell probe order and failure diagnostic | Docker execution phase | Convenience versus deterministic behavior |
| Windows ARM64 support date | Native Windows/release phases | Coverage cost versus target reach |
| Package-manager channels and ownership rules | Distribution phase | Convenience versus maintenance surface |

These gates must not block the Phase 1 help/version foundation, and Phase 1
must not pre-decide them in code.
