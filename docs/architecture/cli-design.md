# DStack CLI design

This document records the approved DStack v2 Rust `2.0.0` command direction.
Exact options for commands not yet implemented can still be refined in their
migration phase, but the unified executable, explicit targeting, typed/raw
split, and absence of positional stack guessing are fixed product decisions.

## Product boundary

DStack's primary job is to target a Docker Compose project from anywhere and
run useful Compose workflows against it. Registry, discovery, active context,
Compose-file selection, and exact Docker argv are core. Generic aliases for
`docker ps`, image/volume/network listing, and host-wide prune are historical
conveniences rather than the product's defining capability.

The future CLI should be one executable named `dstack`. Shell integrations may
provide temporary legacy functions, but business logic and command parsing
must not be duplicated across shells.

## Model comparison

| Criterion | Model A: 43 current-style `d*` commands | Model B: one executable with subcommands |
|---|---|---|
| Discoverability | Requires README or `dhelp`; shell completion must know many names | `dstack --help` exposes one tree; completion follows the parser |
| Learnability | Short after memorization, inconsistent families such as `dlog`/`dllog` | Longer but regular nouns, verbs, and flags |
| Ambiguity | First token may be stack, verb, service, file, or argument | Target is an explicit global option; operands have typed positions |
| Shell independence | Requires sourced functions | Native executable works from every shell |
| Scriptability | Widespread status masking and stdout diagnostics | Stable exit classes and stream policy can be documented |
| Compatibility | Exact current surface | Requires adapters and deprecation |
| Extensibility | More top-level function names and hand-written help | Subcommands can grow without namespace collisions |
| Implementation | One Bash function per shortcut | One parser and shared resolver/process layer |

Recommendation: Model B. Preserve common workflows with a bounded shell
compatibility layer rather than shaping the permanent CLI around current
parser accidents.

## Command grammar

The grammar has global DStack-owned targeting and Compose-selection flags:

```text
dstack [GLOBAL OPTIONS] <COMMAND> [COMMAND OPTIONS] [OPERANDS]

GLOBAL OPTIONS
  -s, --stack <NAME>      target a named registered/discovered stack
  --project-dir <PATH>    target an explicit project directory
  --file <PATH>           select a Compose file; repeatable
  --all-files             select every discovered Compose candidate
  --non-interactive       never prompt; unresolved choices are errors
  --color <auto|always|never>
  --quiet                 suppress DStack informational messages
```

`--stack` and `--project-dir` conflict. Explicit target wins over session
context and exact-current-directory fallback. Files are resolved relative to
the selected project unless absolute. Repeated `--file` preserves order.
DStack-owned options are position-independent around typed commands whenever
the parse is unambiguous:

```text
dstack -s jellyfin logs --file compose.prod.yaml
dstack logs --file compose.prod.yaml -s jellyfin
dstack --file compose.prod.yaml logs -s jellyfin
```

There is no positional stack prefix. For raw passthrough, the boundary rules
below take precedence over position-independent option recognition.

The target is never inferred from a subcommand operand. These examples are
therefore unambiguous:

```text
dstack --stack logs logs
dstack --stack api restart api
dstack logs api --follow
```

In the first example, the stack is `logs` and the subcommand is `logs`. In the
second, the stack and service can both be `api`.

### No-argument behavior

For migration ergonomics, `dstack` with no arguments should perform
`dstack list`. `dstack --help` is the complete command guide and
`dstack --version` is the version interface. This is a deliberate shortcut,
not an absent dispatcher.

### Passthrough boundaries

`dstack compose` is the raw Compose escape hatch. An optional `--` immediately
after `compose` is consumed as a hard DStack delimiter. Every later token—or
every token after `compose` when the delimiter is omitted—is raw Docker Compose
argv. DStack options for this command must therefore occur before `compose`:

```text
dstack --stack web --file compose.prod.yml compose logs -f
dstack --stack web compose -- --ansi never config
```

The separator is optional because the parser stops interpreting DStack flags
at `compose`; it is recommended when it clarifies intent. Apart from consuming
that delimiter, DStack preserves each argument exactly, appends no shell, and
does not evaluate metacharacters.

Typed commands use `--` to separate a command executed inside a service:

```text
dstack --stack web exec api
dstack --stack web exec api -- sh -lc 'printf "%s\n" "$HOME"'
dstack --stack web run worker -- python -m jobs.once
```

