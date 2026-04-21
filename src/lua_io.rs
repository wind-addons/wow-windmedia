//! Read/write data.lua via mlua.
//!
//! Lua handles its own escaping and table serialization — Rust never touches raw strings.

use std::path::Path;

use mlua::{Lua, Table, Value};

use crate::{AddonData, EntryMetadata, Error, MediaEntry, MediaType};

const SERPENT_LUA: &str = include_str!("../vendor/serpent/serpent.lua");

/// Read data.lua from an addon directory and return the parsed AddonData.
pub(crate) fn read_data(addon_dir: &Path) -> Result<AddonData, Error> {
	let path = addon_dir.join("data.lua");
	let content = std::fs::read_to_string(&path).map_err(|e| Error::Io {
		source: e,
		path: path.clone(),
	})?;

	let lua = Lua::new();
	let addon: Table = lua.create_table()?;
	let addon_name = crate::addon_name(addon_dir);

	// Wrap the script as a function to pass args via ...
	let wrapped = format!("return function(...)\n{}\nend", content);
	let func: mlua::Function = lua.load(&wrapped).eval()?;
	func.call::<()>((addon_name.to_string(), addon.clone()))?;

	// Extract addon.data
	let data_val: Value = addon.get("data")?;
	let data_tbl: &Table = match &data_val {
		Value::Table(t) => t,
		_ => return Err(Error::DataLuaParse("addon.data is not a table".into())),
	};

	lua_to_addon_data(data_tbl)
}

fn lua_to_addon_data(tbl: &Table) -> Result<AddonData, Error> {
	Ok(AddonData {
		schema_version: tbl.get("schema_version")?,
		version: tbl.get("version")?,
		generated_at: parse_datetime(&tbl.get::<String>("generated_at")?)?,
		entries: lua_to_entries(tbl.get("entries")?)?,
	})
}

fn lua_to_entries(val: Value) -> Result<Vec<MediaEntry>, Error> {
	let tbl: &Table = match &val {
		Value::Table(t) => t,
		Value::Nil => return Ok(Vec::new()),
		_ => return Err(Error::DataLuaParse("entries is not a table".into())),
	};

	let mut entries = Vec::new();
	for pair in tbl.sequence_values::<Table>() {
		entries.push(lua_to_entry(&pair?)?);
	}
	Ok(entries)
}

fn lua_to_entry(tbl: &Table) -> Result<MediaEntry, Error> {
	let type_str: String = tbl.get("type")?;
	let media_type: MediaType = type_str
		.parse()
		.map_err(|e: String| Error::DataLuaParse(format!("invalid type '{e}'")))?;

	Ok(MediaEntry {
		id: parse_uuid(&tbl.get::<String>("id")?)?,
		media_type,
		key: tbl.get("key")?,
		file: tbl.get("file")?,
		original_name: tbl.get("original_name").ok().flatten(),
		imported_at: parse_datetime(&tbl.get::<String>("imported_at")?)?,
		checksum: tbl.get("checksum").ok().flatten(),
		metadata: lua_to_metadata(tbl.get("metadata")?),
		tags: tbl.get("tags").unwrap_or_default(),
	})
}

fn lua_to_metadata(val: Value) -> Option<EntryMetadata> {
	let tbl: &Table = match &val {
		Value::Table(t) => t,
		_ => return None,
	};

	Some(EntryMetadata {
		image_width: tbl.get("image_width").ok().flatten(),
		image_height: tbl.get("image_height").ok().flatten(),
		font_family: tbl.get("font_family").ok().flatten(),
		font_style: tbl.get("font_style").ok().flatten(),
		font_is_monospace: tbl.get("font_is_monospace").ok().flatten(),
		font_num_glyphs: tbl.get("font_num_glyphs").ok().flatten(),
		locales: tbl.get("locales").unwrap_or_default(),
		audio_duration_secs: tbl.get("audio_duration_secs").ok().flatten(),
		audio_sample_rate: tbl.get("audio_sample_rate").ok().flatten(),
		audio_channels: tbl.get("audio_channels").ok().flatten(),
	})
}

fn parse_uuid(s: &str) -> Result<uuid::Uuid, Error> {
	uuid::Uuid::parse_str(s).map_err(|e| Error::DataLuaParse(format!("invalid UUID: {e}")))
}

