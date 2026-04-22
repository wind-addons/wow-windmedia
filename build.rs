use std::path::PathBuf;

const REQUIRED_VENDOR_FILES: &[&str] = &[
	"vendor/serpent/serpent.lua",
	"vendor/libsharedmedia-3.0/LibStub/LibStub.lua",
	"vendor/libsharedmedia-3.0/CallbackHandler-1.0/CallbackHandler-1.0.lua",
	"vendor/libsharedmedia-3.0/LibSharedMedia-3.0/LibSharedMedia-3.0.lua",
	"vendor/libsharedmedia-3.0/LibSharedMedia-3.0/lib.xml",
];

fn main() {
	println!("cargo:rerun-if-changed=vendor.lock.json");
	for path in REQUIRED_VENDOR_FILES {
		println!("cargo:rerun-if-changed={path}");
	}

	let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("missing CARGO_MANIFEST_DIR"));
	let missing: Vec<&str> = REQUIRED_VENDOR_FILES
		.iter()
		.copied()
		.filter(|path| !root.join(path).exists())
		.collect();

	if !missing.is_empty() {
		panic!(
			"missing vendored assets required for compile-time embedding: {}\nRun `mise run vendor:update` to materialize the pinned snapshot from vendor.lock.json before building.",
			missing.join(", "),
		);
	}

	// TOC_INTERFACE is derived from the vendor LibSharedMedia .toc file.
	// Override with: TOC_INTERFACE=110007 cargo build
	println!("cargo:rerun-if-env-changed=TOC_INTERFACE");

	if let Ok(override_val) = std::env::var("TOC_INTERFACE") {
		println!("cargo:rustc-env=TOC_INTERFACE={override_val}");
	} else {
		let toc_path = root.join("vendor/libsharedmedia-3.0/LibSharedMedia-3.0.toc");
		let content = std::fs::read_to_string(&toc_path)
			.unwrap_or_else(|e| panic!("Failed to read vendor TOC: {e}"));

		let interface_line = content
			.lines()
			.find(|line| line.starts_with("## Interface:"))
			.expect("No `## Interface:` line found in vendor/libsharedmedia-3.0/LibSharedMedia-3.0.toc");

		let max_version = interface_line
			.trim_start_matches("## Interface:")
			.split(',')
			.filter_map(|v| v.trim().parse::<u32>().ok())
			.max()
			.expect("No valid Interface versions found");

		println!("cargo:rustc-env=TOC_INTERFACE={max_version}");
	}
}
