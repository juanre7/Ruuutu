// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 juanre7

//! Fuzzes the image encoder.
//!
//! `encode_image` is the only place in Ruuutu that crosses into C: the WebP path enters
//! libwebp through `webp::Encoder::from_rgba`, which takes a pointer to the buffer plus a
//! width and a height. If those dimensions and the buffer length ever drift apart — a
//! rescale that rounds badly, a side collapsing to zero — the result is not a Rust error
//! but an out-of-bounds read inside libwebp. Built with sanitizers, this target is what
//! catches it.
//!
//! The axes it walks are the ones the user drives from the tray menu: format, quality, PNG
//! level, lossless and save scale; plus the size of the capture, which comes from whatever
//! area is dragged with the mouse.

#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use ruuutu::storage::{encode_image, OutputFormat, SaveOptions};

/// Structured input. Dimensions are capped at 64×64 so the fuzzer gets thousands of runs
/// per second: geometry bugs (rounding, sides collapsing) show up at small sizes, not at 4K.
#[derive(Debug, Arbitrary)]
struct Input {
    width: u8,
    height: u8,
    format: u8,
    quality: u8,
    png_level: u8,
    lossless: bool,
    scale_percent: u8,
    pixels: Vec<u8>,
}

fuzz_target!(|input: Input| {
    let width = (input.width % 64) as u32 + 1;
    let height = (input.height % 64) as u32 + 1;

    // `RgbaImage::from_raw` demands exactly width*height*4 bytes. The fuzzer's bytes are
    // cycled and the rest zero-filled, so the image is always valid and the target under
    // test stays the encoder rather than the constructor.
    let needed = (width * height * 4) as usize;
    let mut raw = vec![0u8; needed];
    if !input.pixels.is_empty() {
        for (dst, src) in raw.iter_mut().zip(input.pixels.iter().cycle()) {
            *dst = *src;
        }
    }
    let img = image::RgbaImage::from_raw(width, height, raw).expect("dimensions match the buffer");

    let opts = SaveOptions {
        format: match input.format % 3 {
            0 => OutputFormat::WebP,
            1 => OutputFormat::Png,
            _ => OutputFormat::Jpeg,
        },
        quality: input.quality,
        lossless: input.lossless,
        png_level: input.png_level,
        scale_percent: input.scale_percent as u32,
    };

    // `encode_image` may legitimately return `Err` when the encoder rejects the input;
    // what it may not do is panic or corrupt memory. When it reports success the bytes
    // have to be there: an empty file on disk is a silent failure, which is exactly what
    // this assertion rules out.
    if let Ok(bytes) = encode_image(&img, &opts) {
        assert!(!bytes.is_empty(), "encoding produced no bytes for {opts:?}");

        // PNG is the only format here that is both lossless and decodable, so it is the
        // one that can show the dimensions surviving the rescale.
        if opts.format == OutputFormat::Png {
            let decoded = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)
                .expect("the PNG we just wrote has to be readable");
            let expected_w = if opts.scale_percent >= 100 { width } else { (width * opts.scale_percent / 100).max(1) };
            let expected_h = if opts.scale_percent >= 100 { height } else { (height * opts.scale_percent / 100).max(1) };
            assert_eq!(
                (decoded.width(), decoded.height()),
                (expected_w, expected_h),
                "the rescale missed the requested dimensions for {opts:?}"
            );
        }
    }
});