fn parse_datetime(s: &str) -> Result<chrono::DateTime<chrono::Utc>, Error> {
	chrono::DateTime::parse_from_rfc3339(s)
		.map(|dt| dt.with_timezone(&chrono::Utc))
		.map_err(|e| Error::DataLuaParse(format!("invalid datetime: {e}")))
}

/// Write AddonData to data.lua (with BAK backup).
///
/// Before writing, creates a numbered backup: data.lua.1.bak, data.lua.2.bak, ...
pub(crate) fn write_data(addon_dir: &Path, data: &AddonData) -> Result<(), Error> {
	let data_path = addon_dir.join("data.lua");

	// Create numbered backup before overwriting
	if data_path.exists() {
		let bak_num = next_bak_number(addon_dir);
		let bak_path = addon_dir.join(format!("data.lua.{bak_num}.bak"));
		std::fs::copy(&data_path, &bak_path).map_err(|e| Error::Io {
			source: e,
			path: bak_path,
		})?;
	}

	// Generate the Lua content
	let body = serialize_addon_data(data)?;
	let content = format!(
		"-- Generated: {}\n-- Tool: wow-sharedmedia v{}\n\nlocal _, addon = ...\n\naddon.data = {}\n",
		data.generated_at.format("%Y-%m-%dT%H:%M:%SZ"),
		data.version,
		body,
	);

	// Atomic write: .tmp → rename
	let tmp_path = addon_dir.join("data.lua.tmp");
	std::fs::write(&tmp_path, &content).map_err(|e| Error::Io {
		source: e,
		path: tmp_path.clone(),
	})?;
	std::fs::rename(&tmp_path, &data_path).map_err(|e| Error::Io {
		source: e,
		path: data_path,
	})?;

	Ok(())
}

fn next_bak_number(addon_dir: &Path) -> u32 {
	let mut max: u32 = 0;
	for entry in std::fs::read_dir(addon_dir).into_iter().flatten() {
		let entry = match entry {
			Ok(e) => e,
			Err(_) => continue,
		};
		let os_name = entry.file_name();
		let name: &str = match os_name.to_str() {
			Some(s) => s,
			None => continue,
		};
		if let Some(rest) = name.strip_prefix("data.lua.")
			&& let Some(rest) = rest.strip_suffix(".bak")
			&& let Ok(n) = rest.parse::<u32>()
		{
			max = max.max(n);
		}
	}
	max + 1
}

fn serialize_addon_data(data: &AddonData) -> Result<String, Error> {
	let lua = Lua::new();

	let serpent: Table = lua.load(SERPENT_LUA).eval()?;
	let block_fn: mlua::Function = serpent.get("block")?;
	let tbl = addon_data_to_table(&lua, data)?;

	let opts = lua.create_table()?;
	opts.set("comment", false)?;

	let body: String = block_fn.call((tbl, opts))?;
	Ok(body)
}

fn addon_data_to_table(lua: &Lua, data: &AddonData) -> Result<Table, Error> {
	let tbl = lua.create_table()?;
	tbl.set("schema_version", data.schema_version)?;
	tbl.set("version", data.version.clone())?;
	tbl.set(
		"generated_at",
		data.generated_at.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
	)?;

	// Entries as array
	let entries_tbl = lua.create_table()?;
	for (i, entry) in data.entries.iter().enumerate() {
		entries_tbl.set(i + 1, entry_to_table(lua, entry)?)?;
	}
	tbl.set("entries", entries_tbl)?;

	Ok(tbl)
}

fn entry_to_table(lua: &Lua, entry: &MediaEntry) -> Result<Table, Error> {
	let tbl = lua.create_table()?;
	tbl.set("id", entry.id.to_string())?;
	tbl.set("type", entry.media_type.to_string())?;
	tbl.set("key", entry.key.as_str())?;
	tbl.set("file", entry.file.as_str())?;

	if let Some(ref name) = entry.original_name {
		tbl.set("original_name", name.clone())?;
	}
	tbl.set(
		"imported_at",
		entry.imported_at.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
	)?;
	if let Some(ref checksum) = entry.checksum {
		tbl.set("checksum", checksum.clone())?;
	}
	if let Some(ref meta) = entry.metadata {
		tbl.set("metadata", metadata_to_table(lua, meta)?)?;
	}
	if !entry.tags.is_empty() {
		let tags_tbl = lua.create_table()?;
		for (i, tag) in entry.tags.iter().enumerate() {
			tags_tbl.set(i + 1, tag.clone())?;
		}
		tbl.set("tags", tags_tbl)?;
	}

	Ok(tbl)
}

