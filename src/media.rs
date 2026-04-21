//! Stateless media operations — import, remove, update.
//!
//! Each operation is atomic: read data.lua → modify → write data.lua.
//! No in-memory state, no dirty tracking, no separate save/generate/deploy steps.

use std::path::{Path, PathBuf};

use sha2::Digest;

use crate::converter;
use crate::lua_io;
use crate::template;
use crate::{AddonData, EntryMetadata, Error, MediaEntry, MediaType};

const MEDIA_SUBDIRS: &[&str] = &["statusbar", "background", "border", "font", "sound"];

/// Check if a char is a CJK character (Chinese, Japanese, Korean).
#[inline]
fn is_cjk_or_hangul(ch: char) -> bool {
	matches!(ch,
		'\u{4e00}'..='\u{9fff}' |     // CJK Unified Ideographs
		'\u{3400}'..='\u{4dbf}' |     // CJK Extension A
		'\u{f900}'..='\u{faff}' |     // CJK Compatibility Ideographs
		'\u{ac00}'..='\u{d7af}' |     // Hangul Syllables
		'\u{3040}'..='\u{309f}' |     // Hiragana
		'\u{30a0}'..='\u{30ff}' |     // Katakana
		'\u{31f0}'..='\u{31ff}'       // Katakana Phonetic Extensions
	)
}

fn sanitize_filename(name: &str) -> String {
	let mut result = String::with_capacity(name.len());
	let mut last_was_underscore = false;

	for ch in name.chars() {
		if ch.is_ascii_lowercase() || ch.is_ascii_digit() || is_cjk_or_hangul(ch) || ch == '.' || ch == '-' {
			result.push(ch);
			last_was_underscore = false;
		} else if ch.is_ascii_uppercase() {
			result.push(ch.to_ascii_lowercase());
			last_was_underscore = false;
		} else if !last_was_underscore {
			result.push('_');
			last_was_underscore = true;
		}
	}

	while result.ends_with('_') {
		result.pop();
	}
	while result.starts_with('_') {
		result.remove(0);
	}

	if result.is_empty() {
		"unnamed".to_string()
	} else {
		result
	}
}

const MAX_IMAGE_SIZE: u64 = 50 * 1024 * 1024;
const MAX_FONT_SIZE: u64 = 200 * 1024 * 1024;
const MAX_AUDIO_SIZE: u64 = 50 * 1024 * 1024;

fn current_version() -> &'static str {
	env!("CARGO_PKG_VERSION")
}

fn refresh_generated_metadata(data: &mut AddonData) {
	data.version = current_version().to_string();
	data.generated_at = chrono::Utc::now();
}

/// Non-fatal warning from import.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ImportWarning {
	/// Stable machine-readable warning code.
	pub code: String,
	/// Human-readable warning message.
	pub message: String,
}

/// Result of an import operation.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ImportResult {
	/// The newly created media entry.
	pub entry: MediaEntry,
	/// Non-fatal warnings emitted during import.
	pub warnings: Vec<ImportWarning>,
}

/// Options for importing a media file.
#[derive(Debug, Clone)]
pub struct ImportOptions {
	/// Target LibSharedMedia type for the imported file.
	pub media_type: MediaType,
	/// Display key used during registration.
	pub key: String,
	/// Source file path on the local filesystem.
	pub source: PathBuf,
	/// Optional locale names for font assets.
	pub locales: Vec<String>,
	/// Optional user-defined tags.
	pub tags: Vec<String>,
	/// When `true`, importing a duplicate key fails instead of coexisting.
	pub reject_duplicates: bool,
}

impl ImportOptions {
	/// Create a new import configuration with sane defaults.
	///
	/// Defaults:
	/// - `locales = []`
	/// - `tags = []`
	/// - `reject_duplicates = true`
	pub fn new(media_type: MediaType, key: impl Into<String>, source: impl Into<PathBuf>) -> Self {
		Self {
			media_type,
			key: key.into(),
			source: source.into(),
			locales: Vec::new(),
			tags: Vec::new(),
			reject_duplicates: true,
		}
	}
}

/// Options for updating an existing entry.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UpdateOptions {
	/// Optional replacement display key.
	pub key: Option<String>,
	/// Optional replacement locale set for font entries.
	pub locales: Option<Vec<String>>,
	/// Optional replacement tag set.
	pub tags: Option<Vec<String>>,
}

/// Result of a remove operation.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct RemovedEntry {
	/// The entry that was removed from the registry.
	pub entry: MediaEntry,
	/// Absolute path of the deleted media file.
	pub deleted_file: PathBuf,
}

