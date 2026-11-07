<img src="./logo/dstack-logo.png">

![CI](https://github.com/kyanjeuring/dstack/actions/workflows/ci.yml/badge.svg)

**Docker Compose stack management**

> [!NOTE]
> The characterized Bash implementation remains the supported DStack v1
> release. DStack v2 is being developed alongside it in Rust, beginning at
> package version `2.0.0`. Its Cargo package is `dstack-compose`, while the
> executable remains `dstack`. The package is not published yet; do not replace
> an existing v1 installation with the development binary.

The in-development v2 foundation provides persistent registry, configuration,
discovery, context planning, and native Docker Compose execution. The process
boundary uses exact OS arguments and never reconstructs a shell command.
Developer-facing commands include:

```text
dstack config path
dstack config show
dstack registry list|show|add|remove
dstack migrate registry [--dry-run]
dstack list
dstack [-s NAME] [--file FILE]... context show
dstack [-s NAME] [--file FILE]... compose [--] [ARGS...]
dstack [-s NAME] [--file FILE]... up [--remove-orphans] [SERVICE...]
dstack [-s NAME] [--file FILE]... start|stop|restart|pull [SERVICE...]
dstack [-s NAME] [--file FILE]... down [--volumes]
dstack [-s NAME] [--file FILE]... recreate [--volumes]
dstack [-s NAME] [--file FILE]... update [SERVICE...]
dstack [-s NAME] [--file FILE]... rebuild [--no-cache] [SERVICE...]
dstack [-s NAME] [--file FILE]... services
dstack [-s NAME] [--file FILE]... logs [-f|--follow] [--tail N] [SERVICE...]
dstack [-s NAME] [--file FILE]... exec SERVICE [-- COMMAND...]
dstack [-s NAME] [--file FILE]... run SERVICE [-- COMMAND...]
dstack container list [-a|--all] [-f|--filter FILTER]...
dstack container ports [CONTAINER [PRIVATE_PORT]]
dstack container ip CONTAINER
dstack container stats [-a|--all] [--no-stream] [CONTAINER...]
dstack container inspect CONTAINER...
dstack image list [-a|--all] [-f|--filter FILTER]...
dstack volume list [-f|--filter FILTER]...
dstack volume inspect NAME...
dstack volume remove NAME...
dstack network list [-f|--filter FILTER]...
dstack network inspect NAME...
```

The first stack, configuration, registry, or migration operation creates
`~/.config/dstack/dstack.conf` and a versioned empty `registry.toml` when
absent. Host-scoped Docker utilities do not read or initialize DStack state.
Set `DSTACK_CONFIG_DIR` to a temporary directory for development and tests; it
replaces that root completely. These v2 commands are not yet a released user
interface.
V2 table output aligns by visible text width and uses restrained color only on
an interactive terminal; redirected output is plain, and `NO_COLOR` disables
optional styling.

Raw `dstack compose` forwards Compose arguments without lifecycle defaults.
Use the hard `--` boundary when a raw Compose argument is also owned by
DStack, such as `dstack compose -- --file special.yaml up`. The typed
`dstack up` command runs `up -d --build`; `update`, `rebuild`, and `recreate`
are fail-fast multi-step operations. Docker stdin, stdout, and stderr remain
inherited, and DStack returns Docker's numeric exit status when available.

`dstack services` lists the services defined by the resolved Compose model by
running `docker compose ... config --services`; it does not depend on running
containers. `dstack logs` is non-following by default and shows the last 100
lines. Use `--follow`/`-f` for streaming and `--tail N` for a non-negative
line count. Service operands retain their order, and DStack does not insert a
pager or rewrite Compose log output.

`dstack exec SERVICE` quietly probes `bash`, `sh`, `zsh`, then `ash` and opens
the first available interactive shell. An explicit command requires the hard
boundary form `dstack exec SERVICE -- COMMAND...`. Likewise,
`dstack run SERVICE` uses the configured service/image command, while
`dstack run SERVICE -- COMMAND...` supplies exact command argv. Typed `run`
always uses `--rm`. These operations inherit terminal streams; DStack never
joins, quotes, or evaluates command arguments through a host shell.

The `container`, `image`, `volume`, and `network` namespaces are host-scoped.
They call the installed `docker` CLI directly and never resolve a DStack
project, registry entry, Compose file, or `ResolvedContext`. Consequently,
`--stack` and `--file` are errors for every command in these namespaces.
Metadata list operations request machine-oriented Docker rows, validate them,
and render aligned DStack tables. Docker stderr remains inherited. Live stats,
inspect JSON, one-container port lookup, and explicit volume removal remain
Docker-owned streaming output. No v2 host command invokes a shell pipeline,
pager, or external table utility.

Phase 6 migrates the useful v1 host helpers to descriptive v2 commands:

| DStack v1 | DStack v2 development command |
|---|---|
| `dps` | `dstack container list` |
| `dpsa` | `dstack container list --all` |
| `dpsg foo` | `dstack container list --filter name=foo` |
| `dport` | `dstack container ports` |
| `dip CONTAINER` | `dstack container ip CONTAINER` |
| `dstats` | `dstack container stats` |
| `dinspect CONTAINER` | `dstack container inspect CONTAINER` |
| `dimg` | `dstack image list` |
| `dvol` | `dstack volume list` |
| `dvolrm NAME` | `dstack volume remove NAME` |
| `dvolinspect NAME` | `dstack volume inspect NAME` |
| `dnet` | `dstack network list` |
| `dnetinspect NAME` | `dstack network inspect NAME` |

The abbreviated v1 names are not Rust command aliases. Host-wide prune,
stop-all, and purge helpers remain deferred; Phase 6 only permits targeted
`volume remove NAME...` mutation.

`DStack` is a Bash tool that lets you manage **multiple Docker Compose projects from anywhere in the terminal**.

No more constantly changing directories just to run `docker compose up` or `down`.

Think of it as Docker Desktop-style convenience for Docker Compose, without a GUI.

---

## Who is this for?

DStack is for developers and operators who:

- Run multiple Docker Compose projects
- Work on servers, over SSH, or in homelabs
- Forget where compose files live
- Are tired of `cd` + `docker compose` loops
- Prefer inspectable shell tools over GUIs

---

## Features

- Run Docker Compose commands from **any directory**
- Named stacks (`dcompose myproject`, `ddown website`, etc.)
- Auto‑discovery of projects in common directories
- Register Compose projects from **any path**
- Optional active stack context (`dstack <name>`)
- Clean, UX‑focused error messages
- No dependencies beyond Docker, Bash, and standard POSIX tools
- Server and SSH friendly

---


## Platform availability

> [!IMPORTANT]
> Requires **Bash 4+** and a working `git` installation.

- Linux: fully supported  
- macOS: limited support; requires Bash 4+ and some commands may require GNU utilities 
- Windows: via **WSL** or **Git Bash**

> [!WARNING]
> DStack currently only supports Bash.
> Zsh, Fish, PowerShell, and CMD are not supported.

---

## Installation

### One‑line install (recommended)

```bash
curl -fsSL https://kyanjeuring.com/scripts/install-dstack | bash
```

### Installing a specific version

```bash
curl -fsSL https://kyanjeuring.com/scripts/install-dstack | DSTACK_VERSION=vx.y.z bash
```

This will:
- Install `dstack` into `~/.local/share/dstack`
- Automatically source it in your shell`.bashrc`

Restart your shell or run:

```bash
source ~/.bashrc
```

### Install via git clone

If you prefer not to use an installer script or pipe remote code into your shell, you can clone the repository directly:

```bash
git clone https://github.com/kyanjeuring/dstack.git ~/.local/share/dstack
chmod +x ~/.local/share/dstack/dstack.sh
```

Then source it into your shell config:

```bash
echo 'source ~/.local/share/dstack/dstack.sh' >> ~/.bashrc
```

---

## 15-second demo

```bash
# list stacks (auto-discovered + registered)
dstack

# compose and start a stack from anywhere
dcompose myproject

# follow logs without cd'ing
dlogs myproject
```

---

## Quick start

### List available stacks

```bash
dstack
```

This shows:
- **Registered stacks** (explicitly added)
- **Auto‑discovered stacks** (common project directories)

---

### Run commands without changing directory

```bash
dcompose myproject
ddown website
dlogs backend
```

No `cd`. No guessing where the project lives.

---

## The `dcompose` command

`dcompose` is a thin wrapper around Docker Compose that automatically resolves the correct Compose context.

It behaves like `docker compose`, but lets you run commands from anywhere by resolving:

1. An explicit stack name
2. The active stack (`dstack <name>`)
3. The current directory

If a stack directory contains multiple Compose files, you can choose which file to use with a positional filename or standard `-f/--file` flags.

### Default behavior

When no arguments are provided, `dcompose` runs:

```bash
docker compose [stack] up -d --build --remove-orphans
```

This provides a fast, consistent "bring everything up" experience.

### Full Docker Compose access

Any arguments passed to `dcompose` are forwarded directly to Docker Compose.

Examples:

```bash
dcompose up -d
dcompose mystack restart
dcompose down -v
dcompose mystack -f compose.yml -f compose.prod.yaml up -d
dcompose -f compose.prod.yaml logs -f
```

This allows advanced workflows without limiting Docker Compose functionality.

DStack also provides a set of convenience commands (`ddown`, `dlogs`, `dexec`, etc.) that wrap common Docker Compose operations with safer defaults and clearer intent.

---

## Included commands (highlights)

| Command | Description |
|------|------------|
| `dhelp` | List available commands |
| `dstack` | List available stacks |
| `dstack add <name> <path>` | Register a stack |
| `dstackunset <name>` | Unregister a stack |
| `dcompose [stack] [compose-file]` | Build & start stack |
| `ddown [stack]` | Stop & remove containers |
| `dlogs [stack]` | Follow logs |
| `dexec [stack] <service>` | Exec into container |

(Plus many more helpers for logs, rebuilds, cleanup, networking.)

All commands can also be run within a project without specifying the current stack name.

Example:
```bash
cd /path/to/project
dcompose
dlogs
```

---

## Auto-discovery locations

DStack automatically discovers Docker Compose projects in common development and server locations, so you don't have to manually register every stack

By default, DStack scans:

- `~/projects`
- `~/src`
- `~/code`
- `/opt/services`
- `C:/Users/<you>/projects (Windows / Git Bash)`
- `C:/Users/<you>/src`
- `C:/Users/<you>/code`

Any directory up to one level deep that contains a supported Docker Compose file is treated as a stack
(for example `media/jellyfin/`).

These locations were chosen because they are widely used across Linux, macOS, and WSL environments

Discovery is limited to shallow nesting (one level below each base directory) to keep behavior fast and predictable.

> [!IMPORTANT]
> When stacks are nested, DStack displays their relative path to avoid name collisions.

---

## Register external Compose projects

You can register any directory containing a supported Docker Compose file:

Outside a project
```bash
dstack add myproject /path/to/project
```

Inside a project:
```bash
cd /path/to/project
dstack add myproject .
```

After that:

```bash
dcompose myproject
dlogs myproject
```

This is especially useful for:
- Monorepos
- External drives
- Non‑standard directory layouts

---

## How stack resolution works

When you run a command, `dstack` resolves the Compose context in this order:

1. Explicit stack name (`dcompose myproject`)
2. Active stack (`dstack myproject`)
3. Local Docker Compose file in the current directory

DStack will use the first supported Compose file it finds by default, in a deterministic order.

When more than one Compose file is present, choose one with `dcompose <stack> <compose-file>`, `dcompose <stack> -f <compose-file>`, or combine multiple files with repeated `-f` flags.

If no context is found, `dstack` tells you exactly what to do.

---

## Docker Compose file support

DStack supports the standard Docker Compose filenames:

- `docker-compose.yml`
- `docker-compose.yaml`
- `compose.yml`
- `compose.yaml`
- Any additional file in the stack directory matching `*compose*.yml` or `*compose*.yaml`

When multiple files are present, DStack still selects the first matching file by default to keep existing workflows predictable.

To pick a different file, pass a filename or `-f/--file` flags to `dcompose`. Multiple files can be combined in the same command, matching Docker Compose's native behavior.

### Customizing Compose filenames (advanced)

By default, DStack looks for the standard Docker Compose filenames mentioned above.

Advanced users can override this behavior by setting the `DSTACK_COMPOSE_FILES` environment variable.

Example:

```bash
export DSTACK_COMPOSE_FILES="docker-compose.yml compose.yml"
```

This allows full control over which Compose files DStack considers during stack resolution.

> [!IMPORTANT]
> This is an advanced feature.
> Overriding Compose filenames can make stack discovery less predictable and is not recommended for most users.

---

## Customizing discovery locations (advanced)

By default, DStack auto-discovers Docker Compose projects in a small set of common directories, such as:
- `~/projects`
- `C:/Users/<you>/projects (Windows / Git Bash)`

### Override discovery paths (recommended)

Advanced users can override the discovery locations without editing the script by setting the `DSTACK_BASES` environment variable.

Example:


Add this to your shell config `.bashrc`, to make it permanent and reload the shell:
```bash
export DSTACK_BASES="/your/custom/path1 /your/custom/path2"
```

DStack will then use only these paths for auto-discovery.

### Editing the script (not recommended)

You can edit the discovery paths directly in the script, but this is discouraged, as it makes updating DStack harder and can lead to merge conflicts.

If you do edit the script, restart your shell or reload your configuration afterward.

This approach keeps DStack simple while still giving power users full control over their environment.

> [!IMPORTANT]
> Custom discovery paths must not contain spaces.

---

## Updating

Updating DStack follows the same method used for installation.

- If you installed via the installer, re-run the installer.
- If you installed via git clone, pull the latest changes.
- If you installed manually, repeat the manual install steps.

After updating, restart your shell or reload your config if needed.

---

## Uninstall

```bash
rm -rf ~/.local/share/dstack
```

Then remove the `dstack` source line from your shell config.

---

## Testing

DStack uses a dependency-free Bash characterization-test harness. The suite
validates its own infrastructure and records current DStack behavior in
isolated child shells. Docker commands are handled by a guarded fake executable.

Run the complete cross-platform, non-interactive characterization suite from
the repository root:

```bash
bash tests/characterization/run.sh
```

Run only the test-infrastructure self-tests:

```bash
bash tests/characterization/run.sh harness
```

Run only the DStack loading and shell-state characterization cases:

```bash
bash tests/characterization/run.sh load
```

Run the Linux interactive/TTY tier (requires Python 3 with Unix `pty` support):

```bash
bash tests/characterization/run.sh interactive
```

Run one case by its TAP name:

```bash
bash tests/characterization/run.sh case harness::fake_docker_applies_sequential_responses
```

Failed case directories are normally removed. Preserve a failed case's capture
files for debugging with:

```bash
bash tests/characterization/run.sh --debug case <case-name>
```

These tests exist so future characterization results can be trusted before
DStack is refactored. Every case receives a temporary `HOME`, project tree,
registry location, Docker configuration, and controlled `PATH`. Docker calls
go to a guarded fake executable that records exact argument boundaries and
configured responses. A fail-closed fallback prevents the suite from silently
using a real Docker CLI, so no Docker daemon is required.

The in-progress Rust v2 implementation has a separate native test suite. Run
its formatting, lint, and test gates with:

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --all-features --locked
```

V2 registry tests use `DSTACK_CONFIG_DIR` to isolate all persistent state.
The current v1 runtime and its real registry are never used by those tests.

---

## Contributing
Contributions are welcome!

Please read [CONTRIBUTING.md](CONTRIBUTING.md) before submitting a PR.

---

## Support

If you found this project useful and would like to support my work:

[https://buymeacoffee.com/kyanjeuring](https://buymeacoffee.com/kyanjeuring)

## License

MIT License

---