`exec SERVICE` without a command retains the current convenience of opening
a shell, but v2 will probe a documented candidate set such as `bash`, `sh`,
`zsh`, and `ash` rather than permanently assume `sh`. The Docker execution
phase will define and test the exact policy. An explicit command always wins.

## Core UX decisions

- `dhelp` becomes `dstack --help`.
- `dversion` becomes `dstack --version`.
- `dstack list`, `dstack use NAME`, and `dstack unset` separate listing and
  session context. Bare `dstack` remains a list shortcut. Effective listing
  includes both registered and discovered stacks and identifies their source.
- Registration is explicit: `dstack registry add NAME PATH`, `remove NAME`,
  and `list`. It is not conflated with session activation. A registered name
  is a profile and may carry ordered default Compose files; multiple names may
  point to one physical project.
- `dcompose` becomes `dstack compose`.
- `dlogs`, `dlog`, `dllogs`, and `dllog` become one `dstack logs [SERVICE]`
  command with `--tail`, `--follow`, and `--pager`/`--no-pager`.
- `dexec` and `drun` become `dstack exec` and `dstack run`.
- `dstack update` updates the selected stack. `dstack self-update` is a
  separate, installation-channel-aware operation implemented in a later phase.
- Rebuild behavior is `dstack rebuild [SERVICE...] [--no-cache]`.
- Recreate behavior is `dstack recreate [--volumes]`; the explicit destructive
  flag represents intent without a redundant confirmation.
- `dstack down --volumes` replaces `ddownv` without adding another command.
- Host-wide prune/stop helpers are not part of the permanent DStack CLI.
- Docker Compose configuration remains raw `dstack compose config`. Top-level
  `dstack config` is reserved for future DStack-owned configuration and is not
  created merely to preserve `dconfig`'s name.

Flags are appropriate for orthogonal modifiers (`--stack`, `--file`,
`--follow`, `--tail`, `--volumes`, `--no-cache`). Actions with separate help,
validation, or operands are subcommands (`list`, `use`, `logs`, `registry`).
`--list` is not recommended because it makes an action look like a global
mode and scales poorly next to registry/context lists.

Compose-file precedence is explicit CLI `--file`/`--all-files`, then a file
selection stored in the current session, then registered-profile default
files, then automatic discovery. Any explicit CLI selection replaces, rather
than appends to, lower-precedence selections.

## Current public command mapping

All 43 characterized public functions are accounted for below.

