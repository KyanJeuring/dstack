# DStack portability audit

This document audits the v1 Bash implementation and records the approved v2
portability direction. DStack v2 is a Rust `2.0.0` executable for Linux,
macOS, and native Windows with minimal shell adapters. Deferred platform
details remain migration gates rather than implicit promises.

## Scope and evidence

This audit is based on `dstack.sh`, the 190-case characterization baseline,
the behavior-classification ledger, current documentation and CI, the
production installer fetched from
`https://kyanjeuring.com/scripts/install-dstack` on 2026-11-04, and the local
IRIS repository. A characterization assertion is evidence of current behavior,
not a promise that the future implementation must preserve it.

The portable product at DStack's center is small: identify a stack, select one
or more Compose files, and launch Docker with exact argv. Most other shell code
implements discovery, text processing, presentation, or historical shortcuts.
The only essential feature that cannot live entirely in a child executable is
session-scoped active context in the user's current shell.

## Current portability boundary

Current Windows CI is Git Bash behavior. WSL is a Linux environment. Neither
is native Windows support. Current macOS support depends on Homebrew Bash and
on utility behavior that is not guaranteed by POSIX. Linux is the primary
runtime assumed by the implementation and installer.

The future boundary should be:

- one native executable owning all parsing, registry, discovery, Compose
  selection, confirmation decisions, and Docker process construction;
- very small adapters for Bash, Zsh, Fish, and PowerShell that establish a
  session context and optional legacy command wrappers;
- native path, config-directory, process, terminal, and locking operations
  selected by the executable for Linux, macOS, and Windows;
- no shell command-string reconstruction at the Docker boundary.

## Bash dependencies

| Current dependency | Consequence today | Future classification | Proposed treatment |
|---|---|---|---|
| Sourcing and shell functions | Defines all 43 commands in the caller and lets `dstack` export state | Shell integration | Source only a generated adapter; run business logic in the executable |
| `BASH_SOURCE` | Locates `dstack.sh` and drives comment-based help discovery | Legacy detail | Executable-relative resources and generated CLI help |
| `BASH_VERSINFO` | Rejects Bash 3; the guard exits a sourcing shell | Legacy detail | No core Bash requirement; adapters declare their own minimum shell versions |
| Indexed arrays and slicing | Preserve Docker argv, but cause cases such as `dllog`'s negative slice | Core behavior plus legacy detail | Preserve argv with native vectors; replace positional tricks with typed parsing |
| `mapfile` and process substitution | Transfer resolver output while losing producer status; errors can become `-f` values | Legacy detail | Return typed results directly; never encode errors as stdout data |
| Here strings and `IFS` splitting | Parse comma selectors and whitespace-delimited environment values | Core behavior plus legacy detail | Parse typed lists; use config arrays and repeated flags |
| `[[ ... ]]`, regexes, arithmetic | Validation, glob matching, option and selector logic | Core behavior | Native string, path, enum, and integer parsing |
| `shopt`/`nullglob` | Expands Compose candidates and leaks option state from `dhelp` | Legacy detail | Native directory iteration; no caller option mutation |
| Caller `errexit`, `nounset`, `pipefail`, and `IFS` | Changes function behavior and pipeline statuses | Legacy detail | Executable owns status semantics independent of caller shell options |
| Parameter expansion and `${!#}` | Selects final arguments and rewrites whitespace | Legacy detail | CLI parser owns services, targets, and passthrough boundaries |
| `-t`, `read`, `read -p` | Enables menus, confirmation, and ANSI color | Core behavior | Native terminal detection and input; adapters do not prompt |
| Exported `DSTACK` and `DSTACK_COMPOSE_FILE` | Active context persists in the current shell and children | Shell integration | Use a per-shell session context; compatibility adapters may mirror legacy variables during migration |
| Current working directory | Exact-directory local fallback and Docker child cwd | Core behavior | Read native process cwd and preserve it for Docker |

The future core should not inherit semantic behavior from shell options. The
legacy Bash suite should remain intact during migration to detect accidental
changes in the old implementation.

## External command dependencies

