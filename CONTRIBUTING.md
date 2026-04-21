# Contributing to wow-sharedmedia

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

| Tool                   | Purpose                                              |
| ---------------------- | ---------------------------------------------------- |
| Rust 1.95+             | Build and test                                       |
| [Bun](https://bun.sh/) | Vendor snapshot script and JS toolchain              |
| SVN                    | Vendor snapshot materialization (LibSharedMedia-3.0) |

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

This materializes the pinned vendor snapshot declared in `vendor.lock.json` into `vendor/`. The directory is gitignored, but Rust embeds the files at build time via `include_str!`, so the snapshot must exist locally before building.

To refresh upstream dependencies intentionally, run:

```bash
bun run refresh-vendor
```

Refresh mode updates `vendor.lock.json` and regenerates `vendor/`. Treat that as a maintainer workflow and review the resulting changes before merging.

## ✅ Checks

Run these before opening a PR:

```bash
cargo fmt --all --check
cargo clippy -p wow-sharedmedia --all-targets -- -D warnings
cargo test -p wow-sharedmedia
cargo doc -p wow-sharedmedia --no-deps
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

PR titles must also follow [Conventional Commits](https://www.conventionalcommits.org/) because the PR check workflow validates the title directly.

Examples:

- `build: pin vendor snapshots and tighten release publishing`
- `fix: preserve addon version metadata in generated files`
- `docs: clarify the vendor refresh workflow`

If you rename a PR after opening it, rerun the PR check only after the latest workflow changes are on the branch. The workflow reads the current live PR title at runtime, so the rerun should validate the updated title instead of the original event payload.

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