| Current command | Current purpose | Proposed future interface | Preserve as alias/wrapper? | Deprecate? | Notes |
|---|---|---|---|---|---|
| `dclean` | Force-prune stopped containers | Direct `docker container prune` | Optional legacy pack only | Yes | Generic host-wide Docker operation |
| `dcleanall` | Confirm then system prune `-a` | Direct `docker system prune -a` | No default wrapper | Yes | Dangerous and outside stack scope |
| `dcleani` | Force-prune dangling images | Direct `docker image prune` | Optional legacy pack only | Yes | Generic Docker operation |
| `dcompose` | Resolve context and run arbitrary Compose argv | `dstack compose [--] ARGS...` | Yes | Yes | High-value migration alias; preserve exact argv |
| `dconfig` | Show resolved Compose config | `dstack compose config` | Yes | Yes | Top-level `config` is reserved for DStack configuration |
| `ddown` | Compose down, preserve volumes | `dstack down` | Yes | Yes | Project-wide; target via global flag |
| `ddownv` | Confirmed down with volumes | `dstack down --volumes` | Yes | Yes | Project-wide; explicit flag is sufficient v2 intent |
| `dexec` | Open `sh` in a service | `dstack exec SERVICE [-- COMMAND...]` | Yes | Yes | Explicit argv wins; omitted command uses the later shell-probe policy |
| `dhelp` | Parse comments and list functions | `dstack --help` | Yes | Yes | Wrapper calls executable help; no source parsing |
| `dimg` | List Docker images | Direct `docker images` | Optional legacy pack only | Yes | Not stack management |
| `dinspect` | Page `docker inspect` | Direct `docker inspect` | Optional legacy pack only | Yes | No hard-coded pager in core |
| `dip` | Print container IP | Direct Docker inspect formatting | Optional legacy pack only | Yes | Historical convenience |
| `dllog` | Paged service logs | `dstack logs SERVICE --no-follow --pager` | Yes | Yes | Fix one-argument defect in new interface |
| `dllogs` | Paged all-service logs | `dstack logs --no-follow --pager` | Yes | Yes | Same parser as other logs forms |
| `dlog` | Follow one service, tail 100 | `dstack logs SERVICE --follow [--tail N]` | Yes | Yes | Tail becomes explicit and documented |
| `dlogs` | Follow all services | `dstack logs --follow [--tail N]` | Yes | Yes | Preserve common workflow |
| `dnet` | List Docker networks | Direct `docker network ls` | Optional legacy pack only | Yes | Not stack management |
| `dnetinspect` | Inspect Docker network | Direct `docker network inspect` | Optional legacy pack only | Yes | Not stack management |
| `dport` | Show container ports | Direct `docker ps --format ...` | Optional legacy pack only | Yes | Avoid formatter dependency |
| `dprunewhat` | Docker system prune dry run | Direct `docker system prune --dry-run` | Optional legacy pack only | Yes | Host-wide; Docker owns it |
| `dps` | Formatted running containers | Direct `docker ps` | Optional legacy pack only | Yes | Remove `sed`/`column` pipeline |
| `dpsa` | Formatted all containers | Direct `docker ps -a` | Optional legacy pack only | Yes | Same |
| `dpsg` | Grep running containers | Docker filters or direct pipeline | Optional legacy pack only | Yes | DStack should not define grep semantics |
| `dpull` | Pull stack images | `dstack pull [SERVICE...]` | Yes | Yes | Propagate Docker failure |
| `drebootstack` | Down then up, preserve volumes | `dstack recreate` | Yes | Yes | Stop on failed down by default |
| `drebuild` | Build and up one service | `dstack rebuild [SERVICE...]` | Yes | Yes | Explicit target removes two-token ambiguity |
| `drebuildnocache` | Build all no-cache then up | `dstack rebuild --no-cache [SERVICE...]` | Yes | Yes | One coherent rebuild command |
| `drecompose` | Down volumes then up | `dstack recreate --volumes` | Yes, warning | Yes | Destructive alias for bounded migration |
| `drestart` | Restart one/all services | `dstack restart [SERVICE...]` | Yes | Yes | Typed service operands |
| `drun` | Compose run `--rm` | `dstack run SERVICE [-- COMMAND...]` | Yes | Yes | Preserve exact trailing argv |
| `dstack` | List/register/select stacks | Executable: no args/`list`, `use`, `registry` | Binary supports narrow transitional old grammar | Partially | Permanent name, redesigned actions |
| `dstackpurge` | Down volumes plus host system prune | Split: `dstack down --volumes`; direct Docker prune | No default wrapper | Yes | Scope jump is unsafe |
| `dstackunset` | Unregister, nominally clear active | `dstack registry remove NAME`; separately `dstack unset` | Yes | Yes | Wrapper may perform both for compatibility |
| `dstart` | Compose start | `dstack start [SERVICE...]` | Yes | Yes | Target via global flag |
| `dstats` | Live Docker stats | Direct `docker stats` | Optional legacy pack only | Yes | Long-running generic Docker operation |
| `dstop` | Compose stop | `dstack stop [SERVICE...]` | Yes | Yes | Target via global flag |
| `dstopall` | Stop every running container | No DStack equivalent | No | Yes, remove early | Dangerous host-wide action |
| `dsvc` | List Compose services | `dstack services` | Yes | Yes | Core stack-oriented inspection |
| `dupdate` | Pull then up | `dstack update [SERVICE...]` | Yes | Yes | Stop if pull fails |
| `dversion` | Print DStack version | `dstack --version` | Yes | Yes | Standard CLI convention |
| `dvol` | List Docker volumes | Direct `docker volume ls` | Optional legacy pack only | Yes | Generic Docker operation |
| `dvolinspect` | Inspect a Docker volume | Direct `docker volume inspect` | Optional legacy pack only | Yes | Generic Docker operation |
| `dvolrm` | Remove a Docker volume | Direct `docker volume rm` | Optional legacy pack only | Yes | Destructive generic operation |

## Direct Docker helpers

The permanent CLI should not add a `dstack docker` namespace merely to wrap
commands Docker already names well. Such a namespace adds documentation,
status, and safety obligations without using DStack's stack resolver.

Classifications:

- **Core DStack:** `services`, Compose config, lifecycle, logs, exec, run,
  registry/discovery/context.
- **Useful compatibility aliases:** current logs/lifecycle/Compose functions
  because users employ them as stack-aware operations.
- **Legacy-only convenience:** container/image/volume/network information
  helpers. Offer them only in an opt-in legacy adapter for one release window.
- **Remove or require direct Docker:** `dstopall`, prune helpers, generic volume
  removal, and combined `dstackpurge`.