fn metadata_to_table(lua: &Lua, meta: &EntryMetadata) -> Result<Table, Error> {
	let tbl = lua.create_table()?;
	if let Some(w) = meta.image_width {
		tbl.set("image_width", w)?;
	}
	if let Some(h) = meta.image_height {
		tbl.set("image_height", h)?;
	}
	if let Some(ref family) = meta.font_family {
		tbl.set("font_family", family.clone())?;
	}
	if let Some(ref style) = meta.font_style {
		tbl.set("font_style", style.clone())?;
	}
	if let Some(mono) = meta.font_is_monospace {
		tbl.set("font_is_monospace", mono)?;
	}
	if let Some(glyphs) = meta.font_num_glyphs {
		tbl.set("font_num_glyphs", glyphs)?;
	}
	if !meta.locales.is_empty() {
		let loc_tbl = lua.create_table()?;
		for (i, loc) in meta.locales.iter().enumerate() {
			loc_tbl.set(i + 1, loc.clone())?;
		}
		tbl.set("locales", loc_tbl)?;
	}
	if let Some(dur) = meta.audio_duration_secs {
		tbl.set("audio_duration_secs", dur)?;
	}
	if let Some(rate) = meta.audio_sample_rate {
		tbl.set("audio_sample_rate", rate)?;
	}
	if let Some(ch) = meta.audio_channels {
		tbl.set("audio_channels", ch)?;
	}
	Ok(tbl)
}

#[cfg(test)]
mod tests {
	use super::*;
	use chrono::Utc;
	use tempfile::TempDir;

	fn sample_data() -> AddonData {
		let mut data = AddonData::empty("0.1.0");
		data.entries.push(MediaEntry {
			id: uuid::Uuid::new_v4(),
			media_type: MediaType::Statusbar,
			key: "Wind Clean".to_string(),
			file: "media/statusbar/wind_clean.tga".to_string(),
			original_name: None,
			imported_at: Utc::now(),
			checksum: Some("sha256:abc123".to_string()),
			metadata: None,
			tags: vec!["clean".to_string()],
		});
		data
	}

	#[test]
	fn test_write_and_read_roundtrip() {
		let dir = TempDir::new().unwrap();
		let data = sample_data();

		write_data(dir.path(), &data).unwrap();
		let read_back = read_data(dir.path()).unwrap();

		assert_eq!(read_back.schema_version, data.schema_version);
		assert_eq!(read_back.version, data.version);
		assert_eq!(read_back.entries.len(), 1);
		assert_eq!(read_back.entries[0].key, "Wind Clean");
		assert_eq!(read_back.entries[0].media_type, MediaType::Statusbar);
		assert_eq!(read_back.entries[0].tags, vec!["clean"]);
	}

	#[test]
	fn test_bak_numbering() {
		let dir = TempDir::new().unwrap();
		let data = sample_data();

		write_data(dir.path(), &data).unwrap(); // no BAK (first write)
		write_data(dir.path(), &data).unwrap(); // creates .1.bak
		write_data(dir.path(), &data).unwrap(); // creates .2.bak

		assert!(dir.path().join("data.lua.1.bak").exists());
		assert!(dir.path().join("data.lua.2.bak").exists());
		assert!(dir.path().join("data.lua").exists());
	}

	#[test]
	fn test_empty_entries_roundtrip() {
		let dir = TempDir::new().unwrap();
		let data = AddonData::empty("1.0.0");

		write_data(dir.path(), &data).unwrap();
		let read_back = read_data(dir.path()).unwrap();

		assert!(read_back.entries.is_empty());
		assert_eq!(read_back.schema_version, crate::SCHEMA_VERSION);
	}

