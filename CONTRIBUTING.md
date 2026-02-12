# Contributing to Polymarket AI Trading Agent

Thank you for your interest in contributing! This document provides guidelines for contributing to the project.

## Getting Started

1. **Fork** the repository on GitHub
2. **Clone** your fork locally:
   ```bash
   git clone https://github.com/YOUR_USERNAME/polymarket-agent.git
   cd polymarket-agent
   ```
3. **Create a branch** for your change:
   ```bash
   git checkout -b feature/your-feature-name
   ```
4. **Make your changes**, following the code style guidelines below
5. **Test** your changes:
   ```bash
   cargo test
   cargo clippy -- -D warnings
   ```
6. **Commit** with a clear message:
   ```bash
   git commit -m "Add: brief description of change"
   ```
7. **Push** to your fork and open a **Pull Request**

## Code Style

- Run `cargo fmt` before committing -- all code must be formatted with rustfmt
- Run `cargo clippy -- -D warnings` -- no clippy warnings allowed
- Write doc comments (`///`) for all public functions and structs
- Use meaningful variable names; avoid single-letter names except for iterators
- Keep functions focused -- one function, one responsibility
- Add `#[cfg(test)]` unit tests for new logic

## Commit Messages

Use clear, descriptive commit messages:

```
Add: new weather specialist desk
Fix: Kelly criterion edge case with zero probability
Update: improve Bull analyst prompt for crypto markets
Refactor: extract common analysis traits
Docs: add setup guide for Windows
```

Prefix with: `Add`, `Fix`, `Update`, `Refactor`, `Docs`, `Test`, `Chore`

## Pull Request Process

1. Update documentation if your change affects user-facing behavior
2. Add tests for new functionality
3. Ensure CI passes (build, test, clippy, fmt)
4. Fill out the PR template completely
5. Request review from a maintainer

## What to Contribute

Great areas for contribution:

- **New specialist desks** -- Add analysis for new market categories
- **Improved prompts** -- Better AI prompts for more accurate analysis
- **Data sources** -- Integrate new data APIs for market research
- **Bug fixes** -- Check the issue tracker for known bugs
- **Documentation** -- Improve guides, add examples, fix typos
- **Tests** -- Increase test coverage
- **Performance** -- Optimize hot paths, reduce API calls

## Reporting Issues

- Use the appropriate issue template (bug report, feature request, trade result)
- Include your environment details (OS, Rust version, config)
- For bugs, include steps to reproduce and expected vs actual behavior

## Code of Conduct

This project follows the [Contributor Covenant Code of Conduct](CODE_OF_CONDUCT.md). By participating, you agree to uphold this code.

## Questions?

Open a discussion or issue on GitHub. We're happy to help!
