# Migrating from DStack v1 to v2

DStack v1.11.0 is the final legacy Bash release. DStack v2 is a native Rust
executable with an intentionally breaking command-line interface. It does not
provide aliases for the old v1 helper functions.

## Before you begin

The historical production v1 installer placed `dstack.sh` under
`~/.local/share/dstack` and added a source line to `.bashrc` or `.zshrc`.
Sourcing that file defines a shell function named `dstack`. A shell function
takes precedence over a same-named executable, so installing v2 alone may
leave both existing and newly opened terminals invoking v1.

Inspect resolution before and after migration:

```bash
type -a dstack
type dstack
```

Use the exact v2 executable path printed by the installer while the v1
function may still be active. The default Unix path is
`$HOME/.local/bin/dstack`; the default native Windows path is
`$HOME\.local\bin\dstack.exe`. If you selected another install directory, use
that exact path instead. `command dstack` bypasses shell functions in Bash when
PATH already resolves the correct binary, but the absolute path is safest.

## Migration procedure

1. Install the v2 binary without deleting v1.
2. Back up the relevant shell profile and legacy registry.
3. Preview and run registry migration through the v2 executable.
4. Remove or disable the confirmed v1 source line manually.
5. Install v2 shell integration through the executable.
6. Reload the profile or open a new terminal.
7. Verify that the v2 executable or managed wrapper is active.
8. Verify the registry, discovered stacks, and a new terminal selection.
9. Only then, optionally delete the old local v1 runtime.

Example for the default Unix installation:

```bash
cp "$HOME/.config/dstack/registry" \
  "$HOME/.config/dstack/registry.v1-backup"
cp "$HOME/.bashrc" "$HOME/.bashrc.dstack-v1-backup"

"$HOME/.local/bin/dstack" migrate registry --dry-run
"$HOME/.local/bin/dstack" migrate registry
```

Back up `.zshrc` instead when that is where v1 was sourced. If the legacy
registry does not exist, the migration command reports that clearly and does
not guess another source.

The migration reads `~/.config/dstack/registry`, imports valid non-conflicting
entries into the versioned v2 registry, reports malformed/conflicting entries,
and never modifies the legacy source file. Repeat migration is safe for
already-identical entries.

The supported command sequence is:

```text
dstack migrate registry --dry-run
dstack migrate registry
```

Use the exact executable path form shown above until the v1 function has been
removed from the active profile and shell.

## Remove the old profile source line

Do not delete arbitrary profile content. Inspect the profile first:

```bash
grep -n 'dstack\.sh' "$HOME/.bashrc" "$HOME/.zshrc" 2>/dev/null
```

The historical production installer wrote a block shaped like this, with the
home directory expanded to an absolute path:

```bash
# DStack
source /home/you/.local/share/dstack/dstack.sh
```

Remove or comment out only the confirmed v1 block in the backed-up profile.
Installers do not perform this cleanup automatically because a profile may have
been moved, edited, or integrated manually.

Then install the new managed adapter and completion block through the exact v2
binary:

```bash
"$HOME/.local/bin/dstack" shell install --shell bash
```

Use `--shell zsh`, `fish`, or `powershell` as appropriate, and use
`--profile PATH` for a custom profile. Follow the printed reload instruction or
open a new terminal.

Verify the result:

```bash
dstack --version
dstack shell status
dstack registry list
dstack list
dstack use NAME
dstack context show
dstack unset
```

`dstack --version` must report a 2.x version. In Bash/Zsh/Fish/PowerShell the
managed wrapper is expected to handle only `use` and `unset` while delegating
all other commands to the executable.

## Session behavior changed

The v1 variables `DSTACK` and `DSTACK_COMPOSE_FILE` do not migrate and are not
read as v2 state. V2 uses a versioned `DSTACK_SESSION` payload owned by the
shell adapter. Select a fresh context in every terminal that needs one:

```bash
dstack use NAME
```