| Command | Current use | Platform issue | Future recommendation |
|---|---|---|---|
| `docker` | Direct Docker and `docker compose` execution | Required product boundary; executable/path differs by platform | Keep intentionally; locate through native PATH and invoke directly |
| `awk` | Deduplication, registry parsing, help formatting | External parsing and locale assumptions | Replace with native collections and structured storage |
| `basename`, `dirname` | Selector matching and source/config paths | `dirname --` is characterized but not universally POSIX | Replace with native path APIs |
| `cat` | Help and fake/pager pipelines | Extra process; pipeline status effects | Replace with native I/O; retain only in tests where it is the stub |
| `column` | Formats container lists | Missing or option-incompatible on some hosts | Eliminate; format in core or remove generic Docker helpers |
| `find` | Auto-discovery at depths one and two | `-mindepth/-maxdepth` are non-POSIX | Replace with bounded native traversal |
| `grep` | Registry rewriting and container filtering | Names become regular expressions; pipelines mask status | Replace with exact native matching |
| `less` | Pages logs and inspection | Not native Windows; interactive behavior varies | Do not hard-code. Respect a pager only for optional human output, or stream directly |
| `mkdir`, `touch`, `mv` | Registry setup and replacement | Atomic replacement and permissions differ on Windows | Replace with native filesystem operations and platform-aware atomic replacement |
| `sed` | Header rewrite for `dps`/`dpsa` | Pipeline and utility variation | Replace with native formatting or retire the helpers |
| `whoami` | Windows username fallback | Username is the wrong abstraction for config and discovery roots | Replace with native user/config directory APIs |
| `xargs` | `dstopall` ID forwarding | Empty-input behavior differs on macOS versus Linux/Git Bash | Remove pipeline; if retained, collect IDs and invoke Docker directly |
| `curl` | Installer bootstrap and download | Not guaranteed on native Windows; trust and TLS policy vary | POSIX installer may use curl with checks; PowerShell uses native web APIs |
| `chmod` | Marks Bash runtime executable | No equivalent NTFS executable bit | Installer uses platform-specific installation behavior |
| Git | Clone install/update path | Unnecessary for release installation | Keep only as a source-development option |

Docker is the sole unavoidable runtime executable. An optional pager may be
retained as integration, but it must not carry business logic or determine
command success.

## Filesystem portability

### Current assumptions

- Registry path is fixed at `$HOME/.config/dstack/registry`; XDG overrides,
  macOS Application Support, and Windows AppData are ignored.
- Discovery includes `$HOME/{projects,src,code}`, `/opt/services`, and literal
  `C:/Users/$USER/...` paths. The latter are Git-Bash spellings, not native
  Windows path handling.
- `/dev/null`, forward slashes, Unix executable bits, and Unix TTY semantics
  appear in runtime or tests.
- Registration uses `pwd -P`, resolving symlinks where the host permits them.
- Whitespace splitting breaks registered and discovery paths containing spaces.
- The line registry cannot safely represent `=`, newlines, or regex-sensitive
  names. A fixed adjacent `registry.tmp` has no locking and is race-prone.
- Case sensitivity is inherited from the host. Unicode is not explicitly
  rejected, but byte-oriented tools and locale behavior are not a clear policy.

### Target assumptions

- Use native user config/data/cache/runtime directories. On Linux honor XDG;
  on macOS use the platform application-support convention; on Windows use
  known folders such as LocalAppData/RoamingAppData according to the data's
  roaming semantics.
- Represent paths with the language's native path type until presentation.
  Do not normalize Windows paths through a POSIX shell or pass `C:/...` through
  WSL translation implicitly.
- Accept spaces and Unicode. Reject NUL because supported operating systems do
  not permit it in path components. Decide explicitly whether control
  characters are valid in stack names.
- Treat drive roots, UNC paths, device prefixes, and case-insensitive lookup as
  Windows concerns in the platform boundary.
