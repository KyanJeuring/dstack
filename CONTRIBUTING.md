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

Phase 1 Rust validation uses:

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --locked
```
