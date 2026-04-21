//! End-to-end integration tests.
//!
//! Every test uses `tempfile::TempDir` — the directory is automatically
//! cleaned up when the `TempDir` value is dropped.
//!
//! Fixtures live under `tests/fixtures/` and are embedded at compile time
//! via `include_bytes!`. None of the test files are shipped in the crate.

use tempfile::TempDir;
use wow_sharedmedia::{
	ImportOptions, MediaType, UpdateOptions, ensure_addon_dir, import_media, read_data, remove_media, update_media,
};

// ---------------------------------------------------------------------------
// Test asset helpers — write embedded fixtures to the temp dir
// ---------------------------------------------------------------------------

/// Montserrat-Bold.ttf (SIL Open Font License).
fn fixture_font(dir: &std::path::Path) -> std::path::PathBuf {
	let path = dir.join("Montserrat-Bold.ttf");
	std::fs::write(&path, include_bytes!("fixtures/Montserrat-Bold.ttf")).unwrap();
	path
}

/// A real PNG image (background).
fn fixture_background(dir: &std::path::Path) -> std::path::PathBuf {
	let path = dir.join("test_background.png");
	std::fs::write(&path, include_bytes!("fixtures/test_background.png")).unwrap();
	path
}

/// A real TGA texture (statusbar).
fn fixture_statusbar(dir: &std::path::Path) -> std::path::PathBuf {
	let path = dir.join("test_statusbar.tga");
	std::fs::write(&path, include_bytes!("fixtures/test_statusbar.tga")).unwrap();
	path
}

/// Write a minimal valid WAV file (programmatic, no fixture needed).
fn create_test_wav(path: &std::path::Path, sample_rate: u32, channels: u16, samples: &[i16]) {
	let bits_per_sample: u16 = 16;
	let block_align: u16 = channels * (bits_per_sample / 8);
	let byte_rate: u32 = sample_rate * block_align as u32;
	let data_size: u32 = std::mem::size_of_val(samples) as u32;
	let riff_size: u32 = 36 + data_size;

	let mut bytes = Vec::with_capacity((44 + data_size) as usize);
	bytes.extend_from_slice(b"RIFF");
	bytes.extend_from_slice(&riff_size.to_le_bytes());
	bytes.extend_from_slice(b"WAVE");
	bytes.extend_from_slice(b"fmt ");
	bytes.extend_from_slice(&16u32.to_le_bytes());
	bytes.extend_from_slice(&1u16.to_le_bytes());
	bytes.extend_from_slice(&channels.to_le_bytes());
	bytes.extend_from_slice(&sample_rate.to_le_bytes());
	bytes.extend_from_slice(&byte_rate.to_le_bytes());
	bytes.extend_from_slice(&block_align.to_le_bytes());
	bytes.extend_from_slice(&bits_per_sample.to_le_bytes());
	bytes.extend_from_slice(b"data");
	bytes.extend_from_slice(&data_size.to_le_bytes());
	for sample in samples {
		bytes.extend_from_slice(&sample.to_le_bytes());
	}
	std::fs::write(path, bytes).unwrap();
}

// ---------------------------------------------------------------------------
// E2E tests
// ---------------------------------------------------------------------------

