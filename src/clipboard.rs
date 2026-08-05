// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 juanre7

use anyhow::{Context, Result};
use arboard::{Clipboard, ImageData};
use image::RgbaImage;
use std::borrow::Cow;

/// Copies an `RgbaImage` directly to the system clipboard.
pub fn copy_to_clipboard(img: &RgbaImage) -> Result<()> {
    let mut clipboard = Clipboard::new().context("Failed to initialize system clipboard")?;
    let (width, height) = img.dimensions();

    let image_data = ImageData {
        width: width as usize,
        height: height as usize,
        bytes: Cow::Borrowed(img.as_raw()),
    };

    clipboard
        .set_image(image_data)
        .context("Failed to set image to system clipboard")?;

    Ok(())
}
