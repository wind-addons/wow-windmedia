# Contributing to wow-windmedia

Thanks for your interest in improving `wow-windmedia`.

This project aims to be a polished, predictable Rust library for managing World of Warcraft SharedMedia assets. Contributions are welcome, but changes should preserve the crate's small public surface, stateless design, and release quality.

## Before You Start

- Read `README.md` for project goals and supported scope
- Read `PUBLISHING.md` if your change affects release behavior

## Development Principles

Please keep changes aligned with these principles:

- **Stateless by design** — no hidden runtime state or background synchronization
- **`data.lua` is the source of truth** — avoid introducing parallel metadata stores
- **WoW-compatible outputs** — generated assets should remain practical for real addon usage
- **Small, stable API surface** — avoid exposing internal helpers without a strong reason
- **Clear failure modes** — prefer explicit errors over silent fallback behavior

## Prerequisites

| Tool      | Purpose                          |
| --------- | -------------------------------- |
| Rust 1.94 | Build and test                   |
| Bun       | Vendor script and JS toolchain   |
| SVN       | Vendor download (libsharedmedia) |

### Install Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### Install Bun

See [bun.sh](https://bun.sh/).

### Install SVN

**Windows** — included with TortoiseSVN or install separately via [VisualSVN](https://www.visualsvn.com/downloads/).

**macOS:**

```bash
brew install subversion
```

**Linux (Debian/Ubuntu):**

```bash
sudo apt-get install subversion
```

## Setup

```bash
bun install
bun run update-vendor
```

## Checks

```bash
cargo fmt --all --check
cargo clippy -p wow-windmedia --all-targets -- -D warnings
cargo test -p wow-windmedia
cargo doc -p wow-windmedia --no-deps
bun run lint
bun run format:check
stylua --check templates/*.lua
```

## Pre-commit Hooks

```bash
cargo install --locked cocogitto
prek install --hook-type pre-commit --hook-type commit-msg --hook-type pre-push
```

This crate keeps its hook and commit configuration in `prek.toml` and `cog.toml`.

## Commit Convention

The repository uses **Conventional Commits**.

Examples:

- `feat: add BLP import support`
- `fix: sync generated file version metadata`
- `docs: refine publishing guide`
- `test: add Lua 5.1 loader runtime coverage`
- `ci: add macOS CI job`

## Pull Request Expectations

Please keep pull requests focused and reviewable.

Good pull requests usually:

- explain the problem being solved
- describe the chosen approach and tradeoffs
- include tests for behavior changes
- update docs when the public API or workflows change
- avoid unrelated cleanup in the same patch

## Commit and Change Quality

Before opening a PR, make sure:

- the crate builds cleanly
- tests pass locally
- public-facing changes are documented
- new files and docs use professional English

## API Changes

For changes that affect the public API, please be extra conservative:

- avoid adding public modules or functions unless necessary
- avoid locking in awkward APIs that will be expensive to support after `0.1.0`
- prefer additive changes over breaking changes where practical

## Documentation Changes

Docs should be concise, professional, and easy to scan.

- README tone should stay polished and release-oriented
- Emoji are welcome, but should be used sparingly and deliberately
- Usage examples should be realistic and minimal

## Reporting Issues

If you are reporting a bug, please include:

- the crate version
- your Rust version
- your operating system
- the asset type and input format involved
- a minimal reproduction, if possible

## Conduct

By participating in this project, you agree to follow the expectations in `CODE_OF_CONDUCT.md`.
