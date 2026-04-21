# wow-sharedmedia

[![CI](https://github.com/fang2hou/wow-sharedmedia/actions/workflows/ci.yml/badge.svg)](https://github.com/fang2hou/wow-sharedmedia/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)
[![Rust 1.94+](https://img.shields.io/badge/rust-1.94.0+-blue.svg)](https://doc.rust-lang.org/edition-guide/rust-2024/)

A Rust library for building and maintaining [World of Warcraft SharedMedia](https://wowpedia.fandom.com/wiki/LibSharedMedia-3.0) addons.

It manages `data.lua`, generates `loader.lua` and `.toc`, converts supported media formats into WoW-compatible outputs, and keeps the addon directory in a consistent state through a small stateless API.

Third-party Lua dependencies are embedded from a pinned vendor snapshot. Normal builds and releases use the tracked snapshot in `vendor.lock.json`; maintainers refresh upstream dependencies explicitly with a separate workflow.

## 📦 Installation

```toml
[dependencies]
wow-sharedmedia = "0.1"
```

Requires Rust 1.94+ (edition 2024).

## 🚀 Quick Start

```rust
use std::path::Path;

use wow_sharedmedia::{
    ensure_addon_dir, import_media, read_data, ImportOptions, MediaType, DEFAULT_MAX_BACKUPS,
};

fn main() -> Result<(), wow_sharedmedia::Error> {
    // The addon name is derived from the folder path.
    // "!!!MyMedia" sorts to top in the addon list;
    // "MyMedia" works too.
    let addon_dir = Path::new("AddOns/MyMedia");
    ensure_addon_dir(addon_dir, DEFAULT_MAX_BACKUPS)?;

    let source = Path::new("assets/my-statusbar.png");
    let result = import_media(
        addon_dir,
        ImportOptions::new(MediaType::Statusbar, "My Statusbar", source),
        DEFAULT_MAX_BACKUPS,
    )?;

    println!("Imported {} as {}", result.entry.key, result.entry.file);

    let data = read_data(addon_dir)?;
    println!("{} entries registered", data.entries.len());

    Ok(())
}
```

## 🧩 Supported Media Types

| Media type   | Accepted input                                   | Stored output      |
| ------------ | ------------------------------------------------ | ------------------ |
| `statusbar`  | `.tga`, `.png`, `.webp`, `.jpg`, `.jpeg`, `.blp` | `.tga`             |
| `background` | `.tga`, `.png`, `.webp`, `.jpg`, `.jpeg`, `.blp` | `.tga`             |
| `border`     | `.tga`, `.png`, `.webp`, `.jpg`, `.jpeg`, `.blp` | `.tga`             |
| `font`       | `.ttf`, `.otf`                                   | original font file |
| `sound`      | `.ogg`, `.mp3`, `.wav`                           | `.ogg`             |

## 🧭 Design

The crate treats `data.lua` as the single source of truth.

Every write operation follows the same model:

1. Ensure the addon directory and static templates exist
2. Read the current registry state from `data.lua`
3. Apply the requested mutation
4. Write the updated registry back to disk

This keeps the runtime model small, deterministic, and easy to integrate into higher-level tools.

### 🏷️ Addon Name Resolution

The addon name is derived from the folder path — no hardcoding required.

| Folder name    | TOC file           | TOC title      |
| -------------- | ------------------ | -------------- |
| `MyMedia`      | `MyMedia.toc`      | `MyMedia`      |
| `!!!MyMedia`   | `!!!MyMedia.toc`   | `MyMedia`      |
| `CoolTextures` | `CoolTextures.toc` | `CoolTextures` |

Leading `!` characters are stripped from the title automatically.

### 🗂️ Addon Layout

```text
MyMedia/                        # or !!!MyMedia — both work
├── MyMedia.toc                 # or !!!MyMedia.toc
├── data.lua
├── loader.lua
├── libraries/
│   ├── LibStub/LibStub.lua
│   ├── CallbackHandler-1.0/CallbackHandler-1.0.lua
│   └── LibSharedMedia-3.0/
│       ├── LibSharedMedia-3.0.lua
│       └── lib.xml
└── media/
    ├── background/
    ├── border/
    ├── font/
    ├── sound/
    └── statusbar/
```

## 📚 See Also

- [Contributing](./CONTRIBUTING.md) — development setup, commit conventions, and PR expectations
- [Release Process](./PUBLISHING.md) — how releases are automated and published to crates.io

## 📄 License

[MIT](./LICENSE).