/// Initialize a LibSharedMedia-compatible addon directory.
///
/// This function creates the media subdirectories, initializes `data.lua` when
/// missing, and re-deploys the static `loader.lua` and `.toc` templates.
///
/// Existing `data.lua` content is preserved.
pub fn ensure_addon_dir(addon_dir: &Path) -> Result<AddonData, Error> {
	// Create directory structure
	std::fs::create_dir_all(addon_dir).map_err(|e| Error::Io {
		source: e,
		path: addon_dir.to_path_buf(),
	})?;
	for sub in MEDIA_SUBDIRS {
		let dir = addon_dir.join("media").join(sub);
		std::fs::create_dir_all(&dir).map_err(|e| Error::Io { source: e, path: dir })?;
	}

	// Write data.lua if missing
	let data = if !addon_dir.join("data.lua").exists() {
		let data = AddonData::empty(current_version());
		lua_io::write_data(addon_dir, &data)?;
		data
	} else {
		lua_io::read_data(addon_dir)?
	};

	// Always deploy templates (they're static, overwrite is fine)
	template::deploy_templates(addon_dir)?;

	Ok(data)
}

/// Read the current addon registry from `data.lua`.
pub fn read_data(addon_dir: &Path) -> Result<AddonData, Error> {
	lua_io::read_data(addon_dir)
}

/// Import a media file into the addon registry.
///
/// This is a one-shot atomic operation: read `data.lua`, convert or copy the
/// asset referenced by [`ImportOptions::source`], append the new entry, and
/// write `data.lua` back to disk.
///
/// The addon directory and static templates are automatically created or
/// refreshed before the import proceeds.
pub fn import_media(addon_dir: &Path, opts: ImportOptions) -> Result<ImportResult, Error> {
	let mut data = ensure_addon_dir(addon_dir)?;
	let source = &opts.source;

	// Validate file size
	let file_size = std::fs::metadata(source)
		.map_err(|e| Error::Io {
			source: e,
			path: source.to_path_buf(),
		})?
		.len();
	let max_size = match opts.media_type {
		MediaType::Statusbar | MediaType::Background | MediaType::Border => MAX_IMAGE_SIZE,
		MediaType::Font => MAX_FONT_SIZE,
		MediaType::Sound => MAX_AUDIO_SIZE,
	};
	if file_size > max_size {
		return Err(Error::FileTooLarge {
			path: source.to_path_buf(),
			actual: file_size,
			max: max_size,
		});
	}

	// Enforce key uniqueness when duplicate rejection is enabled.
	if opts.reject_duplicates
		&& let Some(existing) = find_by_key(&data, opts.media_type, &opts.key)
	{
		return Err(Error::DuplicateKey {
			r#type: opts.media_type,
			key: opts.key,
			existing_id: existing.id,
		});
	}

	// Validate that the input extension is accepted for the target media type.
	let ext = source
		.extension()
		.and_then(|e| e.to_str())
		.map(|e| format!(".{e}"))
		.unwrap_or_default()
		.to_lowercase();
	if !opts.media_type.accepted_extensions().contains(&ext.as_str()) {
		return Err(Error::UnsupportedFormat {
			target_type: opts.media_type,
			extension: ext,
		});
	}

	// Build the normalized addon-relative output path.
	let file_stem = source.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown");
	let sanitized = sanitize_filename(file_stem);
	let output_ext = if ext == ".blp" {
		".blp"
	} else {
		opts.media_type.output_extension()
	};
	let rel_path = build_unique_relative_path(&data, addon_dir, opts.media_type, &sanitized, &ext, output_ext);
	let output_path = addon_dir.join(&rel_path);

	if let Some(parent) = output_path.parent() {
		std::fs::create_dir_all(parent).map_err(|e| Error::Io {
			source: e,
			path: parent.to_path_buf(),
		})?;
	}

	let mut warnings: Vec<ImportWarning> = Vec::new();
	let metadata: Option<EntryMetadata>;

	match opts.media_type {
		MediaType::Statusbar | MediaType::Background | MediaType::Border => {
			let result = if ext == ".blp" {
				std::fs::copy(source, &output_path).map_err(|e| Error::Io {
					source: e,
					path: source.to_path_buf(),
				})?;
				let dynamic = converter::blp::read_blp(source)?;
				converter::image::ImageConvertResult {
					width: dynamic.width(),
					height: dynamic.height(),
					original_width: dynamic.width(),
					original_height: dynamic.height(),
					was_resized: false,
				}
			} else {
				converter::image::convert_to_tga(source, &output_path)?
			};
			if result.was_resized {
				warnings.push(ImportWarning {
					code: "image_resized".into(),
					message: format!(
						"Resized from {}x{} to {}x{}",
						result.original_width, result.original_height, result.width, result.height
					),
				});
			}
			if ext == ".jpg" || ext == ".jpeg" {
				warnings.push(ImportWarning {
					code: "jpeg_no_alpha".into(),
					message: "JPEG does not support transparency. Consider using PNG.".into(),
				});
			}
			metadata = Some(EntryMetadata {
				image_width: Some(result.width),
				image_height: Some(result.height),
				..Default::default()
			});
		}
		MediaType::Font => {
			converter::font::validate_font(source)?;
			let font_meta = converter::font::extract_font_metadata(source)?;
			let locales = if opts.locales.is_empty() {
				converter::font::DEFAULT_LOCALES.iter().map(|s| s.to_string()).collect()
			} else {
				converter::font::validate_locale_names(&opts.locales.iter().map(|s| s.as_str()).collect::<Vec<_>>())?
			};
			std::fs::copy(source, &output_path).map_err(|e| Error::Io {
				source: e,
				path: source.to_path_buf(),
			})?;
			metadata = Some(EntryMetadata {
				font_family: Some(font_meta.family_name),
				font_style: Some(font_meta.style_name),
				font_is_monospace: Some(font_meta.is_monospace),
				font_num_glyphs: Some(font_meta.num_glyphs),
				locales,
				..Default::default()
			});
		}
		MediaType::Sound => {
			if ext == ".ogg" {
				std::fs::copy(source, &output_path).map_err(|e| Error::Io {
					source: e,
					path: source.to_path_buf(),
				})?;
				let audio_meta = converter::audio::probe_audio(&output_path)?;
				metadata = Some(EntryMetadata {
					audio_duration_secs: Some(audio_meta.duration_secs),
					audio_sample_rate: Some(audio_meta.sample_rate),
					audio_channels: Some(audio_meta.channels),
					..Default::default()
				});
			} else {
				let audio_meta = converter::audio::convert_to_ogg(source, &output_path)?;
				metadata = Some(EntryMetadata {
					audio_duration_secs: Some(audio_meta.duration_secs),
					audio_sample_rate: Some(audio_meta.sample_rate),
					audio_channels: Some(audio_meta.channels),
					..Default::default()
				});
			}
		}
	}

	// Checksum
	let file_bytes = std::fs::read(&output_path).map_err(|e| Error::Io {
		source: e,
		path: output_path.clone(),
	})?;
	let digest = sha2::Sha256::digest(&file_bytes);
	let checksum = format!("sha256:{:x}", digest);

	let entry = MediaEntry {
		id: uuid::Uuid::new_v4(),
		media_type: opts.media_type,
		key: opts.key,
		file: rel_path,
		original_name: source.file_name().and_then(|n| n.to_str()).map(|s| s.to_string()),
		imported_at: chrono::Utc::now(),
		checksum: Some(checksum),
		metadata,
		tags: opts.tags,
	};

	data.entries.push(entry.clone());
	refresh_generated_metadata(&mut data);
	lua_io::write_data(addon_dir, &data)?;

	Ok(ImportResult { entry, warnings })
}

