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

Rust tests and manual development commands must set
`DSTACK_CONFIG_DIR` to a temporary directory so they never read or write a
developer's real v1 or v2 state. The override contains `dstack.conf` and
`registry.toml`; its `legacy/registry` child remains the isolated v1 migration
fixture. Automated Rust tests inject a recording process executor and must
never require a Docker daemon, Docker Desktop, containers, or network pulls.
Interactive shell probes use null stdio; final Compose and direct Docker
streaming operations inherit the user's terminal and preserve exact native
arguments. Captured-process fakes must cover stdout bytes, numeric status,
spawn errors, and the null-stdin/captured-stdout/inherited-stderr policy. No
automated test may require a Docker daemon.

The same developer workflows are available through `make fmt-check`,
`make lint`, `make test`, `make build`, and `make ci` on Unix-like systems.