	#[test]
	fn test_all_media_types_roundtrip() {
		let dir = TempDir::new().unwrap();
		let mut data = AddonData::empty("1.0.0");
		data.entries.push(MediaEntry {
			id: uuid::Uuid::new_v4(),
			media_type: MediaType::Statusbar,
			key: "Bar".into(),
			file: "media/statusbar/bar.tga".into(),
			original_name: None,
			imported_at: Utc::now(),
			checksum: None,
			metadata: None,
			tags: vec![],
		});
		data.entries.push(MediaEntry {
			id: uuid::Uuid::new_v4(),
			media_type: MediaType::Font,
			key: "Wind Sans".into(),
			file: "media/font/wind_sans.ttf".into(),
			original_name: None,
			imported_at: Utc::now(),
			checksum: None,
			metadata: Some(EntryMetadata {
				font_family: Some("Wind Sans".into()),
				locales: vec!["western".into(), "zhCN".into()],
				..Default::default()
			}),
			tags: vec![],
		});
		data.entries.push(MediaEntry {
			id: uuid::Uuid::new_v4(),
			media_type: MediaType::Sound,
			key: "Click".into(),
			file: "media/sound/click.ogg".into(),
			original_name: None,
			imported_at: Utc::now(),
			checksum: None,
			metadata: Some(EntryMetadata {
				audio_duration_secs: Some(0.5),
				audio_sample_rate: Some(44100),
				audio_channels: Some(2),
				..Default::default()
			}),
			tags: vec![],
		});

		write_data(dir.path(), &data).unwrap();
		let read_back = read_data(dir.path()).unwrap();

		assert_eq!(read_back.entries.len(), 3);
		assert_eq!(read_back.entries[1].media_type, MediaType::Font);
		assert_eq!(
			read_back.entries[1].metadata.as_ref().unwrap().locales,
			vec!["western", "zhCN"]
		);
		assert_eq!(
			read_back.entries[2].metadata.as_ref().unwrap().audio_duration_secs,
			Some(0.5)
		);
	}

	#[test]
	fn test_read_missing_file() {
		let dir = TempDir::new().unwrap();
		let result = read_data(dir.path());
		assert!(result.is_err());
	}

