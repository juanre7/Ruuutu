// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 juanre7

use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use std::sync::{Mutex, OnceLock};

static CONSOLAS_BOLD_FONT: OnceLock<Option<Vec<u8>>> = OnceLock::new();
static PARSED_FONT: OnceLock<Option<FontRef<'static>>> = OnceLock::new();

fn get_consolas_bold_bytes() -> Option<&'static [u8]> {
    CONSOLAS_BOLD_FONT
        .get_or_init(|| {
            std::fs::read("C:\\Windows\\Fonts\\consolasb.ttf")
                .or_else(|_| std::fs::read("C:\\Windows\\Fonts\\consolas.ttf"))
                .or_else(|_| std::fs::read("C:\\Windows\\Fonts\\segoeuib.ttf"))
                .ok()
        })
        .as_deref()
}

/// The parsed face, built once.
///
/// Parsing is the expensive half: this is called several times per frame (every label
/// drawn plus every `measure_*` behind the button layout), and it used to re-read the
/// font tables on each one.
fn consolas_bold() -> Option<&'static FontRef<'static>> {
    PARSED_FONT
        .get_or_init(|| FontRef::try_from_slice(get_consolas_bold_bytes()?).ok())
        .as_ref()
}

/// The software render target: the `0xRRGGBB` pixel buffer plus the dimensions that
/// describe it.
///
/// Every drawing routine used to take `(buffer, buf_w, buf_h)` as its first three
/// parameters and thread them through by hand. They are one thing — a pixel is at
/// `pixels[y * w + x]`, and that only holds if the three agree — so they travel together.
pub struct Canvas<'a> {
    pub pixels: &'a mut [u32],
    pub w: usize,
    pub h: usize,
}

impl<'a> Canvas<'a> {
    pub fn new(pixels: &'a mut [u32], w: usize, h: usize) -> Self {
        Self { pixels, w, h }
    }
}

/// Draw Consolas Bold Mono text with smooth subpixel antialiasing.
pub fn draw_consolas_bold_text(
    canvas: &mut Canvas,
    text: &str,
    start_x: usize,
    start_y: usize,
    color_rgb: u32,
    font_size_px: f32,
) {
    let max_x = canvas.w;
    draw_consolas_bold_text_clipped(canvas, text, start_x, start_y, max_x, color_rgb, font_size_px);
}

/// Draw Consolas Bold Mono text clipped to max_x boundary so letters appear smoothly as space opens.
pub fn draw_consolas_bold_text_clipped(
    canvas: &mut Canvas,
    text: &str,
    start_x: usize,
    start_y: usize,
    max_x: usize,
    color_rgb: u32,
    font_size_px: f32,
) {
    let Some(font) = consolas_bold() else {
        return;
    };

    let scale = PxScale::from(font_size_px);
    let scaled_font = font.as_scaled(scale);

    let text_r = ((color_rgb >> 16) & 0xFF) as f32;
    let text_g = ((color_rgb >> 8) & 0xFF) as f32;
    let text_b = (color_rgb & 0xFF) as f32;

    let mut cursor_x = start_x as f32;
    let baseline_y = start_y as f32 + scaled_font.ascent() - 2.0;

    for ch in text.chars() {
        if cursor_x + 2.0 > max_x as f32 {
            break;
        }

        let glyph_id = font.glyph_id(ch);
        let glyph = glyph_id.with_scale_and_position(scale, ab_glyph::point(cursor_x, baseline_y));
        cursor_x += scaled_font.h_advance(glyph_id);

        if let Some(outlined) = font.outline_glyph(glyph) {
            let bounds = outlined.px_bounds();
            outlined.draw(|x, y, coverage| {
                let px = (bounds.min.x as i32 + x as i32) as usize;
                let py = (bounds.min.y as i32 + y as i32) as usize;

                if px < canvas.w && px < max_x && py < canvas.h {
                    let idx = py * canvas.w + px;
                    let existing = canvas.pixels[idx];

                    let bg_r = ((existing >> 16) & 0xFF) as f32;
                    let bg_g = ((existing >> 8) & 0xFF) as f32;
                    let bg_b = (existing & 0xFF) as f32;

                    let alpha = coverage.clamp(0.0, 1.0);
                    let out_r = (text_r * alpha + bg_r * (1.0 - alpha)) as u32;
                    let out_g = (text_g * alpha + bg_g * (1.0 - alpha)) as u32;
                    let out_b = (text_b * alpha + bg_b * (1.0 - alpha)) as u32;

                    canvas.pixels[idx] = (out_r << 16) | (out_g << 8) | out_b;
                }
            });
        }
    }
}

