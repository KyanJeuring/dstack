  <img src="./logo/dstack-logo.png">

**Docker Compose stack management**

`DStack` is a small, zero‑dependency Bash tool that lets you manage **multiple Docker Compose projects from anywhere in the terminal**.

No more constantly changing directories just to run `docker compose up` or `down`.

Think of it as Docker Desktop-style convenience for Docker Compose, without a GUI.

---

## Who is this for?

DStack is for developers and operators who:

- Run multiple Docker Compose projects
- Work on servers, over SSH, or in homelabs
- Forget where compose files live
- Are tired of `cd` + `docker compose` loops
- Prefer small, inspectable shell tools over GUIs

---

## Features

- Run Docker Compose commands from **any directory**
- Named stacks (`dcompose myproject`, `ddown website`, etc.)
- Auto‑discovery of projects in common directories
- Register Compose projects from **any path**
- Optional active stack context (`dstack <name>`)
- Clean, UX‑focused error messages
- No dependencies beyond Docker + Bash
- Server and SSH friendly

---

## Platform support

### Linux
Fully supported and tested.

### macOS
Fully supported:
- Works with Docker Desktop for Mac
- Supports Bash and Zsh

### Windows

Supported via WSL2 and Git Bash:

#### WSL2 Fully supported

Works with Docker Desktop <br>
Recommended for the best experience

#### Git Bash (native Windows)

Supported and tested <br>
Requires Docker Desktop

Uses Windows-style paths (C:/...)

Recommended setup on Windows:<br>
WSL2 + Docker Desktop<br>
Git Bash support is provided for users who prefer a native Windows shell.

PowerShell and CMD are not supported

---

## Installation

### One‑line install (recommended)

```bash
curl -fsSL https://kyanjeuring.com/scripts/install-dstack.sh | bash
```

### Installing a specific version

```bash
curl https://kyanjeuring.com/scripts/install-dstack.sh | DSTACK_VERSION=vx.y.z bash
```

This will:
- Install `dstack` into `~/.local/share/dstack`
- Automatically source it in your shell (`.bashrc` / `.zshrc`)

Restart your shell or run:

```bash
source ~/.bashrc   # or ~/.zshrc
```

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

### Default behavior

When no arguments are provided, `dcompose` runs:

```bash
docker compose [stack] up -d --build --remove-orphans
```

This provides a fast, consistent "bring everything up" experience.

Full Docker Compose access
Any arguments passed to `dcompose` are forwarded directly to Docker Compose.

Examples:

```bash
dcompose up -d
dcompose mystack restart
dcompose down -v
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
| `dcompose [stack]` | Build & start stack |
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

Supported filenames:

- `docker-compose.yml`
- `docker-compose.yaml`
- `compose.yml`
- `compose.yaml`

These locations were chosen because they are widely used across Linux, macOS, and WSL environments

Discovery is intentionally limited to one level of nesting to keep behavior fast and predictable.

### Important note:
```
When stacks are nested, DStack displays their relative path to avoid name collisions.
```

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

DStack will use the first supported Compose file it finds, in a deterministic order.

If no context is found, `dstack` tells you exactly what to do.

---

## Docker Compose file support

DStack supports the standard Docker Compose filenames:

- `docker-compose.yml`
- `docker-compose.yaml`
- `compose.yml`
- `compose.yaml`

When multiple files are present, DStack selects the first matching file in a fixed order to ensure predictable behavior.

DStack does not automatically combine multiple Compose files.
If you rely on overrides (for example `docker-compose.override.yml`), manage those explicitly via Docker Compose itself.

### Customizing Compose filenames (advanced)

By default, DStack looks for the standard Docker Compose filenames mentioned above.

Advanced users can override this behavior by setting the `DSTACK_COMPOSE_FILES` environment variable.

Example:

```bash
export DSTACK_COMPOSE_FILES="docker-compose.yml compose.yml"
```

This allows full control over which Compose files DStack considers during stack resolution.

### Important note:
```
This is an advanced feature.
Overriding Compose filenames can make stack discovery less predictable and is not recommended for most users.
```

---

## Customizing discovery locations (advanced)

By default, DStack auto-discovers Docker Compose projects in a small set of common directories, such as:
- `~/projects`
- `C:/Users/<you>/projects (Windows / Git Bash)`

### Override discovery paths (recommended)

Advanced users can override the discovery locations without editing the script by setting the `DSTACK_BASES` environment variable.

Example:


Add this to your shell config (.bashrc, .zshrc, etc.) to make it permanent and reload the shell:
```bash
export DSTACK_BASES="/your/custom/path1 /your/custom/path2"
```

DStack will then use only these paths for auto-discovery.

### Editing the script (not recommended)

You can edit the discovery paths directly in the script, but this is discouraged, as it makes updating DStack harder and can lead to merge conflicts.

If you do edit the script, restart your shell or reload your configuration afterward.

This approach keeps DStack simple and dependency-free, while still giving power users full control over their environment.

### Important note:
```
Custom discovery paths must not contain spaces.
```

---

## Updating

### Update to the latest version

To update DStack, simply re-run the installer:

```bash
curl -fsSL https://kyanjeuring.com/scripts/install-dstack.sh | bash
```

This will:

- Fetch the latest release
- Replace the existing installation in ~/.local/share/dstack
- Keep your shell configuration intact

Restart your shell or reload your config if needed:

```bash
source ~/.bashrc   # or ~/.zshrc
```

### Update to a specific version

If you want to pin or roll back to a specific version:

```bash
curl https://kyanjeuring.com/scripts/install-dstack.sh | DSTACK_VERSION=vx.y.z bash
```

This is useful if:
- You want a known, stable version
- You’re debugging a regression

You manage multiple machines and want consistency

---

## Uninstall

```bash
rm -rf ~/.local/share/dstack
```

Then remove the `dstack` source line from your shell config.

---

## Contributing
Contributions are welcome!

Please read [CONTRIBUTING.md](CONTRIBUTING.md) before submitting a PR.

---

## License

MIT License

---