- Store user-facing absolute paths without leaking Windows `\\?\` canonical
  prefixes to Docker. Resolve filesystem identity separately when needed.
- Use uniquely named temporary files in the destination directory, flush,
  atomically replace where the platform supports it, and hold an advisory lock
  for the complete read-modify-write transaction.
- Do not rely on executable bits on Windows. Release archives and installers
  must express permissions per platform.

Symlink canonicalization should be a reviewed product decision. Blindly
copying `pwd -P` can erase the path the user recognizes and can produce Windows
path forms that Docker does not accept. The proposed registry stores the
user-facing normalized absolute path and validates the resolved target at use.

## Process execution

The current implementation mostly preserves argv with Bash arrays, which is a
valuable behavior to retain. Other pipelines reconstruct behavior through
`sed`, `grep`, `column`, `less`, and `xargs`; their final status can depend on
`pipefail`, and many Docker failures are hidden with `|| true`.

The future process layer should:

1. resolve a Docker executable without invoking a shell;
2. construct an ordered vector such as `docker`, `compose`, repeated `-f`, then
   the user's exact trailing argv;
3. inherit the caller's working directory and relevant environment unless a
   command explicitly documents an override;
4. attach stdin/stdout/stderr directly for Compose, logs, exec, run, and stats;
5. capture output only for bounded probes or structured commands;
6. propagate Docker's exit status by default;
7. forward Ctrl+C and termination appropriately and avoid leaving children;
8. distinguish user cancellation from failure;
9. never concatenate argv into a shell command string.

Long-running and interactive Docker commands must remain attached to the
terminal. Native Windows process groups and console control events need a
dedicated implementation path; Unix signal assumptions must not leak into it.

## Docker boundary

DStack needs only the installed Docker CLI and Compose v2's `docker compose`
surface. It does not need the Docker Engine API for the core use case. Keeping
the CLI boundary preserves Docker contexts, credential helpers, Desktop
integration, plugins, and platform-specific transport behavior.

The core owns target and Compose-file selection. Docker owns Compose parsing,
container behavior, daemon access, and its own output. DStack should check
`docker compose version` when diagnosing prerequisites, but should not require
a running daemon merely to list registry entries or show help.

On native Windows, launch `docker.exe` as found on PATH and pass native Windows
paths. Docker Desktop may target Linux or Windows containers; DStack should not
guess or translate paths to WSL form. WSL DStack is a Linux build invoking the
CLI visible inside WSL. Git Bash remains a compatibility environment and may
perform MSYS argument conversion; it must not define native Windows semantics.

## Windows analysis

### Current

- Git Bash supplies Bash, Unix tools, POSIX-style drive paths, and MSYS
  translation. Characterization records this environment only.
- WSL runs the Linux implementation and filesystem model.
- The current installer requires Bash, curl, chmod, `$HOME`, and a sourced
  profile. It cannot provide native PowerShell installation or execution.

### Future

- Ship an MSVC Windows executable and a PowerShell installer; add ARM64 when a
  repeatable build and smoke-test path is available.
- Use native config/data/temp directories, Unicode paths, drive letters and UNC
  paths, Windows file replacement/locking, and console APIs.
- Test against Docker Desktop's CLI without treating its daemon state as part
  of ordinary unit/characterization CI.
- Generate native PowerShell initialization. Do not source Bash or depend on
  Git for Windows.
- Keep Git Bash and WSL documented as distinct compatibility modes that can
  use the Windows and Linux binaries respectively.

## Shell analysis

The core owns command parsing, validation, resolution, registry/config I/O,
Docker argv, status, logging policy, and prompts. Shell adapters own only
session initialization and legacy wrappers.

| Shell | Shell-specific work | Core-owned work | Notes |
|---|---|---|---|
| Bash | Source `dstack shell init bash`; export a session-state pointer; define temporary legacy `d*` functions | Everything else | Must not change caller options, `IFS`, or `shopt` |
| Zsh | Native `source` syntax and Zsh function/completion definitions | Everything else | Never source `dstack.sh`; current installer behavior is invalid as product support |
| Fish | `dstack shell init fish | source`, Fish exports/functions/completions | Everything else | No Bash syntax, arrays, or command substitution protocol assumptions |
| PowerShell | Dot-source generated PowerShell, set `$env:` session pointer, native completion/wrappers | Everything else | Use native Windows paths and process behavior; no Git Bash dependency |

The proposed session model uses a per-shell state file whose location is placed
in an exported variable when the adapter initializes. `dstack use` and
`dstack unset` can then update that file from a child process without unsafe
`eval` or attempting to mutate the parent. Compatibility adapters may mirror
legacy `DSTACK` variables only during migration. The state file must be
user-private, uniquely named, atomically updated, and cleaned on shell exit
where reliable.

Completions are generated from the same command schema as help. They may offer
stack and service names, but must not duplicate resolution logic in shell.

## Current installer audit

The external installer is Bash with `set -euo pipefail`. It defaults
`DSTACK_VERSION` to mutable `main`, downloads raw `dstack.sh` from GitHub into
`~/.local/share/dstack`, marks it executable, and appends a source line to
`.bashrc` or `.zshrc` based on `$SHELL`.

Consequences:

- Docker and curl are hard prerequisites even for installation; Docker is not
  version-checked and need not be running.
- A version value may be a tag, branch, commit, or arbitrary ref. No checksum,
  signature, tag validation, or release manifest is checked.
- Default installation is not release-reproducible because `main` moves.
- The target is overwritten in place. There is no staged validation, atomic
  swap, backup, rollback, or partial-download recovery policy.
- Profile detection supports only names `bash` and `zsh`. Adding `source
  dstack.sh` to `.zshrc` does not make Bash functions valid Zsh integration.
- The source line is unquoted, and idempotency is a broad grep for `dstack.sh`.
- Reinstall is the update mechanism. Uninstall is printed as manual file and
  profile editing. Native Windows, Fish, PowerShell, and CMD are unsupported.
- Piping an externally hosted mutable script to Bash is the initial trust
  root; the downloaded runtime has a second unverified network trust step.

The installer should become repository-owned, release-versioned, reviewed, and
tested. The stable website URL can remain a bootstrap address, but should
redirect to or serve an immutable release installer rather than contain a
separate unreviewed implementation.

## Lessons from IRIS

### Patterns worth adopting

| IRIS pattern | Why it works in IRIS | Fit and difference for DStack | Complexity effect |
|---|---|---|---|
| Thin `main.rs` plus library modules | The entry point only formats a top-level error; TUI/config/input logic is testable in modules | DStack also benefits, but needs only CLI, state, resolution, process, shell, and platform modules—not renderer layers | Simplifies testing and error ownership |
| Clap-derived CLI/help/version and `PathBuf` operands | IRIS has one authoritative parser and avoids hand-written help | DStack needs nested subcommands and trailing Compose argv rather than IRIS's flat flags | Removes 43 hand parsers and comment-derived help; modest dependency |
| Native config directory plus legacy fallback | `dirs::config_dir` works across hosts and IRIS reads its earlier config filename | DStack has mutable machine-specific registry data and needs locking/import, which IRIS does not | Simplifies locations; adds justified migration code |
| Native-runner release matrix | IRIS builds Linux x86_64/ARM64, macOS Intel/ARM64, and Windows x86_64 where each toolchain is native | DStack needs the same primary targets and should add Windows ARM64 only with execution evidence | Reuses an understood pattern; avoids complex cross-linking |
| Tag/manifest verification, checksummed archives, draft release | IRIS prevents mismatched tags and allows artifact review | DStack additionally needs release installers, rollback, and shell adapters | Strongly simplifies reproducibility; installer tests add necessary work |
| Separate quality and build jobs with locked dependencies | IRIS gets fast semantic feedback separately from platform compilation | DStack should retain Bash characterization alongside equivalent compiled jobs | Clearer failures with moderate CI cost |
| Install/update docs by channel | IRIS distinguishes Cargo, native packages, archives, and source updates | DStack initially needs archive/installer/source channels; package managers can wait | Simplifies user expectations without copying every package format |

### Patterns not to copy

- IRIS's image, PDF, TUI, theme, and terminal-protocol layers solve a different
  product and would be severe over-engineering for DStack.
- Its CLI is mostly flags around one viewer action; copying that flat model
  would make DStack's many operations ambiguous.
- Automatic creation of a large commented config file is unnecessary for a
  tool whose defaults can remain implicit.
- crates.io publication and multiple Linux package formats can be later
  distribution channels, not prerequisites for DStack's first binary release.
- The Makefile installer uses Unix `install -Dm755`; it is not a native Windows
  installer and offers no checksum or rollback.
- IRIS's Linux `/proc` producer detection is a consciously isolated
  platform-specific enhancement. DStack should copy the isolation principle,
  not that feature.

IRIS demonstrates that this maintainer can operate Rust CI and release
infrastructure, but it does not by itself select Rust for DStack.

## Major portability blockers

1. Business logic and parent-shell state are currently inseparable because all
   commands are sourced functions.
2. Registry/config paths and formats cannot represent ordinary cross-platform
   names and paths safely.
3. Shell word splitting, process substitution, and utility pipelines determine
   resolution, errors, and statuses.
4. Current Windows evidence is Git Bash, not native Windows.
5. The installer is Bash-only, mutable-branch based, and unverified.
6. Generic Docker convenience commands enlarge the portability and safety
   surface without serving the central stack-management product.

These blockers are addressable with a compact native core plus adapters; they
do not require four shell implementations or a Docker Engine API rewrite.
