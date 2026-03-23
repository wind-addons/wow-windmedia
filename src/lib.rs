//! # wow-windmedia
//!
//! A Rust library for managing [World of Warcraft][wow] addon SharedMedia assets
//! — fonts, textures, sounds, borders, and statusbars — powered by
//! [LibSharedMedia-3.0][lsm].
//!
//! ## Overview
//!
//! `wow-windmedia` provides stateless, one-shot operations for:
//!
//! - **Importing** media files (PNG, TGA, WebP, JPEG, BLP, TTF, OTF, OGG, MP3, WAV)
//!   with automatic format conversion to WoW-compatible formats.
//! - **Removing** media entries and their associated files.
//! - **Updating** entry metadata (display key, tags, locale masks).
//! - **Reading** the addon's `data.lua` file into a typed Rust struct.
//!
//! Each operation is atomic: read `data.lua` → modify → write `data.lua`.
//! No in-memory state, no dirty tracking, no separate save/generate/deploy steps.
//!
//! ## Installation
//!
//! ```toml
//! [dependencies]
//! wow-windmedia = "0.1"
//! ```
//!
//! ## Quick Start
//!
//! ```no_run
//! use wow_windmedia::{ensure_addon_dir, import_media, read_data, ImportOptions, MediaType};
//! use std::path::Path;
//!
//! fn main() -> Result<(), wow_windmedia::Error> {
//!
//! // Initialize addon directory (creates data.lua, loader.lua, .toc, media/ subdirs)
//! let addon_dir = Path::new("AddOns/WindMedia");
//! ensure_addon_dir(addon_dir)?;
//!
//! // Import a statusbar texture
//! let source = Path::new("assets/my-statusbar.png");
//! let opts = ImportOptions::new(MediaType::Statusbar, "My Bar", &source);
//! let result = import_media(addon_dir, opts)?;
//! println!("Imported: {} (ID: {})", result.entry.key, result.entry.id);
//!
//! // Read all entries
//! let data = read_data(addon_dir)?;
//! for entry in &data.entries {
//!     println!("  {} [{}] → {}", entry.key, entry.media_type, entry.file);
//! }
//!
//! Ok(())
//! }
//! ```
//!
//! ## Addon Directory Structure
//!
//! After `ensure_addon_dir`, the directory layout is:
//!
//! ```text
//! AddOns/WindMedia/
//! ├── data.lua          # Media registry (Lua table, single source of truth)
//! ├── loader.lua        # LSM registration script (auto-generated)
//! ├── WindMedia.toc     # WoW addon manifest (auto-generated)
//! └── media/
//!     ├── statusbar/    # TGA texture files
//!     ├── background/   # TGA texture files
//!     ├── border/       # TGA texture files
//!     ├── font/         # TTF/OTF font files
//!     └── sound/        # OGG audio files
//! ```
//!
//! [wow]: https://worldofwarcraft.blizzard.com
//! [lsm]: https://www.wowace.com/projects/libsharedmedia-3-0/

#![warn(missing_docs)]
#![deny(unsafe_code)]

pub mod converter;
mod data;
mod entry;
mod error;
mod lua_io;
mod media;
pub mod template;

/// The default addon directory name used by WindMedia.
pub const ADDON_DIR_NAME: &str = "!!!WindMedia";

pub use data::*;
pub use entry::*;
pub use error::*;
pub use media::*;