## Destructive command policy

Stack-local and host-wide scope must never be combined.

- `dstack down --volumes` and `dstack recreate --volumes` explicitly name
  stack-local volume deletion. Supplying the flag is sufficient intent; v2
  does not add a redundant confirmation.
- DStack must never silently expand a stack-local action into host-wide scope.
- DStack does not provide stop-all or host prune in the permanent surface.
- Raw `dstack compose` remains capable of destructive Docker behavior; it is
  deliberately an advanced passthrough and does not pretend to infer safety.

## Active stack and session UX

```text
dstack use media/jellyfin
dstack context show
dstack logs --follow
dstack unset
```

`use` requires shell initialization and writes a per-shell session context via
the core. Without an adapter/session, it explains that the user can either
initialize their shell or use `--stack` for a one-shot command. It must not
silently create a machine-global active stack shared by every terminal.

The session stores a stable stack reference plus optional Compose selections,
not only a path. Resolution revalidates it before each operation. Invalid
active context is an error; it does not silently fall back to cwd.

## Compatibility plan

Default shell initialization should temporarily define wrappers for:

```text
dhelp dversion dstackunset dsvc dstart dstop dcompose ddown ddownv
drecompose drebootstack drestart dlogs dlog dllogs dllog dexec drun
dpull dupdate drebuild drebuildnocache dconfig
```

They translate argv to the new executable and emit one concise deprecation
warning per shell session. They must not reimplement discovery or resolution.
Wrappers for generic Docker helpers belong in an explicitly requested
`dstack shell init <shell> --legacy-docker-shortcuts` mode and should have a
shorter support window. Do not provide default wrappers for `dstopall`,
`dstackpurge`, or `dcleanall`.

Proposed duration: wrappers documented for two minor releases, warnings become
prominent in the next major prerelease, and wrappers are removed at that major
release. Release count is more predictable than elapsed calendar time. This
schedule requires maintainer approval.

The executable itself should not be installed under 43 names. During the
window it may recognize narrow deprecated `dstack add`, `dstack ls`, and
`dstack NAME` grammar after known subcommands have been checked. That parser is
isolated as compatibility code and removed with the wrappers.

## Lessons from IRIS

IRIS uses Clap's derive API to generate `iris --help` and `iris --version`,
parses its file operand as a native path, and keeps `main` as a thin error
boundary. Its installation documentation consistently names one executable.
These are clean patterns for DStack.

IRIS has one principal action—open an input—so a flat option set works. DStack
has registry, context, lifecycle, log, execution, and migration actions; a flat
copy of IRIS's CLI would replace one ambiguity with many flags. DStack should
reuse typed parsing and automatic help, not IRIS's exact shape. IRIS also has
no parent-shell state problem or legacy family of 43 sourced functions.

The IRIS README does well at separating install/update/uninstall instructions
by distribution channel. DStack's CLI reference should similarly distinguish
core commands, raw Compose passthrough, shell initialization, and legacy
wrappers instead of mixing them in one highlights table.

## Approved CLI direction

```text
dstack [-s NAME | --stack NAME | --project-dir PATH]
       [--file PATH]... [--all-files]
       [--non-interactive] [--color MODE] [--quiet]
├── list                         # also the no-argument shortcut
├── use NAME [--file PATH]...    # requires initialized shell session
├── unset
├── context
│   └── show
├── registry
│   ├── list
│   ├── add NAME PATH
│   └── remove NAME
├── services
├── compose [--] ARGS...
├── up [SERVICE...] [--remove-orphans]
├── start [SERVICE...]
├── stop [SERVICE...]
├── down [--volumes]
├── recreate [--volumes]
├── restart [SERVICE...]
├── pull [SERVICE...]
├── update [SERVICE...]
├── rebuild [SERVICE...] [--no-cache]
├── logs [SERVICE] [--tail N] [--follow] [--pager | --no-pager]
├── exec SERVICE [--] [COMMAND...]
├── run SERVICE [--] [COMMAND...]
├── self-update
├── shell
│   └── init <bash|zsh|fish|powershell> [--legacy]
├── completion <bash|zsh|fish|powershell>
└── migrate
    └── registry [--dry-run] [--backup PATH]

dstack --help
dstack --version
```

`up` provides the approved typed default `up -d --build`; service operands are
appended when present. `--remove-orphans` is explicit and is not part of the
default. Raw Compose users retain exact Compose semantics through `compose`, so
`dstack compose up` receives none of the typed defaults. Bare `dstack` remains
the effective-list shortcut.
