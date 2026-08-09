// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 juanre7

use anyhow::{Context, Result};
use chrono::Local;
use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::{CompressionType, FilterType as PngFilterType, PngEncoder};
use image::imageops::FilterType;
use image::{ExtendedColorType, ImageEncoder, RgbaImage};
use std::path::{Path, PathBuf};

/// Supported image formats for Ruuutu.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    WebP,
    Png,
    Jpeg,
}

impl OutputFormat {
    pub fn extension(&self) -> &'static str {
        match self {
            OutputFormat::WebP => "webp",
            OutputFormat::Png => "png",
            OutputFormat::Jpeg => "jpg",
        }
    }
}

/// Encoding and downscaling settings resolved from `AppConfig`.
///
/// Each format honours a different subset of these fields, because "quality"
/// does not mean the same thing in all three:
/// - WebP: lossy VP8 at `quality`, or VP8L when `lossless`.
/// - JPEG: lossy at `quality`. Alpha is dropped (the format has no alpha channel).
/// - PNG: always lossless. Only `png_level` applies, and it changes file size and
///   encoding time, never the pixels.
#[derive(Debug, Clone, Copy)]
pub struct SaveOptions {
    pub format: OutputFormat,
    /// Lossy quality, 1..=100.
    pub quality: u8,
    /// WebP only: encode VP8L lossless and ignore `quality`.
    pub lossless: bool,
    /// PNG only: DEFLATE level 1..=9.
    pub png_level: u8,
    /// Downscale applied before encoding. 100 = original resolution.
    pub scale_percent: u32,
}

impl Default for SaveOptions {
    fn default() -> Self {
        Self {
            format: OutputFormat::WebP,
            quality: 90,
            lossless: false,
            png_level: 4,
            scale_percent: 100,
        }
    }
}

/// Returns the user's Screenshots directory (~/Pictures/Ruuutu), creating it if necessary.
pub fn get_screenshots_dir() -> Result<PathBuf> {
    let base_dir = dirs::picture_dir()
        .or_else(dirs::desktop_dir)
        .unwrap_or_else(|| PathBuf::from("."));

    let screenshots_dir = base_dir.join("Ruuutu");
    if !screenshots_dir.exists() {
        std::fs::create_dir_all(&screenshots_dir)
            .context("Failed to create Ruuutu Screenshots directory")?;
    }
    Ok(screenshots_dir)
}

/// Downscales with Lanczos3 (best detail retention for text-heavy screenshots).
/// Returns `None` when no resampling is needed, so the caller can skip the copy.
///
/// Public because the capture flow resamples once up front and hands the same
/// pixels to both the encoder and the clipboard.
pub fn downscaled(img: &RgbaImage, scale_percent: u32) -> Option<RgbaImage> {
    if scale_percent >= 100 {
        return None;
    }
    let w = (img.width() * scale_percent / 100).max(1);
    let h = (img.height() * scale_percent / 100).max(1);
    Some(image::imageops::resize(img, w, h, FilterType::Lanczos3))
}

/// Downscales and encodes into an in-memory buffer.
pub fn encode_image(img: &RgbaImage, opts: &SaveOptions) -> Result<Vec<u8>> {
    let resized = downscaled(img, opts.scale_percent);
    let img = resized.as_ref().unwrap_or(img);
    let (w, h) = img.dimensions();
    let quality = opts.quality.clamp(1, 100);
    let mut out = Vec::new();

    match opts.format {
        OutputFormat::WebP => {
            let encoder = webp::Encoder::from_rgba(img.as_raw(), w, h);
            let mem = if opts.lossless {
                encoder.encode_lossless()
            } else {
                encoder.encode(quality as f32)
            };
            out.extend_from_slice(&mem);
        }
        OutputFormat::Png => {
            PngEncoder::new_with_quality(
                &mut out,
                CompressionType::Level(opts.png_level.clamp(1, 9)),
                PngFilterType::Adaptive,
            )
            .write_image(img.as_raw(), w, h, ExtendedColorType::Rgba8)
            .context("PNG encoding failed")?;
        }
        OutputFormat::Jpeg => {
            // The JPEG encoder rejects Rgba8 outright, so drop alpha first.
            let rgb = image::DynamicImage::ImageRgba8(img.clone()).into_rgb8();
            JpegEncoder::new_with_quality(&mut out, quality)
                .write_image(rgb.as_raw(), w, h, ExtendedColorType::Rgb8)
                .context("JPEG encoding failed")?;
        }
    }

    Ok(out)
}

/// Saves an `RgbaImage` to the Screenshots directory automatically.
pub fn save_image(img: &RgbaImage, opts: &SaveOptions) -> Result<PathBuf> {
    let dir = get_screenshots_dir()?;
    let timestamp = Local::now().format("%Y-%m-%d_%H%M%S");
    let filename = format!("Ruuutu_{}.{}", timestamp, opts.format.extension());
    let file_path = dir.join(&filename);

    let bytes = encode_image(img, opts)?;
    std::fs::write(&file_path, bytes)
        .with_context(|| format!("Failed to save screenshot to {:?}", file_path))?;

    Ok(file_path)
}

