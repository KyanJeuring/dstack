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

### 5. Run ShellCheck (recommended)

```bash
shellcheck DStack.sh
```

### 6. Submit a Pull Request

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