/// Remove a media entry: ensure addon exists → delete file → remove entry → write data.lua.
pub fn remove_media(addon_dir: &Path, id: &uuid::Uuid) -> Result<RemovedEntry, Error> {
	let mut data = ensure_addon_dir(addon_dir)?;

	let idx = data
		.entries
		.iter()
		.position(|e| &e.id == id)
		.ok_or(Error::EntryNotFound(*id))?;

	let entry = data.entries.remove(idx);
	let file_path = addon_dir.join(&entry.file);
	let deleted_file = file_path.clone();

	if file_path.exists() {
		std::fs::remove_file(&file_path).map_err(|e| Error::Io {
			source: e,
			path: file_path,
		})?;
	}

	refresh_generated_metadata(&mut data);
	lua_io::write_data(addon_dir, &data)?;

	Ok(RemovedEntry { entry, deleted_file })
}

/// Update entry metadata: ensure addon exists → modify in memory → write data.lua.
pub fn update_media(addon_dir: &Path, id: &uuid::Uuid, opts: UpdateOptions) -> Result<MediaEntry, Error> {
	let mut data = ensure_addon_dir(addon_dir)?;

	let idx = data
		.entries
		.iter()
		.position(|e| &e.id == id)
		.ok_or(Error::EntryNotFound(*id))?;

	// Reject collisions when renaming to an existing key of the same media type.
	if let Some(ref new_key) = opts.key
		&& new_key != &data.entries[idx].key
		&& let Some(dup) = find_by_key(&data, data.entries[idx].media_type, new_key)
	{
		return Err(Error::DuplicateKey {
			r#type: data.entries[idx].media_type,
			key: new_key.clone(),
			existing_id: dup.id,
		});
	}

	let entry = &mut data.entries[idx];
	if let Some(ref new_key) = opts.key {
		entry.key = new_key.clone();
	}
	if let Some(ref locales) = opts.locales {
		if entry.media_type != MediaType::Font {
			return Err(Error::InvalidLocale(
				"Locale masks are only supported for font entries".to_string(),
			));
		}

		let validated_locales = if locales.is_empty() {
			Vec::new()
		} else {
			crate::converter::font::validate_locale_names(&locales.iter().map(|s| s.as_str()).collect::<Vec<_>>())?
		};

		if let Some(ref mut meta) = entry.metadata {
			meta.locales = validated_locales;
		} else if !validated_locales.is_empty() {
			entry.metadata = Some(EntryMetadata {
				locales: validated_locales,
				..Default::default()
			});
		}
	}
	if let Some(ref tags) = opts.tags {
		entry.tags = tags.clone();
	}

	refresh_generated_metadata(&mut data);
	lua_io::write_data(addon_dir, &data)?;

	Ok(data.entries[idx].clone())
}

fn find_by_key<'a>(data: &'a AddonData, media_type: MediaType, key: &str) -> Option<&'a MediaEntry> {
	data.entries.iter().find(|e| e.media_type == media_type && e.key == key)
}

