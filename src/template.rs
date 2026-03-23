//! Addon template management for `loader.lua` and `!!!WindMedia.toc`.
//!
//! Template sources live in `templates/` and are embedded into the crate with
//! `include_str!`. Rust is responsible only for version and interface
//! substitution plus writing the final files to disk.

use std::path::Path;

use crate::Error;

const TOC_INTERFACE: &str = "120001";
const LOADER_TEMPLATE: &str = include_str!("../templates/loader.lua");
const TOC_TEMPLATE: &str = include_str!("../templates/!!!WindMedia.toc");

const LIBSTUB_LUA: &str = include_str!("../vendor/libsharedmedia-3.0/LibStub/LibStub.lua");
const CALLBACKHANDLER_LUA: &str =
	include_str!("../vendor/libsharedmedia-3.0/CallbackHandler-1.0/CallbackHandler-1.0.lua");
const LSM_LUA: &str = include_str!("../vendor/libsharedmedia-3.0/LibSharedMedia-3.0/LibSharedMedia-3.0.lua");
const INNER_LIB_XML: &str = include_str!("../vendor/libsharedmedia-3.0/LibSharedMedia-3.0/lib.xml");

fn generate_loader(version: &str) -> String {
	LOADER_TEMPLATE.replace("__WINDMEDIA_VERSION__", version)
}

fn generate_toc(version: &str) -> String {
	TOC_TEMPLATE
		.replace("__WINDMEDIA_VERSION__", version)
		.replace("__WINDMEDIA_INTERFACE__", TOC_INTERFACE)
}

/// Write template files (`loader.lua`, `!!!WindMedia.toc`) to the addon directory.
///
/// `data.lua` is intentionally excluded because it is managed independently by
/// the registry writer.
pub fn deploy_templates(addon_dir: &Path) -> Result<(), Error> {
	let version = env!("CARGO_PKG_VERSION");

	write_file(addon_dir, "loader.lua", &generate_loader(version))?;
	write_file(addon_dir, "!!!WindMedia.toc", &generate_toc(version))?;

	write_file(addon_dir, "libraries/LibStub/LibStub.lua", LIBSTUB_LUA)?;
	write_file(
		addon_dir,
		"libraries/CallbackHandler-1.0/CallbackHandler-1.0.lua",
		CALLBACKHANDLER_LUA,
	)?;
	write_file(addon_dir, "libraries/LibSharedMedia-3.0/lib.xml", INNER_LIB_XML)?;
	write_file(
		addon_dir,
		"libraries/LibSharedMedia-3.0/LibSharedMedia-3.0.lua",
		LSM_LUA,
	)?;

	Ok(())
}