#[test]
fn e2e_full_lifecycle_all_media_types() {
	let tmp = TempDir::new().unwrap();
	let addon_dir = tmp.path().join("!!!WindMedia");

	// Phase 1: Initialize
	let data = ensure_addon_dir(&addon_dir).unwrap();
	assert_eq!(data.entries.len(), 0);
	assert!(addon_dir.join("data.lua").exists());
	assert!(addon_dir.join("loader.lua").exists());
	assert!(addon_dir.join("!!!WindMedia.toc").exists());
	for subdir in ["statusbar", "background", "border", "font", "sound"] {
		assert!(addon_dir.join(format!("media/{subdir}")).is_dir());
	}

	// Phase 2: Import statusbar (real TGA fixture)
	let tga = fixture_statusbar(tmp.path());
	let sb = import_media(&addon_dir, ImportOptions::new(MediaType::Statusbar, "Wind Tools", &tga)).unwrap();
	assert_eq!(sb.entry.key, "Wind Tools");
	assert!(sb.entry.file.ends_with(".tga"));
	assert!(addon_dir.join(&sb.entry.file).exists());

	// Phase 3: Import background (real PNG fixture)
	let png = fixture_background(tmp.path());
	let bg = import_media(&addon_dir, ImportOptions::new(MediaType::Background, "Art BG", &png)).unwrap();
	assert_eq!(bg.entry.key, "Art BG");
	assert!(bg.entry.file.ends_with(".tga"));
	assert!(addon_dir.join(&bg.entry.file).exists());

	// Phase 4: Import sound (WAV → OGG, programmatic)
	let wav = tmp.path().join("click.wav");
	create_test_wav(&wav, 44_100, 1, &[0, 8192, -8192, 0]);
	let snd = import_media(&addon_dir, ImportOptions::new(MediaType::Sound, "Click", &wav)).unwrap();
	assert_eq!(snd.entry.key, "Click");
	assert!(snd.entry.file.ends_with(".ogg"));
	assert!(snd.entry.metadata.as_ref().unwrap().audio_duration_secs > Some(0.0));
	assert!(addon_dir.join(&snd.entry.file).exists());

	// Phase 5: Import font (real TTF fixture)
	let ttf = fixture_font(tmp.path());
	let fnt = import_media(&addon_dir, ImportOptions::new(MediaType::Font, "Montserrat Bold", &ttf)).unwrap();
	assert_eq!(fnt.entry.key, "Montserrat Bold");
	assert!(fnt.entry.file.ends_with(".ttf"));
	assert!(addon_dir.join(&fnt.entry.file).exists());
	let meta = fnt.entry.metadata.as_ref().unwrap();
	assert!(meta.font_num_glyphs.is_some_and(|n| n > 0));
	assert!(!meta.font_family.as_deref().is_none_or(|s| s.is_empty()));

	// Phase 6: Verify auto-generated Lua uses single-line comments
	let lua = std::fs::read_to_string(addon_dir.join("data.lua")).unwrap();
	assert!(!lua.contains("--[["));
	assert!(lua.contains("-- Generated:"));
	assert!(lua.contains("-- Tool: wow-sharedmedia"));

	let loader = std::fs::read_to_string(addon_dir.join("loader.lua")).unwrap();
	assert!(!loader.contains("--[["));
	assert!(loader.contains("-- Media registration loader"));
	assert!(loader.contains("-- Version:"));

	// Phase 7: Verify vendor libraries deployed
	assert!(addon_dir.join("libraries/LibStub/LibStub.lua").exists());
	assert!(
		addon_dir
			.join("libraries/CallbackHandler-1.0/CallbackHandler-1.0.lua")
			.exists()
	);
	assert!(
		addon_dir
			.join("libraries/LibSharedMedia-3.0/LibSharedMedia-3.0.lua")
			.exists()
	);

	// Phase 8: Read back and verify entry count
	let data = read_data(&addon_dir).unwrap();
	assert_eq!(data.entries.len(), 4);

	// Phase 9: Update the statusbar key + tags
	update_media(
		&addon_dir,
		&sb.entry.id,
		UpdateOptions {
			key: Some("Wind Tools Pro".into()),
			locales: None,
			tags: Some(vec!["clean".into(), "updated".into()]),
		},
	)
	.unwrap();

	let data = read_data(&addon_dir).unwrap();
	let bar = data
		.entries
		.iter()
		.find(|e| e.media_type == MediaType::Statusbar)
		.unwrap();
	assert_eq!(bar.key, "Wind Tools Pro");
	assert_eq!(bar.tags, vec!["clean", "updated"]);

	// Phase 10: Remove the sound entry
	let _ = remove_media(&addon_dir, &snd.entry.id).unwrap();
	assert!(!addon_dir.join(&snd.entry.file).exists());

	// Phase 11: Remove the background entry
	let _ = remove_media(&addon_dir, &bg.entry.id).unwrap();
	assert!(!addon_dir.join(&bg.entry.file).exists());

	// Phase 12: Verify final state
	let data = read_data(&addon_dir).unwrap();
	assert_eq!(data.entries.len(), 2);
	let keys: Vec<&str> = data.entries.iter().map(|e| e.key.as_str()).collect();
	assert!(keys.contains(&"Wind Tools Pro"));
	assert!(keys.contains(&"Montserrat Bold"));
}

#[test]
fn e2e_import_duplicate_key_rejected() {
	let tmp = TempDir::new().unwrap();
	let addon_dir = tmp.path().join("!!!WindMedia");
	ensure_addon_dir(&addon_dir).unwrap();

	let tga = fixture_statusbar(tmp.path());
	import_media(&addon_dir, ImportOptions::new(MediaType::Statusbar, "Dupe", &tga)).unwrap();

	let result = import_media(&addon_dir, ImportOptions::new(MediaType::Statusbar, "Dupe", &tga));
	assert!(result.is_err());
}

#[test]
fn e2e_remove_nonexistent_id_fails() {
	let tmp = TempDir::new().unwrap();
	let addon_dir = tmp.path().join("!!!WindMedia");
	ensure_addon_dir(&addon_dir).unwrap();

	let result = remove_media(&addon_dir, &uuid::Uuid::new_v4());
	assert!(result.is_err());
}

#[test]
fn e2e_data_lua_roundtrip_preserves_chinese_keys() {
	let tmp = TempDir::new().unwrap();
	let addon_dir = tmp.path().join("!!!WindMedia");
	ensure_addon_dir(&addon_dir).unwrap();

	let tga = fixture_statusbar(tmp.path());
	let imported = import_media(&addon_dir, ImportOptions::new(MediaType::Statusbar, "清风明月", &tga)).unwrap();

	let data = read_data(&addon_dir).unwrap();
	assert_eq!(data.entries[0].key, "清风明月");
	assert_eq!(data.entries[0].id, imported.entry.id);
}