/// Calculate exact pixel width of Consolas Bold Mono text.
pub fn measure_consolas_bold_width(text: &str, font_size_px: f32) -> usize {
    let Some(font) = consolas_bold() else {
        // No font: fall back to the average advance of a monospace face.
        return (text.chars().count() as f32 * (font_size_px * 0.6)) as usize;
    };

    let scale = PxScale::from(font_size_px);
    let scaled_font = font.as_scaled(scale);

    let mut width = 0.0f32;
    for ch in text.chars() {
        let glyph_id = font.glyph_id(ch);
        width += scaled_font.h_advance(glyph_id);
    }
    width.ceil() as usize
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconType {
    Clipboard,
    Save,
    Combo,
    Cancel,
}

// Official Open-Source Lucide Vector SVG Icons using double hash raw string literals r##"..."##
const SVG_COPY: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#FFFFFF" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><rect width="14" height="14" x="8" y="8" rx="2" ry="2"/><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"/></svg>"##;
const SVG_SAVE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#FFFFFF" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z"/><polyline points="17 21 17 13 7 13 7 21"/><polyline points="7 3 7 8 15 8"/></svg>"##;
const SVG_BOTH: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#FFFFFF" stroke-width="3.0" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"/></svg>"##;
const SVG_CANCEL: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#FFFFFF" stroke-width="2.8" stroke-linecap="round" stroke-linejoin="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>"##;

/// Icons already rasterized, keyed by type and pixel size.
///
/// The set is tiny and fixed (four icons, one or two sizes), so a linear scan beats a
/// hash map and keeps the dependency surface at zero.
type IconCache = Mutex<Vec<((IconType, u32), Vec<u8>)>>;
static ICON_CACHE: OnceLock<IconCache> = OnceLock::new();

/// Rasterizes one Lucide icon to `size * size` premultiplied RGBA.
fn rasterize_icon(icon_type: IconType, size: u32) -> Option<Vec<u8>> {
    let svg_str = match icon_type {
        IconType::Clipboard => SVG_COPY,
        IconType::Save => SVG_SAVE,
        IconType::Combo => SVG_BOTH,
        IconType::Cancel => SVG_CANCEL,
    };

    let opt = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_str(svg_str, &opt).ok()?;

    let pixmap_size = tree.size().to_int_size();
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size, size)?;

    let transform = resvg::tiny_skia::Transform::from_scale(
        size as f32 / pixmap_size.width() as f32,
        size as f32 / pixmap_size.height() as f32,
    );

    resvg::render(&tree, transform, &mut pixmap.as_mut());
    Some(pixmap.data().to_vec())
}

/// Composites a Lucide Open-Source SVG icon into the softbuffer.
///
/// Parsing the SVG and running resvg over it happened on every call, which meant four
/// XML parses and four vector rasterizations per frame for a set of icons that never
/// change. The result is cached instead, and only the blend below runs per frame.
pub fn draw_svg_icon(
    canvas: &mut Canvas,
    icon_type: IconType,
    start_x: usize,
    start_y: usize,
    target_size_px: u32,
) {
    if target_size_px == 0 {
        return;
    }

    let cache = ICON_CACHE.get_or_init(|| Mutex::new(Vec::new()));
    let Ok(mut cache) = cache.lock() else {
        return;
    };

    let key = (icon_type, target_size_px);
    if !cache.iter().any(|(k, _)| *k == key) {
        let Some(pixels) = rasterize_icon(icon_type, target_size_px) else {
            return;
        };
        cache.push((key, pixels));
    }
    let Some((_, data)) = cache.iter().find(|(k, _)| *k == key) else {
        return;
    };

    let size = target_size_px as usize;
    for y in 0..size {
        let py = start_y + y;
        if py >= canvas.h {
            break;
        }
        for x in 0..size {
            let px = start_x + x;
            if px >= canvas.w {
                break;
            }
            let idx = (y * size + x) * 4;
            let a = data[idx + 3] as f32 / 255.0;

            if a > 0.0 {
                let buf_idx = py * canvas.w + px;
                let bg = canvas.pixels[buf_idx];

                // `Pixmap` stores premultiplied alpha, so the colour channels already
                // carry the coverage factor: `src + dst * (1 - a)`, not `src * a + ...`,
                // which was applying it a second time and darkening antialiased edges.
                let out_r = (data[idx] as f32 + ((bg >> 16) & 0xFF) as f32 * (1.0 - a)) as u32;
                let out_g = (data[idx + 1] as f32 + ((bg >> 8) & 0xFF) as f32 * (1.0 - a)) as u32;
                let out_b = (data[idx + 2] as f32 + (bg & 0xFF) as f32 * (1.0 - a)) as u32;

                canvas.pixels[buf_idx] = (out_r.min(255) << 16) | (out_g.min(255) << 8) | out_b.min(255);
            }
        }
    }
}
