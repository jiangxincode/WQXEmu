# Contributing to WQXEmu

Thank you for your interest in contributing to WQXEmu! This document provides guidelines and information for contributors.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [How to Contribute](#how-to-contribute)
- [Development Setup](#development-setup)
- [Code Style](#code-style)
- [Testing](#testing)
- [Pull Request Process](#pull-request-process)
- [Reporting Bugs](#reporting-bugs)
- [Feature Requests](#feature-requests)

## Code of Conduct

This project follows a standard code of conduct. Please be respectful and constructive in all interactions.

## How to Contribute

There are many ways to contribute:

- **Bug fixes** — fix issues in the emulator
- **New features** — add new functionality
- **Documentation** — improve or add documentation
- **Testing** — test game compatibility and report issues
- **Translations** — help translate the website or documentation

## Development Setup

1. **Fork the repository** on GitHub
2. **Clone your fork** locally:
   ```bash
   git clone https://github.com/YOUR_USERNAME/WQXEmu.git
   cd WQXEmu
   ```
3. **Add upstream remote**:
   ```bash
   git remote add upstream https://github.com/AloysHF/WQXEmu.git
   ```
4. **Install Rust** via [rustup](https://rustup.rs/)
5. **Build the project**:
   ```bash
   cargo build --release
   ```

## Code Style

- Use English for all comments and documentation
- Use `snake_case` for functions and variables
- Use `PascalCase` for types and structs
- Prefer `anyhow::Result` for error handling
- Use `log` crate for logging (not `println!`)
- Follow [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)

### Formatting

Run `cargo fmt` before committing to ensure consistent formatting:

```bash
cargo fmt
```

### Linting

Run `cargo clippy` to check for common issues:

```bash
cargo clippy -- -D warnings
```

## Testing

### Running Tests

Run all unit tests:

```bash
cargo test --workspace
```

### Smoke Tests

There are smoke tests that load games and verify they run without panicking:

```bash
# WQXEmu smoke test
cargo test -p wqxemu-core --test smoke -- --ignored --nocapture
```

These tests require game assets that are not distributed with the repository.

### Test Coverage

We aim for good test coverage. When adding new features, please include tests.

## Pull Request Process

1. **Create a feature branch** from `master`:
   ```bash
   git checkout -b feature/your-feature-name
   ```
2. **Make your changes** and commit them with clear, descriptive messages
3. **Run tests** to ensure everything passes:
   ```bash
   cargo test --workspace
   cargo clippy -- -D warnings
   cargo fmt -- --check
   ```
4. **Push to your fork** and create a pull request
5. **Describe your changes** in the PR description
6. **Wait for review** — maintainers will review your PR

### Commit Messages

Use clear, descriptive commit messages:

```
feat: add new feature
fix: fix bug description
docs: update documentation
test: add tests
refactor: refactor code
```

## Reporting Bugs

When reporting bugs, please include:

1. **Description** — clear description of the issue
2. **Steps to reproduce** — how to reproduce the bug
3. **Expected behavior** — what you expected to happen
4. **Actual behavior** — what actually happened
5. **Environment** — OS, Rust version, etc.
6. **Screenshots** — if applicable

## Feature Requests

Feature requests are welcome! Please:

1. **Check existing issues** to avoid duplicates
2. **Describe the feature** clearly
3. **Explain the use case** — why is this feature needed?
4. **Consider implementation** — how might this be implemented?

## Questions?

If you have questions about contributing, feel free to open an issue or reach out to the maintainers.

Thank you for contributing to WQXEmu!