/// Encodes and writes to an explicit path chosen elsewhere (the native save dialog).
pub fn save_image_to(img: &RgbaImage, path: &Path, opts: &SaveOptions) -> Result<()> {
    let bytes = encode_image(img, opts)?;
    std::fs::write(path, bytes)
        .with_context(|| format!("Failed to save screenshot to {:?}", path))?;
    Ok(())
}

/// Timestamped file name for the given format, e.g. `Ruuutu_2026-08-05_143012.webp`.
pub fn default_file_name(format: OutputFormat) -> String {
    format!(
        "Ruuutu_{}.{}",
        Local::now().format("%Y-%m-%d_%H%M%S"),
        format.extension()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `get_screenshots_dir` and `save_image` are deliberately not exercised here: both
    /// resolve `~/Pictures` and would create a directory in the home of whoever runs the
    /// suite. Everything they do beyond that is `encode_image` plus `fs::write`, and
    /// `save_image_to` covers that pair against a temporary directory instead.
    fn temp_dir() -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("ruuutu-test-{}-{}", std::process::id(), nanos));
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    /// A smooth gradient, deliberately: a flat colour compresses to almost nothing and
    /// leaves the size comparisons below inside the noise, while a high-frequency pattern
    /// swings the other way — a regular one compresses *better* than its own resample,
    /// because Lanczos3 turns it into something DEFLATE cannot predict. A gradient is
    /// both compressible and well-behaved under resampling, which is what these
    /// assertions are actually about.
    fn sample_image(w: u32, h: u32) -> RgbaImage {
        RgbaImage::from_fn(w, h, |x, y| {
            image::Rgba([(x * 255 / w.max(1)) as u8, (y * 255 / h.max(1)) as u8, ((x + y) / 2) as u8, 255])
        })
    }

    const ALL_FORMATS: [OutputFormat; 3] = [OutputFormat::WebP, OutputFormat::Png, OutputFormat::Jpeg];

    #[test]
    fn each_format_carries_its_own_extension() {
        assert_eq!(OutputFormat::WebP.extension(), "webp");
        assert_eq!(OutputFormat::Png.extension(), "png");
        assert_eq!(OutputFormat::Jpeg.extension(), "jpg");
    }

    /// The default is what the tray ships with on a fresh install.
    #[test]
    fn the_default_options_are_lossy_webp_at_full_size() {
        let opts = SaveOptions::default();
        assert_eq!(opts.format, OutputFormat::WebP);
        assert_eq!(opts.quality, 90);
        assert!(!opts.lossless);
        assert_eq!(opts.scale_percent, 100);
    }

    /// `Ruuutu_YYYY-MM-DD_HHMMSS.ext`, which is the name promised in the README.
    #[test]
    fn the_default_file_name_is_a_timestamp_with_the_right_extension() {
        for format in ALL_FORMATS {
            let name = default_file_name(format);
            let stamp = name
                .strip_prefix("Ruuutu_")
                .and_then(|n| n.strip_suffix(&format!(".{}", format.extension())))
                .unwrap_or_else(|| panic!("unexpected file name {name}"));

            assert_eq!(stamp.len(), 17, "{name} does not carry a YYYY-MM-DD_HHMMSS stamp");
            assert_eq!(stamp.as_bytes()[4], b'-');
            assert_eq!(stamp.as_bytes()[7], b'-');
            assert_eq!(stamp.as_bytes()[10], b'_');
            assert!(
                stamp.chars().filter(|c| c.is_ascii_digit()).count() == 14,
                "{name} has non-digits inside the stamp"
            );
        }
    }

    /// 100 % and above must hand back `None` so the caller can skip a full-image copy.
    #[test]
    fn downscaling_is_skipped_at_full_size_or_larger() {
        let img = sample_image(120, 80);
        assert!(downscaled(&img, 100).is_none());
        assert!(downscaled(&img, 150).is_none());
    }

    /// 120x80 at 25 % is exactly 30x20. The off-by-one this guards against is the
    /// reason the arithmetic multiplies before dividing.
    #[test]
    fn downscaling_lands_on_exact_dimensions() {
        let img = sample_image(120, 80);
        for (percent, expected) in [(75, (90, 60)), (50, (60, 40)), (25, (30, 20))] {
            let out = downscaled(&img, percent).expect("a resample below 100%");
            assert_eq!(out.dimensions(), expected, "{percent}% of 120x80");
        }
    }

    /// A 1x1 image at 25 % rounds to zero, and an image of zero width cannot be encoded.
    /// The `.max(1)` in `downscaled` is what keeps that from happening.
    #[test]
    fn downscaling_never_collapses_a_side_to_zero() {
        let out = downscaled(&sample_image(1, 1), 25).expect("a resample below 100%");
        assert_eq!(out.dimensions(), (1, 1));
    }

    /// Each encoder must produce bytes its own format's magic number identifies. This is
    /// what proves the JPEG branch really drops alpha: the encoder rejects `Rgba8`
    /// outright, so a regression there surfaces as an error, not as a wrong pixel.
    #[test]
    fn every_format_encodes_rgba_into_its_own_container() {
        let img = sample_image(120, 80);
        for format in ALL_FORMATS {
            let bytes = encode_image(&img, &SaveOptions { format, ..Default::default() })
                .unwrap_or_else(|e| panic!("{format:?} failed to encode: {e}"));
            assert!(!bytes.is_empty(), "{format:?} encoded to nothing");

            match format {
                OutputFormat::WebP => {
                    assert_eq!(&bytes[0..4], b"RIFF");
                    assert_eq!(&bytes[8..12], b"WEBP");
                }
                OutputFormat::Png => assert_eq!(&bytes[0..8], b"\x89PNG\r\n\x1a\n"),
                OutputFormat::Jpeg => assert_eq!(&bytes[0..2], &[0xFF, 0xD8]),
            }
        }
    }

    /// Proof the quality setting reaches the encoder at all: VP8L and VP8 are different
    /// codecs, so identical output would mean the `lossless` flag is being ignored.
    #[test]
    fn webp_lossless_differs_from_lossy() {
        let img = sample_image(120, 80);
        let lossy = encode_image(&img, &SaveOptions { quality: 50, ..Default::default() }).unwrap();
        let lossless = encode_image(&img, &SaveOptions { lossless: true, ..Default::default() }).unwrap();
        assert_ne!(lossy, lossless);
    }

    /// PNG is always lossless, so the compression level is the only lever it has, and it
    /// must move the file size without touching a single pixel.
    #[test]
    fn the_png_level_changes_size_but_not_pixels() {
        let img = sample_image(200, 150);
        let decode = |level: u8| {
            let bytes = encode_image(
                &img,
                &SaveOptions { format: OutputFormat::Png, png_level: level, ..Default::default() },
            )
            .unwrap();
            (bytes.len(), image::load_from_memory(&bytes).unwrap().to_rgba8())
        };

        let (fast_len, fast_px) = decode(1);
        let (best_len, best_px) = decode(9);
        assert!(best_len < fast_len, "level 9 ({best_len}) did not beat level 1 ({fast_len})");
        assert_eq!(fast_px, best_px, "the DEFLATE level altered the pixels");
        assert_eq!(fast_px, img, "PNG did not round-trip losslessly");
    }

    /// Scale is the only lever that shrinks a PNG for real, and it is applied before
    /// encoding — so the file both weighs less and decodes back at the smaller size.
    #[test]
    fn scaling_shrinks_the_encoded_png_and_its_dimensions() {
        let img = sample_image(200, 150);
        let full = encode_image(&img, &SaveOptions { format: OutputFormat::Png, ..Default::default() }).unwrap();
        let half = encode_image(
            &img,
            &SaveOptions { format: OutputFormat::Png, scale_percent: 50, ..Default::default() },
        )
        .unwrap();

        assert!(half.len() < full.len(), "50% ({}) did not beat 100% ({})", half.len(), full.len());
        let decoded = image::load_from_memory(&half).unwrap().to_rgba8();
        assert_eq!(decoded.dimensions(), (100, 75));
    }

    /// A quality outside 1..=100 must be clamped rather than reach the encoder, which
    /// takes it as a percentage and has no defined behaviour past the ends.
    #[test]
    fn quality_outside_the_valid_range_is_clamped() {
        let img = sample_image(60, 40);
        for quality in [0, 101, 255] {
            for format in [OutputFormat::WebP, OutputFormat::Jpeg] {
                let bytes = encode_image(&img, &SaveOptions { format, quality, ..Default::default() });
                assert!(bytes.map(|b| !b.is_empty()).unwrap_or(false), "{format:?} at quality {quality}");
            }
        }
    }

    /// The path the native save dialog takes: encode, then write exactly where the user
    /// pointed. The bytes on disk must be the same ones the encoder produced.
    #[test]
    fn saving_to_an_explicit_path_writes_the_encoded_bytes() {
        let img = sample_image(120, 80);
        let dir = temp_dir();

        for format in ALL_FORMATS {
            let opts = SaveOptions { format, ..Default::default() };
            let path = dir.join(default_file_name(format));
            save_image_to(&img, &path, &opts).expect("write to a temp path");

            assert!(path.exists(), "{format:?} left no file behind");
            assert_eq!(std::fs::read(&path).unwrap(), encode_image(&img, &opts).unwrap());
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Saving must fail cleanly, not panic, when the directory does not exist — the
    /// dialog can hand back a path on a drive that has since gone away.
    #[test]
    fn saving_into_a_missing_directory_reports_an_error() {
        let path = temp_dir().join("no-such-subdir").join("shot.png");
        let opts = SaveOptions { format: OutputFormat::Png, ..Default::default() };
        assert!(save_image_to(&sample_image(20, 20), &path, &opts).is_err());
    }
}