fn build_unique_relative_path(
	data: &AddonData,
	addon_dir: &Path,
	media_type: MediaType,
	base_name: &str,
	input_ext: &str,
	output_ext: &str,
) -> String {
	let folder = media_type.folder_name();
	let fallback_ext = input_ext.trim_start_matches('.');

	for index in 0.. {
		let stem = if index == 0 {
			base_name.to_string()
		} else {
			format!("{base_name}-{index}")
		};

		let file_name = if output_ext.is_empty() {
			format!("{stem}.{fallback_ext}")
		} else {
			format!("{stem}{output_ext}")
		};

		let rel_path = format!("media/{folder}/{file_name}");
		let path_used_by_entry = data.entries.iter().any(|entry| entry.file == rel_path);
		let path_exists_on_disk = addon_dir.join(&rel_path).exists();

		if !path_used_by_entry && !path_exists_on_disk {
			return rel_path;
		}
	}

	unreachable!("unique media output path generation should always terminate")
}

#[cfg(test)]
mod tests {
	use super::*;
	use tempfile::TempDir;

	/// Create a minimal 1x1 RGBA PNG using the `image` crate (guaranteed valid).
	fn create_test_png(path: &std::path::Path) {
		let img =
			image::DynamicImage::ImageRgba8(image::ImageBuffer::from_pixel(1, 1, image::Rgba([255, 255, 255, 255])));
		let mut buf = std::io::Cursor::new(Vec::new());
		img.write_to(&mut buf, image::ImageFormat::Png).unwrap();
		std::fs::write(path, buf.into_inner()).unwrap();
	}

	fn import_statusbar(addon_dir: &std::path::Path, source: &std::path::Path, key: &str) -> ImportResult {
		import_media(addon_dir, ImportOptions::new(MediaType::Statusbar, key, source)).unwrap()
	}

	fn normalize_data_lua_snapshot(content: &str) -> String {
		let mut lines = Vec::new();
		for line in content.replace("\r\n", "\n").lines() {
			let trimmed = line.trim_start();
			let indent = &line[..line.len() - trimmed.len()];
			let normalized = if trimmed.starts_with("-- Generated: ") {
				format!("{indent}Generated: <GENERATED_AT>")
			} else if trimmed.starts_with("generated_at = ") || trimmed.starts_with("imported_at = ") {
				let ts_normalized = strip_timestamp_value(trimmed);
				format!("{indent}{ts_normalized}")
			} else if trimmed.starts_with("id = ") {
				format!("{indent}id = \"<UUID>\"")
			} else if trimmed.starts_with("checksum = ") {
				format!("{indent}checksum = \"<CHECKSUM>\"")
			} else {
				line.to_string()
			};
			lines.push(normalized);
		}
		lines.join("\n")
	}

	fn strip_timestamp_value(s: &str) -> String {
		let mut result = s.to_string();
		while let Some(pos) = result.find("\"20") {
			let rest = &result[pos + 1..];
			if let Some(end) = rest.find('"') {
				result = format!("{}<TS>\"{}", &result[..pos + 1], &rest[end + 1..]);
			} else {
				break;
			}
		}
		result
	}

	fn read_data_lua_snapshot(addon_dir: &std::path::Path) -> String {
		let content = std::fs::read_to_string(addon_dir.join("data.lua")).unwrap();
		normalize_data_lua_snapshot(&content)
	}

	#[test]
	fn test_ensure_creates_fresh_addon() {
		let dir = TempDir::new().unwrap();
		let addon_dir = dir.path().join("TestAddon");

		let data = ensure_addon_dir(&addon_dir).unwrap();

		assert!(addon_dir.join("data.lua").exists());
		assert!(addon_dir.join("loader.lua").exists());
		assert!(addon_dir.join("TestAddon.toc").exists());
		assert!(addon_dir.join("media").join("statusbar").is_dir());
		assert!(addon_dir.join("media").join("background").is_dir());
		assert!(addon_dir.join("media").join("border").is_dir());
		assert!(addon_dir.join("media").join("font").is_dir());
		assert!(addon_dir.join("media").join("sound").is_dir());
		assert_eq!(data.schema_version, crate::SCHEMA_VERSION);
		assert!(data.entries.is_empty());
	}

	#[test]
	fn test_ensure_is_idempotent() {
		let dir = TempDir::new().unwrap();
		let addon_dir = dir.path().join("TestAddon");

		let data1 = ensure_addon_dir(&addon_dir).unwrap();
		let data2 = ensure_addon_dir(&addon_dir).unwrap();

		// Second call reads existing data.lua — version and schema should match
		assert_eq!(data1.version, data2.version);
		assert_eq!(data1.schema_version, data2.schema_version);
		// generated_at may differ in nanosecond precision (write truncates to seconds)
		// but the data should be semantically the same
		assert_eq!(data1.entries.len(), data2.entries.len());
	}