Registry profiles are persistent; a selected session is terminal-local.
Removing a registry profile does not clear an already captured terminal
snapshot, and `dstack unset` does not remove the profile.

## Command mapping

| v1 workflow | v2 command |
|---|---|
| `dstack` | `dstack list` |
| `dstack NAME` | `dstack use NAME` |
| `dstack add NAME PATH` | `dstack registry add NAME PATH` |
| `dstackunset NAME` | `dstack registry remove NAME` |
| `dcompose ...` | `dstack compose ...` or typed `dstack up` |
| `dstart` | `dstack start` |
| `dstop` | `dstack stop` |
| `ddown` | `dstack down` |
| `dupdate` | `dstack update` |
| `dlogs` | `dstack logs` |
| `dexec` | `dstack exec` |
| `drun` | `dstack run` |
| `dimg` | `dstack image list` |
| `dvol` | `dstack volume list` |
| `dnet` | `dstack network list` |
| `dclean` | `dstack container prune --force` |
| `dcleani` | `dstack image prune --force` |
| `dcleanall` | `dstack system prune --all`; add `--force` only intentionally |
| `dstopall` | `dstack container stop --all` |

`dstack update` pulls images and runs Compose `up -d`. Add
`--remove-orphans` only when you intentionally want that `up` step to remove
Compose-project containers for services no longer defined by the current
Compose model.

V2 deliberately does not replace the mixed stack-and-system behavior of
`dstackpurge`. Run explicit stack teardown and host maintenance as separate
commands after reviewing each scope. `dprunewhat` also has no heuristic
replacement because Docker exposes no trustworthy general prune preview.

## Compose, logs, exec, and run differences

- V2 resolves `--stack`, then `DSTACK_SESSION`, then the exact current
  directory. It never searches parent directories.
- Ordered repeated `--file` values are explicit. Otherwise DStack checks the
  configured Compose filenames in order and selects the first existing file;
  it does not present v1's interactive multi-file selector.
- `dstack compose` is raw forwarding and adds no default `up` operation. Use
  typed `dstack up` for detached, build-aware startup.
- `dstack logs` defaults to 100 lines and does not follow. Use `--follow` and
  `--tail N` explicitly; v2 does not insert a pager.
- `dstack exec SERVICE` probes common interactive shells. Explicit command
  arguments require `dstack exec SERVICE -- COMMAND...`.
- `dstack run SERVICE -- COMMAND...` preserves exact argv and always uses
  Compose `--rm`.
- Docker-facing failures propagate their status instead of being broadly
  masked as successful v1 helpers often were.

## Destructive commands

V2 uses explicit namespaces and long-form destructive options. Docker owns
prune confirmation unless `--force` is supplied. `dstack system prune` is
host-wide, and `dstack container stop --all` affects every running container
visible to the active Docker daemon/context, not only registered stacks.

Review `dstack COMMAND --help` before replacing v1 cleanup automation. V2 adds
no compatibility aliases and does not automatically delete profiles, legacy
files, registries, or Docker resources during installation or migration.

## Self-update and package ownership

Standalone installations use:

```bash
dstack self-update --check
dstack self-update
```

System package installations must be updated with their package manager.
Cargo-managed installations must be updated through Cargo. The self-updater
checks compatible stable v2 GitHub releases, verifies checksums, and does not
support automatic downgrades.

## Rollback

For a standalone v2 installation, reinstall a known stable 2.x release with
the published installer contract. For the first v2 release:

```bash
curl -fsSL \
  https://github.com/KyanJeuring/dstack/releases/latest/download/install-dstack.sh \
  | DSTACK_VERSION=v2.0.0 sh
```

This is explicit version pinning, not an updater downgrade.

The historical [v1.11.0 tag and release](https://github.com/KyanJeuring/dstack/releases/tag/v1.11.0)
remain available. If you retained your old v1 files, you can temporarily
restore the backed-up profile source line to return to them. Do not expect that
path to work after manually deleting the legacy runtime; obtain the historical
`dstack.sh` from the v1.11.0 tag first and review it before sourcing.
