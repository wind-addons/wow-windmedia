//! Image format conversion (PNG/WebP/JPEG/BLP → TGA).

use std::path::Path;

use base64::Engine;
use image::ImageEncoder;

use crate::Error;

/// Maximum texture dimension (pixels).
const MAX_DIMENSION: u32 = 4096;

/// Convert an image file to TGA format for WoW addon storage.
///
/// Opens the source file, converts to DynamicImage, then delegates to
/// `write_dynamic_image_as_tga` for power-of-two resizing and TGA encoding.
pub fn convert_to_tga(input: &Path, output: &Path) -> Result<ImageConvertResult, Error> {
	let img = image::open(input).map_err(|e| Error::InvalidImage(e.to_string()))?;
	write_dynamic_image_as_tga(&img, output)
}

/// Convert any supported image to a PNG data URI for frontend display.
///
/// Returns a `data:image/png;base64,...` string suitable for use as
/// an `<img>` src attribute in the Tauri webview.
pub fn convert_to_preview_data_uri(input: &Path) -> Result<String, Error> {
	let img = image::open(input).map_err(|e| Error::InvalidImage(e.to_string()))?;

	// Write PNG to buffer
	let mut buf = std::io::Cursor::new(Vec::new());
	img.write_to(&mut buf, image::ImageFormat::Png)
		.map_err(|e| Error::ImageConversion(e.to_string()))?;

	Ok(format!(
		"data:image/png;base64,{}",
		base64::engine::general_purpose::STANDARD.encode(buf.into_inner())
	))
}

/// Convert a BLP file to TGA format.
///
/// Decodes the BLP via wow_blp, applies power-of-two resizing,
/// and writes as TGA to the output path.
#[allow(dead_code)]
pub(crate) fn convert_blp_to_tga(input: &Path, output: &Path) -> Result<ImageConvertResult, Error> {
	let dynamic = super::blp::read_blp(input)?;
	write_dynamic_image_as_tga(&dynamic, output)
}

/// Convert a `DynamicImage` to TGA with power-of-two resizing.
///
/// Shared implementation used by both `convert_to_tga` and `convert_blp_to_tga`.
fn write_dynamic_image_as_tga(img: &image::DynamicImage, output: &Path) -> Result<ImageConvertResult, Error> {
	let original_width = img.width();
	let original_height = img.height();

	let width = original_width.next_power_of_two().min(MAX_DIMENSION);
	let height = original_height.next_power_of_two().min(MAX_DIMENSION);
	let was_resized = width != original_width || height != original_height;

	let resized = if was_resized {
		img.resize_exact(width, height, image::imageops::FilterType::Triangle)
	} else {
		img.clone()
	};

	let rgba = resized.to_rgba8();
	let (w, h) = rgba.dimensions();
	let pixels = rgba.as_raw();

	if let Some(parent) = output.parent() {
		std::fs::create_dir_all(parent).map_err(|e| Error::Io {
			source: e,
			path: parent.to_path_buf(),
		})?;
	}
	let file = std::fs::File::create(output).map_err(|e| Error::Io {
		source: e,
		path: output.to_path_buf(),
	})?;
	let writer = std::io::BufWriter::new(file);
	let encoder = image::codecs::tga::TgaEncoder::new(writer);
	encoder
		.write_image(pixels, w, h, image::ExtendedColorType::Rgba8)
		.map_err(|e| Error::ImageConversion(e.to_string()))?;

	Ok(ImageConvertResult {
		width: w,
		height: h,
		original_width,
		original_height,
		was_resized,
	})
}

/// Result of an image conversion operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageConvertResult {
	/// Output width in pixels.
	pub width: u32,
	/// Output height in pixels.
	pub height: u32,
	/// Source width before resizing.
	pub original_width: u32,
	/// Source height before resizing.
	pub original_height: u32,
	/// Whether the image was resized to match WoW texture constraints.
	pub was_resized: bool,
}

#[cfg(test)]
mod tests {
	use super::*;
	use tempfile::TempDir;

	fn create_test_image(dir: &std::path::Path, name: &str, w: u32, h: u32) -> std::path::PathBuf {
		let mut img = image::RgbImage::new(w, h);
		for pixel in img.pixels_mut() {
			*pixel = image::Rgb([100, 200, 50]);
		}
		let path = dir.join(name);
		img.save(&path).unwrap();
		path
	}

	#[test]
	fn test_convert_png_to_tga() {
		let tmp = TempDir::new().unwrap();
		let input = create_test_image(tmp.path(), "test.png", 64, 32);
		let output = tmp.path().join("test.tga");
		let result = convert_to_tga(&input, &output).unwrap();
		assert_eq!(result.width, 64);
		assert_eq!(result.height, 32);
		assert!(!result.was_resized);
		assert!(output.exists());
	}

	#[test]
	fn test_resize_non_power_of_two() {
		let tmp = TempDir::new().unwrap();
		let input = create_test_image(tmp.path(), "np2.png", 100, 50);
		let output = tmp.path().join("np2.tga");
		let result = convert_to_tga(&input, &output).unwrap();
		assert_eq!(result.width, 128);
		assert_eq!(result.height, 64);
		assert!(result.was_resized);
	}

	#[test]
	fn test_preview_data_uri() {
		let tmp = TempDir::new().unwrap();
		let input = create_test_image(tmp.path(), "preview.png", 32, 32);
		let uri = convert_to_preview_data_uri(&input).unwrap();
		assert!(uri.starts_with("data:image/png;base64,"));
	}

	#[test]
	fn test_zero_dimensions() {
		let tmp = TempDir::new().unwrap();
		let mut img = image::RgbImage::new(1, 1);
		for pixel in img.pixels_mut() {
			*pixel = image::Rgb([0; 3]);
		}
		let path = tmp.path().join("tiny.png");
		img.save(&path).unwrap();
		let result = convert_to_tga(&path, &tmp.path().join("out.tga"));
		assert!(result.is_ok());
	}
}