	#[test]
	fn test_ensure_preserves_existing_data() {
		let dir = TempDir::new().unwrap();
		let addon_dir = dir.path().join("TestAddon");

		// Create addon with initial data
		let data1 = ensure_addon_dir(&addon_dir).unwrap();
		assert_eq!(data1.entries.len(), 0);

		// Manually inject an entry via write_data to simulate pre-existing data
		let mut modified = data1.clone();
		modified.entries.push(MediaEntry {
			id: uuid::Uuid::new_v4(),
			media_type: MediaType::Statusbar,
			key: "Pre-existing".into(),
			file: "media/statusbar/pre.tga".into(),
			original_name: None,
			imported_at: chrono::Utc::now(),
			checksum: None,
			metadata: None,
			tags: vec![],
		});
		crate::lua_io::write_data(&addon_dir, &modified).unwrap();

		// ensure_addon_dir should read back the existing data
		let data2 = ensure_addon_dir(&addon_dir).unwrap();
		assert_eq!(data2.entries.len(), 1);
		assert_eq!(data2.entries[0].key, "Pre-existing");
		assert_eq!(data2.version, env!("CARGO_PKG_VERSION"));
	}

	#[test]
	fn test_import_image_creates_entry_and_file() {
		let dir = TempDir::new().unwrap();
		let addon_dir = dir.path().join("TestAddon");
		ensure_addon_dir(&addon_dir).unwrap();

		// Create a valid PNG source
		let source = dir.path().join("test.png");
		create_test_png(&source);

		let opts = ImportOptions::new(MediaType::Statusbar, "Test Bar", &source);
		let result = import_media(&addon_dir, opts).unwrap();

		assert_eq!(result.entry.key, "Test Bar");
		assert_eq!(result.entry.media_type, MediaType::Statusbar);
		assert!(result.entry.checksum.is_some());
		assert!(result.entry.metadata.is_some());
		assert!(result.entry.metadata.as_ref().unwrap().image_width.is_some());

		// File should exist on disk
		assert!(addon_dir.join(&result.entry.file).exists());

		// data.lua should contain the entry
		let data = read_data(&addon_dir).unwrap();
		assert_eq!(data.entries.len(), 1);
		assert_eq!(data.entries[0].key, "Test Bar");
		assert_eq!(data.version, env!("CARGO_PKG_VERSION"));
	}

