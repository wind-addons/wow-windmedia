# Contributing to wow-windmedia

Contributions are welcome. This guide covers what you need to get started.

## 🎯 Before You Start

- Read [README.md](./README.md) for project goals and supported scope
- Read [PUBLISHING.md](./PUBLISHING.md) if your change affects the release pipeline

## 🏗️ Design Principles

Keep changes aligned with these goals:

- **Stateless** — no hidden runtime state or background synchronization
- **`data.lua` is the source of truth** — avoid introducing parallel metadata stores
- **WoW-compatible outputs** — generated assets should work in real addons
- **Small, stable API** — avoid exposing internal helpers without a strong reason
- **Explicit failures** — prefer clear errors over silent fallbacks

## 📋 Prerequisites

| Tool                   | Purpose                              |
| ---------------------- | ------------------------------------ |
| Rust 1.94+             | Build and test                       |
| [Bun](https://bun.sh/) | Vendor script and JS toolchain       |
| SVN                    | Vendor download (LibSharedMedia-3.0) |

### 💻 Platform-specific setup

**Windows** — SVN is included with [TortoiseSVN](https://tortoisesvn.net/) or [VisualSVN](https://www.visualsvn.com/downloads/).

**macOS:**

```bash
brew install subversion
```

**Linux (Debian/Ubuntu):**

```bash
sudo apt-get install subversion
```

## ⚙️ Setup

```bash
bun install
bun run update-vendor
```

This downloads third-party WoW libraries into `vendor/`. The directory is gitignored — Rust embeds the files at build time via `include_str!`.

## ✅ Checks

Run these before opening a PR:

```bash
cargo fmt --all --check
cargo clippy -p wow-windmedia --all-targets -- -D warnings
cargo test -p wow-windmedia
cargo doc -p wow-windmedia --no-deps
stylua --check templates/*.lua
bun run lint
bun run format:check
```

## 🪝 Pre-commit Hooks

```bash
cargo install --locked cocogitto
prek install --hook-type pre-commit --hook-type commit-msg --hook-type pre-push
```

Hook and commit configuration lives in `prek.toml` and `cog.toml`.

## 💬 Commit Convention

The repository uses [Conventional Commits](https://www.conventionalcommits.org/).

Examples:

- `feat: add BLP import support`
- `fix: sync generated file version metadata`
- `docs: clarify addon name resolution`
- `test: add Lua 5.1 loader runtime coverage`
- `ci: pin GitHub Actions to Node.js 24`

Cocogitto uses these prefixes to determine version bumps and generate changelogs. See `cog.toml` for the full type configuration.

## 📬 Pull Requests

Keep PRs focused and reviewable. Good PRs:

- explain the problem being solved
- describe the chosen approach and tradeoffs
- include tests for behavior changes
- update docs when the public API or workflows change
- avoid unrelated cleanup in the same patch

## ⚠️ API Changes

Be conservative with public API additions:

- avoid adding public modules or functions unless necessary
- prefer additive changes over breaking changes
- avoid locking in awkward APIs that will be expensive to maintain long-term

## 📝 Documentation

- keep tone concise and professional
- usage examples should be realistic and minimal
- emoji in headings are welcome, used sparingly

## 🐛 Reporting Bugs

Include when possible:

- crate version and Rust version
- operating system
- media type and input format
- minimal reproduction

## 🤝 Conduct

By participating, you agree to follow the expectations in [CODE_OF_CONDUCT.md](./CODE_OF_CONDUCT.md).
