use std::path::PathBuf;

/// Core error type for the wow-sharedmedia library.
#[derive(Debug, thiserror::Error)]
pub enum Error {
	// === Addon Directory Errors ===
	/// The requested addon directory does not exist.
	#[error("Addon directory not found: {0}")]
	AddonNotFound(PathBuf),

	// === Data Errors ===
	/// Failed to parse the addon's `data.lua` file into structured data.
	#[error("Failed to parse data.lua: {0}")]
	DataLuaParse(String),

	// === Entry Errors ===
	/// No entry exists for the provided UUID.
	#[error("Entry not found: {0}")]
	EntryNotFound(uuid::Uuid),

	/// An entry with the same media type and key already exists.
	#[error("Duplicate key '{key}' for type '{type}' (existing ID: {existing_id})")]
	DuplicateKey {
		/// Asset type involved in the collision.
		r#type: crate::MediaType,
		/// Duplicate display key.
		key: String,
		/// UUID of the existing conflicting entry.
		existing_id: uuid::Uuid,
	},

	// === Import Errors ===
	/// The input file extension is not accepted for the target media type.
	#[error("Unsupported file format for type '{target_type}': {extension}")]
	UnsupportedFormat {
		/// Target asset type.
		target_type: crate::MediaType,
		/// Rejected file extension.
		extension: String,
	},

	/// The image file could not be parsed or validated.
	#[error("Invalid image file: {0}")]
	InvalidImage(String),

	/// The image has invalid zero or negative-equivalent dimensions.
	#[error("Image dimensions must be positive, got {width}x{height}")]
	InvalidDimensions {
		/// Reported width.
		width: u32,
		/// Reported height.
		height: u32,
	},

	/// The image exceeds the supported maximum dimension.
	#[error("Image too large (max {max}x{max}): {actual}x{actual}")]
	ImageTooLarge {
		/// Maximum allowed dimension.
		max: u32,
		/// Actual dimension encountered.
		actual: u32,
	},

	/// The font file could not be parsed or validated.
	#[error("Invalid font file: {0}")]
	InvalidFont(String),

	/// Locale configuration is invalid for the requested operation.
	#[error("Invalid locale configuration: {0}")]
	InvalidLocale(String),

	/// The audio file could not be parsed or validated.
	#[error("Invalid audio file: {0}")]
	InvalidAudio(String),

	/// The file exceeds the per-type size limit.
	#[error("File too large: {actual} bytes (max {max} bytes)")]
	FileTooLarge {
		/// File path checked.
		path: PathBuf,
		/// Actual file size in bytes.
		actual: u64,
		/// Maximum accepted file size in bytes.
		max: u64,
	},

	// === Conversion Errors ===
	/// Image transcoding or serialization failed.
	#[error("Failed to convert image: {0}")]
	ImageConversion(String),

	/// Audio transcoding or serialization failed.
	#[error("Failed to convert audio: {0}")]
	AudioConversion(String),

	// === I/O Errors ===
	/// Filesystem access failed.
	#[error("I/O error on {path}: {source}")]
	Io {
		/// Original I/O error.
		#[source]
		source: std::io::Error,
		/// Path involved in the failed operation.
		path: PathBuf,
	},

	// === Lua Errors ===
	/// Interaction with the embedded Lua runtime failed.
	#[error("Lua error: {0}")]
	Lua(#[from] mlua::Error),
}