#[test]
fn e2e_data_lua_is_valid_lua() {
	let tmp = TempDir::new().unwrap();
	let addon_dir = tmp.path().join("!!!WindMedia");
	ensure_addon_dir(&addon_dir).unwrap();

	let tga = fixture_statusbar(tmp.path());
	import_media(&addon_dir, ImportOptions::new(MediaType::Statusbar, "Bar", &tga)).unwrap();

	let lua = mlua::Lua::new();
	let content = std::fs::read_to_string(addon_dir.join("data.lua")).unwrap();
	let wrapped = format!("return function(...)\n{}\nend", content);
	let func: mlua::Function = lua.load(&wrapped).eval().unwrap();

	let addon = lua.create_table().unwrap();
	func.call::<()>(("!!!WindMedia".to_string(), addon.clone())).unwrap();

	let data_val: mlua::Value = addon.get("data").unwrap();
	assert!(matches!(data_val, mlua::Value::Table(_)));
}

#[test]
fn e2e_idempotent_ensure_addon_dir() {
	let tmp = TempDir::new().unwrap();
	let addon_dir = tmp.path().join("!!!WindMedia");

	let data1 = ensure_addon_dir(&addon_dir).unwrap();

	let tga = fixture_statusbar(tmp.path());
	import_media(&addon_dir, ImportOptions::new(MediaType::Statusbar, "Bar", &tga)).unwrap();

	let data2 = ensure_addon_dir(&addon_dir).unwrap();
	assert_eq!(data1.entries.len(), 0);
	assert_eq!(data2.entries.len(), 1);
}

#[test]
fn e2e_plain_folder_name() {
	let tmp = TempDir::new().unwrap();
	let addon_dir = tmp.path().join("WindMedia");

	// Phase 1: Initialize — TOC file name and title derive from folder name
	let data = ensure_addon_dir(&addon_dir).unwrap();
	assert_eq!(data.entries.len(), 0);
	assert!(addon_dir.join("data.lua").exists());
	assert!(addon_dir.join("loader.lua").exists());
	assert!(addon_dir.join("WindMedia.toc").exists());
	assert!(!addon_dir.join("!!!WindMedia.toc").exists());
	for subdir in ["statusbar", "background", "border", "font", "sound"] {
		assert!(addon_dir.join(format!("media/{subdir}")).is_dir());
	}

	// Verify TOC content — no `!` stripping needed for plain names
	let toc = std::fs::read_to_string(addon_dir.join("WindMedia.toc")).unwrap();
	assert!(toc.contains("## Title: WindMedia"));
	assert!(!toc.contains("## Title: !"));

	// Phase 2: Import statusbar
	let tga = fixture_statusbar(tmp.path());
	let sb = import_media(&addon_dir, ImportOptions::new(MediaType::Statusbar, "Plain Bar", &tga)).unwrap();
	assert_eq!(sb.entry.key, "Plain Bar");
	assert!(addon_dir.join(&sb.entry.file).exists());

	// Phase 3: Read back
	let data = read_data(&addon_dir).unwrap();
	assert_eq!(data.entries.len(), 1);
	assert_eq!(data.entries[0].key, "Plain Bar");

	// Phase 4: Update
	let updated = update_media(
		&addon_dir,
		&sb.entry.id,
		UpdateOptions {
			key: Some("Renamed Bar".into()),
			locales: None,
			tags: Some(vec!["plain".into()]),
		},
	)
	.unwrap();
	assert_eq!(updated.key, "Renamed Bar");

	// Phase 5: Remove
	let _ = remove_media(&addon_dir, &sb.entry.id).unwrap();
	assert!(!addon_dir.join(&sb.entry.file).exists());

	// Phase 6: Verify final state
	let data = read_data(&addon_dir).unwrap();
	assert!(data.entries.is_empty());
}

#[test]
fn e2e_custom_addon_name() {
	let tmp = TempDir::new().unwrap();
	let addon_dir = tmp.path().join("!!MyCustomMedia");

	let data = ensure_addon_dir(&addon_dir).unwrap();
	assert_eq!(data.entries.len(), 0);
	assert!(addon_dir.join("!!MyCustomMedia.toc").exists());

	let toc = std::fs::read_to_string(addon_dir.join("!!MyCustomMedia.toc")).unwrap();
	assert!(toc.contains("## Title: MyCustomMedia"));
	assert!(!toc.contains("WindMedia"));

	let tga = fixture_statusbar(tmp.path());
	let sb = import_media(&addon_dir, ImportOptions::new(MediaType::Statusbar, "Custom Bar", &tga)).unwrap();
	assert_eq!(sb.entry.key, "Custom Bar");
	assert!(addon_dir.join(&sb.entry.file).exists());

	let data = read_data(&addon_dir).unwrap();
	assert_eq!(data.entries.len(), 1);

	let loader = std::fs::read_to_string(addon_dir.join("loader.lua")).unwrap();
	assert!(loader.contains("ADDON_NAME"));
	assert!(!loader.contains("MyCustomMedia"));
}
