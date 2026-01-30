# DStack

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
Supported **via WSL (Windows Subsystem for Linux)**:
- Works in WSL2 with Docker Desktop
- Native Windows shells (PowerShell / CMD) are **not supported**

> Recommended setup on Windows: **WSL2 + Docker Desktop**

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

# start a stack from anywhere
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

## Included commands (highlights)

| Command | Description |
|------|------------|
| `dhelp` | List available commands |
| `dstack` | List available stacks |
| `dstack add <name> <path>` | Register a stack |
| `dstackunset <name>` | Unregister a stack |
| `dcompose [stack]` | Build & start stack |
| `ddown [stack]` | Stop & remove stack |
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

Any direct subdirectory that contains a docker-compose.yml file is treated as a stack

These locations were chosen because they are widely used across Linux, macOS, and WSL environments

## Register external Compose projects

You can register **any directory** containing a `docker-compose.yml`:

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

## How stack resolution works?

When you run a command, `dstack` resolves the Compose context in this order:

1. Explicit stack name (`dcompose myproject`)
2. Active stack (`dstack myproject`)
3. Local `docker-compose.yml`

If no context is found, `dstack` tells you exactly what to do.

---

## Customizing discovery locations (advanced)

If you want full control over discovery paths, you can edit the script and adjust the discovery locations section:

- `~/projects`
- `~/src`
- `~/code`
- `/opt/services`
- `/your/custom/directory`

This keeps dstack simple and dependency-free, while still allowing advanced users to tailor it to their environment.

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

## Why dstack?

Docker Compose is great.

Having to `cd` into the right directory **every single time** is not.

That's where `DStack` comes in. It allows you to manage stacks no matter in what directory you are in.