	#[test]
	fn test_import_rejects_duplicate_key() {
		let dir = TempDir::new().unwrap();
		let addon_dir = dir.path().join("TestAddon");
		ensure_addon_dir(&addon_dir).unwrap();

		let source = dir.path().join("test.png");
		create_test_png(&source);

		let opts = ImportOptions::new(MediaType::Statusbar, "Dupe", &source);
		import_media(&addon_dir, opts).unwrap();

		// Second import with same key should fail
		let opts2 = ImportOptions::new(MediaType::Statusbar, "Dupe", &source);
		let result = import_media(&addon_dir, opts2);
		assert!(result.is_err());
		match result.unwrap_err() {
			Error::DuplicateKey { r#type, key, .. } => {
				assert_eq!(r#type, MediaType::Statusbar);
				assert_eq!(key, "Dupe");
			}
			other => panic!("Expected DuplicateKey, got: {other}"),
		}
	}

	#[test]
	fn test_import_rejects_invalid_extension() {
		let dir = TempDir::new().unwrap();
		let addon_dir = dir.path().join("TestAddon");
		ensure_addon_dir(&addon_dir).unwrap();

		let source = dir.path().join("test.xyz");
		std::fs::write(&source, b"not an image").unwrap();

		let opts = ImportOptions::new(MediaType::Statusbar, "Bad", &source);
		let result = import_media(&addon_dir, opts);
		assert!(result.is_err());
		match result.unwrap_err() {
			Error::UnsupportedFormat { extension, .. } => {
				assert_eq!(extension, ".xyz");
			}
			other => panic!("Expected UnsupportedFormat, got: {other}"),
		}
	}

	#[test]
	fn test_import_missing_source_file() {
		let dir = TempDir::new().unwrap();
		let addon_dir = dir.path().join("TestAddon");
		ensure_addon_dir(&addon_dir).unwrap();

		let source = dir.path().join("nonexistent.png");
		let opts = ImportOptions::new(MediaType::Statusbar, "Missing", &source);
		let result = import_media(&addon_dir, opts);
		assert!(result.is_err());
	}

	#[test]
	fn test_import_auto_bootstraps_missing_addon_dir() {
		let dir = TempDir::new().unwrap();
		let addon_dir = dir.path().join("TestAddon");

		let source = dir.path().join("bootstrap.png");
		create_test_png(&source);

		let result = import_media(
			&addon_dir,
			ImportOptions::new(MediaType::Statusbar, "Bootstrap", &source),
		)
		.unwrap();

		assert_eq!(result.entry.key, "Bootstrap");
		assert!(addon_dir.join("data.lua").exists());
		assert!(addon_dir.join("loader.lua").exists());
		assert!(addon_dir.join("TestAddon.toc").exists());
		assert!(addon_dir.join(&result.entry.file).exists());
	}

	#[test]
	fn test_import_overwrite_allows_duplicate() {
		let dir = TempDir::new().unwrap();
		let addon_dir = dir.path().join("TestAddon");
		ensure_addon_dir(&addon_dir).unwrap();

		let source = dir.path().join("test.png");
		create_test_png(&source);

		let mut opts = ImportOptions::new(MediaType::Statusbar, "Same", &source);
		opts.reject_duplicates = true;
		import_media(&addon_dir, opts).unwrap();

		// With reject_duplicates = false, should succeed
		let mut opts2 = ImportOptions::new(MediaType::Statusbar, "Same", &source);
		opts2.reject_duplicates = false;
		let result = import_media(&addon_dir, opts2);
		assert!(result.is_ok());
	}

	#[test]
	fn test_import_avoids_file_name_collisions() {
		let dir = TempDir::new().unwrap();
		let addon_dir = dir.path().join("TestAddon");
		ensure_addon_dir(&addon_dir).unwrap();

		let source_a = dir.path().join("same-name.png");
		let source_b_dir = dir.path().join("nested");
		std::fs::create_dir_all(&source_b_dir).unwrap();
		let source_b = source_b_dir.join("same-name.png");
		create_test_png(&source_a);
		create_test_png(&source_b);

		let a = import_statusbar(&addon_dir, &source_a, "Alpha");
		let b = import_statusbar(&addon_dir, &source_b, "Beta");

		assert_ne!(a.entry.file, b.entry.file);
		assert!(addon_dir.join(&a.entry.file).exists());
		assert!(addon_dir.join(&b.entry.file).exists());

		let data = read_data(&addon_dir).unwrap();
		assert_eq!(data.entries.len(), 2);
	}

	#[test]
	fn test_remove_deletes_entry_and_file() {
		let dir = TempDir::new().unwrap();
		let addon_dir = dir.path().join("TestAddon");
		ensure_addon_dir(&addon_dir).unwrap();

		let source = dir.path().join("test.png");
		create_test_png(&source);

		let opts = ImportOptions::new(MediaType::Statusbar, "ToRemove", &source);
		let entry_id = import_media(&addon_dir, opts).unwrap().entry.id;

		let file_path = addon_dir.join("media/statusbar/test.tga");
		assert!(file_path.exists());

		let removed = remove_media(&addon_dir, &entry_id).unwrap();
		assert_eq!(removed.entry.key, "ToRemove");
		assert!(!file_path.exists());

		// data.lua should be empty now
		let data = read_data(&addon_dir).unwrap();
		assert!(data.entries.is_empty());
	}

	#[test]
	fn test_remove_nonexistent_id() {
		let dir = TempDir::new().unwrap();
		let addon_dir = dir.path().join("TestAddon");
		ensure_addon_dir(&addon_dir).unwrap();

		let fake_id = uuid::Uuid::new_v4();
		let result = remove_media(&addon_dir, &fake_id);
		assert!(result.is_err());
		match result.unwrap_err() {
			Error::EntryNotFound(id) => assert_eq!(id, fake_id),
			other => panic!("Expected EntryNotFound, got: {other}"),
		}
	}

	#[test]
	fn test_remove_auto_bootstraps_missing_addon_dir() {
		let dir = TempDir::new().unwrap();
		let addon_dir = dir.path().join("TestAddon");
		let fake_id = uuid::Uuid::new_v4();

		let result = remove_media(&addon_dir, &fake_id);
		assert!(result.is_err());
		match result.unwrap_err() {
			Error::EntryNotFound(id) => assert_eq!(id, fake_id),
			other => panic!("Expected EntryNotFound, got: {other}"),
		}

		assert!(addon_dir.join("data.lua").exists());
		assert!(addon_dir.join("loader.lua").exists());
		assert!(addon_dir.join("TestAddon.toc").exists());
	}

	#[test]
	fn test_remove_succeeds_when_file_already_deleted() {
		let dir = TempDir::new().unwrap();
		let addon_dir = dir.path().join("TestAddon");
		ensure_addon_dir(&addon_dir).unwrap();

		let source = dir.path().join("test.png");
		create_test_png(&source);

		let opts = ImportOptions::new(MediaType::Statusbar, "Ghost", &source);
		let entry_id = import_media(&addon_dir, opts).unwrap().entry.id;

		// Manually delete the file before calling remove
		let file_path = addon_dir.join("media/statusbar/test.tga");
		assert!(file_path.exists());
		std::fs::remove_file(&file_path).unwrap();

		// Remove should still succeed (just can't delete the already-missing file)
		let removed = remove_media(&addon_dir, &entry_id).unwrap();
		assert_eq!(removed.entry.key, "Ghost");

		let data = read_data(&addon_dir).unwrap();
		assert!(data.entries.is_empty());
	}

	#[test]
	fn test_update_key() {
		let dir = TempDir::new().unwrap();
		let addon_dir = dir.path().join("TestAddon");
		ensure_addon_dir(&addon_dir).unwrap();

		let source = dir.path().join("test.png");
		create_test_png(&source);

		let opts = ImportOptions::new(MediaType::Statusbar, "OldKey", &source);
		let entry_id = import_media(&addon_dir, opts).unwrap().entry.id;

		let updated = update_media(
			&addon_dir,
			&entry_id,
			UpdateOptions {
				key: Some("NewKey".into()),
				locales: None,
				tags: None,
			},
		)
		.unwrap();

		assert_eq!(updated.key, "NewKey");

		// Persisted to data.lua
		let data = read_data(&addon_dir).unwrap();
		assert_eq!(data.entries[0].key, "NewKey");
	}

	#[test]
	fn test_update_tags() {
		let dir = TempDir::new().unwrap();
		let addon_dir = dir.path().join("TestAddon");
		ensure_addon_dir(&addon_dir).unwrap();

		let source = dir.path().join("test.png");
		create_test_png(&source);

		let opts = ImportOptions::new(MediaType::Statusbar, "TagMe", &source);
		let entry_id = import_media(&addon_dir, opts).unwrap().entry.id;

		let updated = update_media(
			&addon_dir,
			&entry_id,
			UpdateOptions {
				key: None,
				locales: None,
				tags: Some(vec!["a".into(), "b".into()]),
			},
		)
		.unwrap();

		assert_eq!(updated.tags, vec!["a", "b"]);
	}

	#[cfg(target_os = "windows")]
	#[test]
	fn test_update_font_locales_validates_names() {
		let dir = TempDir::new().unwrap();
		let addon_dir = dir.path().join("TestAddon");
		ensure_addon_dir(&addon_dir).unwrap();

		let source = dir.path().join("font.ttf");
		std::fs::copy(r"C:\Windows\Fonts\arial.ttf", &source).unwrap();

		let mut opts = ImportOptions::new(MediaType::Font, "Body Font", &source);
		opts.locales = vec!["western".into()];
		let entry_id = import_media(&addon_dir, opts).unwrap().entry.id;

		let result = update_media(
			&addon_dir,
			&entry_id,
			UpdateOptions {
				key: None,
				locales: Some(vec!["bad-locale".into()]),
				tags: None,
			},
		);
		assert!(result.is_err());
		match result.unwrap_err() {
			Error::InvalidLocale(msg) => assert!(msg.contains("Invalid locale names")),
			other => panic!("Expected InvalidLocale, got: {other}"),
		}
	}

	#[test]
	fn test_update_non_font_locales_rejected() {
		let dir = TempDir::new().unwrap();
		let addon_dir = dir.path().join("TestAddon");
		ensure_addon_dir(&addon_dir).unwrap();

		let source = dir.path().join("test.png");
		create_test_png(&source);
		let entry_id = import_statusbar(&addon_dir, &source, "Statusbar").entry.id;

		let result = update_media(
			&addon_dir,
			&entry_id,
			UpdateOptions {
				key: None,
				locales: Some(vec!["western".into()]),
				tags: None,
			},
		);
		assert!(result.is_err());
		match result.unwrap_err() {
			Error::InvalidLocale(msg) => assert!(msg.contains("only supported for font entries")),
			other => panic!("Expected InvalidLocale, got: {other}"),
		}
	}

	#[test]
	fn test_update_rejects_duplicate_key() {
		let dir = TempDir::new().unwrap();
		let addon_dir = dir.path().join("TestAddon");
		ensure_addon_dir(&addon_dir).unwrap();

		let source = dir.path().join("test.png");
		create_test_png(&source);

		// Import two different entries
		let opts1 = ImportOptions::new(MediaType::Statusbar, "Alpha", &source);
		let id1 = import_media(&addon_dir, opts1).unwrap().entry.id;

		let source2 = dir.path().join("test2.png");
		create_test_png(&source2);
		let opts2 = ImportOptions::new(MediaType::Statusbar, "Beta", &source2);
		import_media(&addon_dir, opts2).unwrap();

		// Try to rename Alpha → Beta (duplicate)
		let result = update_media(
			&addon_dir,
			&id1,
			UpdateOptions {
				key: Some("Beta".into()),
				locales: None,
				tags: None,
			},
		);

		assert!(result.is_err());
		match result.unwrap_err() {
			Error::DuplicateKey { key, .. } => assert_eq!(key, "Beta"),
			other => panic!("Expected DuplicateKey, got: {other}"),
		}
	}

	#[test]
	fn test_update_nonexistent_id() {
		let dir = TempDir::new().unwrap();
		let addon_dir = dir.path().join("TestAddon");
		ensure_addon_dir(&addon_dir).unwrap();

		let fake_id = uuid::Uuid::new_v4();
		let result = update_media(
			&addon_dir,
			&fake_id,
			UpdateOptions {
				key: Some("X".into()),
				locales: None,
				tags: None,
			},
		);
		assert!(result.is_err());
	}

	#[test]
	fn test_update_auto_bootstraps_missing_addon_dir() {
		let dir = TempDir::new().unwrap();
		let addon_dir = dir.path().join("TestAddon");
		let fake_id = uuid::Uuid::new_v4();

		let result = update_media(
			&addon_dir,
			&fake_id,
			UpdateOptions {
				key: Some("Bootstrap Update".into()),
				locales: None,
				tags: None,
			},
		);
		assert!(result.is_err());
		match result.unwrap_err() {
			Error::EntryNotFound(id) => assert_eq!(id, fake_id),
			other => panic!("Expected EntryNotFound, got: {other}"),
		}

		assert!(addon_dir.join("data.lua").exists());
		assert!(addon_dir.join("loader.lua").exists());
		assert!(addon_dir.join("TestAddon.toc").exists());
	}

	#[test]
	fn test_read_from_nonexistent_dir() {
		let dir = TempDir::new().unwrap();
		let result = read_data(dir.path());
		assert!(result.is_err());
	}

	#[test]
	fn test_full_lifecycle() {
		let dir = TempDir::new().unwrap();
		let addon_dir = dir.path().join("TestAddon");

		// 1. Init
		let data = ensure_addon_dir(&addon_dir).unwrap();
		assert_eq!(data.entries.len(), 0);

		// 2. Import
		let source = dir.path().join("a.png");
		create_test_png(&source);
		let id = import_media(&addon_dir, ImportOptions::new(MediaType::Statusbar, "A", &source))
			.unwrap()
			.entry
			.id;

		let source2 = dir.path().join("b.png");
		create_test_png(&source2);
		let id2 = import_media(&addon_dir, ImportOptions::new(MediaType::Statusbar, "B", &source2))
			.unwrap()
			.entry
			.id;

		// 3. Read back
		let data = read_data(&addon_dir).unwrap();
		assert_eq!(data.entries.len(), 2);

		// 4. Update
		let _ = update_media(
			&addon_dir,
			&id2,
			UpdateOptions {
				key: Some("B-Renamed".into()),
				locales: None,
				tags: Some(vec!["renamed".into()]),
			},
		)
		.unwrap();

		// 5. Remove one
		let _ = remove_media(&addon_dir, &id).unwrap();

		// 6. Verify final state
		let data = read_data(&addon_dir).unwrap();
		assert_eq!(data.entries.len(), 1);
		assert_eq!(data.entries[0].key, "B-Renamed");
		assert_eq!(data.entries[0].tags, vec!["renamed"]);
		assert_eq!(data.version, env!("CARGO_PKG_VERSION"));
	}

	#[test]
	fn test_data_lua_end_to_end_state_transition_snapshot() {
		let dir = TempDir::new().unwrap();
		let addon_dir = dir.path().join("TestAddon");

		ensure_addon_dir(&addon_dir).unwrap();
		let initial_snapshot = read_data_lua_snapshot(&addon_dir);
		assert!(initial_snapshot.contains(&format!("Tool: wow-sharedmedia v{}", env!("CARGO_PKG_VERSION"))));
		assert!(initial_snapshot.contains(&format!("version = \"{}\"", env!("CARGO_PKG_VERSION"))));
		assert!(!initial_snapshot.contains("Entries:"));
		assert!(!initial_snapshot.contains("--[[table:"));

		let source = dir.path().join("lifecycle.png");
		create_test_png(&source);

		let imported = import_media(
			&addon_dir,
			ImportOptions::new(MediaType::Statusbar, "Lifecycle", &source),
		)
		.unwrap();
		let after_import = read_data_lua_snapshot(&addon_dir);
		assert_ne!(initial_snapshot, after_import);
		assert!(!after_import.contains("Entries:"));
		assert!(after_import.contains("key = \"Lifecycle\""));
		assert!(after_import.contains("file = \"media/statusbar/lifecycle.tga\""));
		assert!(after_import.contains("image_height = 1"));
		assert!(after_import.contains("image_width = 1"));

		update_media(
			&addon_dir,
			&imported.entry.id,
			UpdateOptions {
				key: Some("Lifecycle Updated".into()),
				locales: None,
				tags: Some(vec!["golden".into(), "stateful".into()]),
			},
		)
		.unwrap();
		let after_update = read_data_lua_snapshot(&addon_dir);
		assert_ne!(after_import, after_update);
		assert!(after_update.contains("key = \"Lifecycle Updated\""));
		assert!(!after_update.contains("key = \"Lifecycle\""));
		assert!(after_update.contains("tags = {"));

		remove_media(&addon_dir, &imported.entry.id).unwrap();
		let after_remove = read_data_lua_snapshot(&addon_dir);
		assert_eq!(initial_snapshot, after_remove);
	}

	#[test]
	fn test_sanitize_chinese_preserved() {
		assert_eq!(sanitize_filename("中文材质.tga"), "中文材质.tga");
	}

	#[test]
	fn test_sanitize_special_chars_stripped() {
		assert_eq!(sanitize_filename("My Cool Texture!! 2.png"), "my_cool_texture_2.png");
	}

	#[test]
	fn test_sanitize_consecutive_underscores() {
		assert_eq!(sanitize_filename("hello___world"), "hello_world");
	}

	#[test]
	fn test_sanitize_empty_string() {
		assert_eq!(sanitize_filename(""), "unnamed");
		assert_eq!(sanitize_filename("!!!"), "unnamed");
	}

	#[test]
	fn test_sanitize_trimming() {
		assert_eq!(sanitize_filename("_hello_"), "hello");
	}

	#[test]
	fn test_sanitize_korean_preserved() {
		assert_eq!(sanitize_filename("한글폰트.ttf"), "한글폰트.ttf");
	}

	#[test]
	fn test_sanitize_japanese_preserved() {
		assert_eq!(sanitize_filename("フォント.otf"), "フォント.otf");
	}
}
