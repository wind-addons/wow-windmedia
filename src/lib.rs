//! # wow-sharedmedia
//!
//! A Rust library for managing [World of Warcraft][wow] addon SharedMedia assets
//! — fonts, textures, sounds, borders, and statusbars — powered by
//! [LibSharedMedia-3.0][lsm].
//!
//! ## Overview
//!
//! `wow-sharedmedia` provides stateless, one-shot operations for:
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
//! wow-sharedmedia = "0.1"
//! ```
//!
//! ## Quick Start
//!
//! ```no_run
//! use wow_sharedmedia::{ensure_addon_dir, import_media, read_data, ImportOptions, MediaType};
//! use std::path::Path;
//!
//! fn main() -> Result<(), wow_sharedmedia::Error> {
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
//! After `ensure_addon_dir`, the directory layout is driven by the folder
//! name. For a folder named `MyAddon`:
//!
//! ```text
//! MyAddon/
//! ├── MyAddon.toc       # WoW addon manifest (auto-generated)
//! ├── data.lua           # Media registry (Lua table, single source of truth)
//! ├── loader.lua         # LSM registration script (auto-generated)
//! ├── libraries/         # Vendored LibSharedMedia-3.0 dependencies
//! └── media/
//!     ├── statusbar/    # TGA texture files
//!     ├── background/   # TGA texture files
//!     ├── border/       # TGA texture files
//!     ├── font/         # TTF/OTF font files
//!     └── sound/        # OGG audio files
//! ```
//!
//! If the folder name starts with `!` (e.g. `!!!WindMedia`), the `.toc`
//! file will be `!!!WindMedia.toc` but the in-addon title will strip the
//! leading `!` characters (e.g. `WindMedia`).
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

use std::path::Path;

pub use data::*;
pub use entry::*;
pub use error::*;
pub use media::*;

/// Extract the addon name from its directory path.
///
/// Returns the final component of `addon_dir` as a string slice.
///
/// # Panics
/// Panics if the directory name cannot be determined or is not valid UTF-8.
pub fn addon_name(addon_dir: &Path) -> &str {
	addon_dir
		.file_name()
		.expect("addon_dir must have a file name")
		.to_str()
		.expect("addon_dir name must be valid UTF-8")
}

/// Derive the human-readable addon title from the addon name.
///
/// Strips leading `!` characters. For example, `!!!WindMedia` → `WindMedia`.
pub fn addon_title(addon_name: &str) -> &str {
	addon_name.trim_start_matches('!')
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::path::PathBuf;

	#[test]
	fn test_addon_name_plain() {
		let path = PathBuf::from("/wow/AddOns/WindMedia");
		assert_eq!(addon_name(&path), "WindMedia");
	}

	#[test]
	fn test_addon_name_with_bangs() {
		let path = PathBuf::from("/wow/AddOns/!!!WindMedia");
		assert_eq!(addon_name(&path), "!!!WindMedia");
	}

	#[test]
	fn test_addon_title_strips_bangs() {
		assert_eq!(addon_title("!!!WindMedia"), "WindMedia");
	}

	#[test]
	fn test_addon_title_single_bang() {
		assert_eq!(addon_title("!TestAddon"), "TestAddon");
	}

	#[test]
	fn test_addon_title_no_bang() {
		assert_eq!(addon_title("WindMedia"), "WindMedia");
	}

	#[test]
	fn test_addon_title_all_bangs() {
		assert_eq!(addon_title("!!!"), "");
	}
}
