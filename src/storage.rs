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
