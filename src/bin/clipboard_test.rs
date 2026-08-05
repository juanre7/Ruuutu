// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 juanre7

use anyhow::Result;
use image::RgbaImage;

// Included wholesale but used in part: each tool needs a slice of the module, and
// `#[path]` brings in all of it. The unused half is not dead code, it is the rest of
// the application. Scoped to the include so the tool's own code stays linted.
#[path = "../clipboard.rs"]
#[allow(dead_code)]
mod clipboard;

use clipboard::copy_to_clipboard;

fn main() -> Result<()> {
    println!("Testing clipboard copy...");
    let mut img = RgbaImage::new(100, 100);
    for y in 0..100 {
        for x in 0..100 {
            img.put_pixel(x, y, image::Rgba([255, 0, 0, 255])); // Red 100x100 square
        }
    }

    match copy_to_clipboard(&img) {
        Ok(_) => println!("SUCCESS: Image copied to clipboard successfully!"),
        Err(e) => eprintln!("ERROR copying to clipboard: {:?}", e),
    }

    Ok(())
}