	#[test]
	fn test_special_chars_in_key() {
		let dir = TempDir::new().unwrap();
		let mut data = AddonData::empty("1.0.0");
		data.entries.push(MediaEntry {
			id: uuid::Uuid::new_v4(),
			media_type: MediaType::Statusbar,
			key: "Test \"Quote\"".to_string(),
			file: "media/statusbar/test.tga".to_string(),
			original_name: None,
			imported_at: Utc::now(),
			checksum: None,
			metadata: None,
			tags: vec![],
		});

		write_data(dir.path(), &data).unwrap();
		let read_back = read_data(dir.path()).unwrap();
		assert_eq!(read_back.entries[0].key, r#"Test "Quote""#);
	}

	#[test]
	fn test_chinese_keys_roundtrip() {
		let dir = TempDir::new().unwrap();
		let mut data = AddonData::empty("1.0.0");
		data.entries.push(MediaEntry {
			id: uuid::Uuid::new_v4(),
			media_type: MediaType::Statusbar,
			key: "清风明月".to_string(),
			file: "media/statusbar/qfmy.tga".to_string(),
			original_name: None,
			imported_at: Utc::now(),
			checksum: None,
			metadata: None,
			tags: vec![],
		});

		write_data(dir.path(), &data).unwrap();
		let read_back = read_data(dir.path()).unwrap();
		assert_eq!(read_back.entries[0].key, "清风明月");
	}

	#[test]
	fn test_generated_lua_is_valid() {
		let dir = TempDir::new().unwrap();
		let data = sample_data();
		write_data(dir.path(), &data).unwrap();

		let content = std::fs::read_to_string(dir.path().join("data.lua")).unwrap();

		assert!(content.contains("Generated: "));
		assert!(content.contains("Tool: wow-sharedmedia v0.1.0"));
		assert!(content.contains("local _, addon = ..."));
		assert!(content.contains("addon.data"));
		assert!(!content.contains("--[[table:"));

		assert!(read_data(dir.path()).is_ok());
	}

	#[test]
	fn test_read_corrupted_lua_syntax() {
		let dir = TempDir::new().unwrap();
		std::fs::write(dir.path().join("data.lua"), "this is not valid lua {{{").unwrap();

		let result = read_data(dir.path());
		assert!(result.is_err());
	}

	#[test]
	fn test_read_missing_addon_data_field() {
		let dir = TempDir::new().unwrap();
		// Valid Lua but no addon.data
		std::fs::write(dir.path().join("data.lua"), "local _, addon = ...\naddon.other = {}\n").unwrap();

		let result = read_data(dir.path());
		assert!(result.is_err());
		match result.unwrap_err() {
			Error::DataLuaParse(msg) => assert!(msg.contains("not a table")),
			other => panic!("Expected DataLuaParse, got: {other}"),
		}
	}

	#[test]
	fn test_read_corrupted_entries_field() {
		let dir = TempDir::new().unwrap();
		// Valid Lua, addon.data is a table, but entries is a string instead of table
		std::fs::write(
			dir.path().join("data.lua"),
			"local _, addon = ...\naddon.data = { entries = \"not a table\" }\n",
		)
		.unwrap();

		let result = read_data(dir.path());
		assert!(result.is_err());
	}

	#[test]
	fn test_bak_preserves_original_on_write() {
		let dir = TempDir::new().unwrap();
		let mut data1 = AddonData::empty("1.0.0");
		data1.entries.push(MediaEntry {
			id: uuid::Uuid::new_v4(),
			media_type: MediaType::Statusbar,
			key: "Original".into(),
			file: "media/statusbar/orig.tga".into(),
			original_name: None,
			imported_at: Utc::now(),
			checksum: None,
			metadata: None,
			tags: vec![],
		});

		write_data(dir.path(), &data1).unwrap();

		// Modify and write again
		let mut data2 = data1.clone();
		data2.entries[0].key = "Modified".into();
		write_data(dir.path(), &data2).unwrap();

		// BAK should contain the original
		let bak_content = std::fs::read_to_string(dir.path().join("data.lua.1.bak")).unwrap();
		assert!(bak_content.contains("Original"));
		assert!(!bak_content.contains("Modified"));

		// Current file should have the modification
		let current = std::fs::read_to_string(dir.path().join("data.lua")).unwrap();
		assert!(current.contains("Modified"));
		assert!(!current.contains("Original"));
	}

	#[test]
	fn test_bak_content_matches_original() {
		let dir = TempDir::new().unwrap();
		let data = sample_data();

		write_data(dir.path(), &data).unwrap();
		let original_content = std::fs::read_to_string(dir.path().join("data.lua")).unwrap();

		// Second write creates BAK
		write_data(dir.path(), &data).unwrap();
		let bak_content = std::fs::read_to_string(dir.path().join("data.lua.1.bak")).unwrap();
		assert_eq!(bak_content, original_content);
	}

	#[test]
	fn test_write_creates_atomic() {
		let dir = TempDir::new().unwrap();
		let data = sample_data();

		write_data(dir.path(), &data).unwrap();

		// tmp file should be cleaned up
		assert!(!dir.path().join("data.lua.tmp").exists());
		assert!(dir.path().join("data.lua").exists());
	}

	#[test]
	fn test_write_data_renders_supplied_version_in_header_and_body() {
		let dir = TempDir::new().unwrap();
		let mut data = AddonData::empty("9.9.9-test");
		data.entries.push(MediaEntry {
			id: uuid::Uuid::new_v4(),
			media_type: MediaType::Statusbar,
			key: "Version Probe".into(),
			file: "media/statusbar/version_probe.tga".into(),
			original_name: None,
			imported_at: Utc::now(),
			checksum: Some("sha256:test".into()),
			metadata: None,
			tags: vec![],
		});

		write_data(dir.path(), &data).unwrap();
		let content = std::fs::read_to_string(dir.path().join("data.lua")).unwrap();

		assert!(content.contains("Generated: "));
		assert!(content.contains("Tool: wow-sharedmedia v9.9.9-test"));
		assert!(!content.contains("Entries:"));
		assert!(!content.contains("DO NOT EDIT MANUALLY"));
		assert!(!content.contains("--[[table:"));
		assert!(content.contains("version = \"9.9.9-test\""));

		let read_back = read_data(dir.path()).unwrap();
		assert_eq!(read_back.version, "9.9.9-test");
		assert_eq!(read_back.entries[0].key, "Version Probe");
	}
}