fn write_file(dir: &Path, filename: &str, content: &str) -> Result<(), Error> {
	let path = dir.join(filename);
	if let Some(parent) = path.parent() {
		std::fs::create_dir_all(parent).map_err(|e| Error::Io {
			source: e,
			path: parent.to_path_buf(),
		})?;
	}
	std::fs::write(&path, content).map_err(|e| Error::Io {
		source: e,
		path: path.clone(),
	})
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::sync::{Arc, Mutex};

	use mlua::{Lua, Value, Variadic};
	use tempfile::TempDir;

	type Registration = (String, String, String, Option<i64>);

	#[test]
	fn test_deploy_creates_files() {
		let dir = TempDir::new().unwrap();
		deploy_templates(dir.path()).unwrap();

		assert!(dir.path().join("loader.lua").exists());
		assert!(dir.path().join("!!!WindMedia.toc").exists());

		let loader = std::fs::read_to_string(dir.path().join("loader.lua")).unwrap();
		assert!(loader.contains("WindMedia loader"));
		assert!(loader.contains("local ADDON_NAME, addon = ..."));
		assert!(loader.contains("BASE_PATH"));
		assert!(loader.contains("ADDON_NAME"));
		assert!(loader.contains(&format!("Version: {}", env!("CARGO_PKG_VERSION"))));

		let toc = std::fs::read_to_string(dir.path().join("!!!WindMedia.toc")).unwrap();
		assert!(toc.contains("data.lua"));
		assert!(toc.contains("loader.lua"));
		assert!(toc.contains("## Title: WindMedia"));
		assert!(toc.contains("## Notes: Provides textures, sounds, and other media for LibSharedMedia addons."));
		assert!(!toc.contains("## Author:"));
		assert!(!toc.contains("!!!WindMedia"));
	}

	#[test]
	fn test_deploy_creates_vendor_files() {
		let dir = TempDir::new().unwrap();
		deploy_templates(dir.path()).unwrap();

		assert!(dir.path().join("libraries/LibStub/LibStub.lua").exists());
		assert!(
			dir.path()
				.join("libraries/CallbackHandler-1.0/CallbackHandler-1.0.lua")
				.exists()
		);
		assert!(
			dir.path()
				.join("libraries/LibSharedMedia-3.0/LibSharedMedia-3.0.lua")
				.exists()
		);
		assert!(dir.path().join("libraries/LibSharedMedia-3.0/lib.xml").exists());
	}

	#[test]
	fn test_deploy_overwrites() {
		let dir = TempDir::new().unwrap();
		deploy_templates(dir.path()).unwrap();

		std::fs::write(dir.path().join("loader.lua"), "corrupted").unwrap();

		deploy_templates(dir.path()).unwrap();
		let loader = std::fs::read_to_string(dir.path().join("loader.lua")).unwrap();
		assert!(loader.contains("WindMedia loader"));
		assert!(loader.contains("Version: "));
		assert!(!loader.contains("DO NOT EDIT MANUALLY"));
		assert!(!loader.contains("Reads the data table"));
	}

	#[test]
	fn test_toc_contains_interface_version() {
		let dir = TempDir::new().unwrap();
		deploy_templates(dir.path()).unwrap();

		let toc = std::fs::read_to_string(dir.path().join("!!!WindMedia.toc")).unwrap();
		assert!(toc.contains(&format!("## Interface: {}", TOC_INTERFACE)));
		assert!(toc.contains(&format!("## Version: {}", env!("CARGO_PKG_VERSION"))));
		assert!(toc.contains("## Title: WindMedia"));
		assert!(toc.contains("## Notes: Provides textures, sounds, and other media for LibSharedMedia addons."));
		assert!(toc.contains("## DefaultState: enabled"));
		assert!(!toc.contains("## Author:"));
		assert!(!toc.contains("!!!WindMedia"));
		assert!(toc.contains("libraries\\LibStub\\LibStub.lua"));
		assert!(toc.contains("LibSharedMedia-3.0\\lib.xml"));
	}

	#[test]
	fn test_toc_orders_libraries_before_runtime_files() {
		let dir = TempDir::new().unwrap();
		deploy_templates(dir.path()).unwrap();

		let toc = std::fs::read_to_string(dir.path().join("!!!WindMedia.toc")).unwrap();
		let lines: Vec<&str> = toc.lines().collect();

		let libstub = lines
			.iter()
			.position(|line| *line == "libraries\\LibStub\\LibStub.lua")
			.unwrap();
		let callbackhandler = lines
			.iter()
			.position(|line| *line == "libraries\\CallbackHandler-1.0\\CallbackHandler-1.0.lua")
			.unwrap();
		let lsm = lines
			.iter()
			.position(|line| *line == "libraries\\LibSharedMedia-3.0\\lib.xml")
			.unwrap();
		let data = lines.iter().position(|line| *line == "data.lua").unwrap();
		let loader = lines.iter().position(|line| *line == "loader.lua").unwrap();

		assert!(libstub < callbackhandler);
		assert!(callbackhandler < lsm);
		assert!(lsm < data);
		assert!(data < loader);
	}

	#[test]
	fn test_toc_skips_data_lua() {
		let dir = TempDir::new().unwrap();
		deploy_templates(dir.path()).unwrap();
		assert!(!dir.path().join("data.lua").exists());
	}

	#[test]
	fn test_loader_uses_dynamic_addon_name() {
		let dir = TempDir::new().unwrap();
		deploy_templates(dir.path()).unwrap();

		let loader = std::fs::read_to_string(dir.path().join("loader.lua")).unwrap();
		assert!(loader.contains("WindMedia loader"));
		assert!(loader.contains("local ADDON_NAME, addon = ..."));
		assert!(loader.contains(r#"Interface\\AddOns\\"#));
		assert!(loader.contains("ADDON_NAME"));
		assert!(loader.contains("data.entries"));
		assert!(!loader.contains("DO NOT EDIT MANUALLY"));
	}

	#[test]
	fn test_generate_loader_reflects_version_changes() {
		let v1 = generate_loader("1.2.3");
		let v2 = generate_loader("9.9.9");

		assert!(v1.contains("Version: 1.2.3"));
		assert!(v2.contains("Version: 9.9.9"));
		assert_ne!(v1, v2);
	}

	#[test]
	fn test_generate_toc_reflects_version_changes() {
		let v1 = generate_toc("0.1.0");
		let v2 = generate_toc("0.2.0");

		assert!(v1.contains("## Version: 0.1.0"));
		assert!(v2.contains("## Version: 0.2.0"));
		assert_ne!(v1, v2);
	}

	#[test]
	fn test_loader_executes_in_lua51_style_runtime() {
		let lua = Lua::new();
		let registrations: Arc<Mutex<Vec<Registration>>> = Arc::new(Mutex::new(Vec::new()));

		let lsm = lua.create_table().unwrap();
		lsm.set("LOCALE_BIT_koKR", 1).unwrap();
		lsm.set("LOCALE_BIT_ruRU", 2).unwrap();
		lsm.set("LOCALE_BIT_zhCN", 4).unwrap();
		lsm.set("LOCALE_BIT_zhTW", 8).unwrap();
		lsm.set("LOCALE_BIT_western", 16).unwrap();

		let regs = registrations.clone();
		let register = lua
			.create_function_mut(move |_, args: Variadic<Value>| {
				let kind = match &args[1] {
					Value::String(s) => s.to_str()?.to_string(),
					other => panic!("unexpected type arg: {other:?}"),
				};
				let key = match &args[2] {
					Value::String(s) => s.to_str()?.to_string(),
					other => panic!("unexpected key arg: {other:?}"),
				};
				let file = match &args[3] {
					Value::String(s) => s.to_str()?.to_string(),
					other => panic!("unexpected file arg: {other:?}"),
				};
				let mask = args.get(4).and_then(|v| match v {
					Value::Integer(i) => Some(*i),
					_ => None,
				});
				regs.lock().unwrap().push((kind, key, file, mask));
				Ok(())
			})
			.unwrap();
		lsm.set("Register", register).unwrap();

		let globals = lua.globals();
		let libstub_lsm = lsm.clone();
		let libstub = lua
			.create_function(move |_, (_name, _silent): (String, bool)| Ok(libstub_lsm.clone()))
			.unwrap();
		globals.set("LibStub", libstub).unwrap();

		let addon = lua.create_table().unwrap();
		let data = lua.create_table().unwrap();
		let entries = lua.create_table().unwrap();

		let font = lua.create_table().unwrap();
		font.set("type", "font").unwrap();
		font.set("key", "Body Font").unwrap();
		font.set("file", "media/font/body.ttf").unwrap();
		let metadata = lua.create_table().unwrap();
		let locales = lua.create_table().unwrap();
		locales.set(1, "western").unwrap();
		locales.set(2, "zhCN").unwrap();
		metadata.set("locales", locales).unwrap();
		font.set("metadata", metadata).unwrap();

		let statusbar = lua.create_table().unwrap();
		statusbar.set("type", "statusbar").unwrap();
		statusbar.set("key", "Smooth").unwrap();
		statusbar.set("file", "media/statusbar/smooth.tga").unwrap();

		entries.set(1, font).unwrap();
		entries.set(2, statusbar).unwrap();
		data.set("entries", entries).unwrap();
		addon.set("data", data).unwrap();

		let loader = generate_loader("1.2.3");
		let wrapped = format!("return function(...)\n{}\nend", loader);
		let func: mlua::Function = lua.load(&wrapped).eval().unwrap();
		func.call::<()>(("!!!WindMedia".to_string(), addon)).unwrap();

		let regs = registrations.lock().unwrap();
		assert_eq!(regs.len(), 2);
		assert_eq!(regs[0].0, "font");
		assert_eq!(regs[0].1, "Body Font");
		assert_eq!(regs[0].2, r#"Interface\AddOns\!!!WindMedia\media/font/body.ttf"#);
		assert_eq!(regs[0].3, Some(20));
		assert_eq!(regs[1].0, "statusbar");
		assert_eq!(regs[1].1, "Smooth");
		assert_eq!(regs[1].2, r#"Interface\AddOns\!!!WindMedia\media/statusbar/smooth.tga"#);
		assert_eq!(regs[1].3, None);
	}
}
