// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 juanre7

//! Procedural Ruuutu icon, drawn in code at any square size.
//!
//! Single source of truth for both the tray icon (rasterized at runtime by `tray.rs`) and the
//! executable icon (rasterized at build time by `build.rs`, which pulls this file in with
//! `#[path]`). Because `build.rs` includes it directly, this module must stay on `std` only —
//! no crate dependencies, no `use` of anything from `ruuutu`.

/// Ruuutu accent blue, used for the outer frame and the viewfinder lines.
const ACCENT: [u8; 4] = [0, 162, 255, 255];
/// Slate dark background of the inner square.
const FILL: [u8; 4] = [30, 41, 59, 230];
const TRANSPARENT: [u8; 4] = [0, 0, 0, 0];

/// Render the icon as `size * size` RGBA8 pixels, row-major, top-down.
///
/// The 32×32 layout is the reference: a 1 px outer frame, and viewfinder lines at 4 px from each
/// edge that run the full width/height of the image, enclosing the dark square. Every measurement
/// below is that design expressed as a fraction of `size`, so 16, 48 or 256 keep the proportions.
pub fn icon_rgba(size: u32) -> Vec<u8> {
    // Line thickness: 1 px at 32×32, growing with the icon so it stays visible when scaled up.
    let t = size.div_ceil(32);
    // Distance from the edge to the viewfinder lines: 4 px at 32×32.
    let inset = size / 8;

    let mut rgba = Vec::with_capacity((size * size * 4) as usize);

    for y in 0..size {
        for x in 0..size {
            let on_frame = x < t || x >= size - t || y < t || y >= size - t;
            let on_line = |v: u32| (v >= inset && v < inset + t) || (v >= size - inset - t && v < size - inset);
            let inside = |v: u32| v >= inset + t && v < size - inset - t;

            let px = if on_frame || on_line(x) || on_line(y) {
                ACCENT
            } else if inside(x) && inside(y) {
                FILL
            } else {
                TRANSPARENT
            };

            rgba.extend_from_slice(&px);
        }
    }

    rgba
}

// Like the rest of this module, the tests stay on `std`: `build.rs` pulls the file in with
// `#[path]`, and anything else here would break the build script.
#[cfg(test)]
mod tests {
    use super::*;

    /// The sizes `build.rs` packs into the `.ico`, plus the 32×32 the tray asks for.
    const SIZES: [u32; 7] = [16, 24, 32, 48, 64, 128, 256];

    /// Reads the pixel at `(x, y)` out of the row-major RGBA buffer.
    fn px(rgba: &[u8], size: u32, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * size + x) * 4) as usize;
        [rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]]
    }

    #[test]
    fn every_size_fills_exactly_one_rgba_buffer() {
        for size in SIZES {
            assert_eq!(
                icon_rgba(size).len(),
                (size * size * 4) as usize,
                "size {size} produced a buffer that is not size*size*4"
            );
        }
    }

    /// Only the three palette entries may appear. A stray colour would mean the
    /// classification below fell through to something unintended.
    #[test]
    fn only_the_three_palette_colours_appear() {
        for size in SIZES {
            let rgba = icon_rgba(size);
            for chunk in rgba.chunks_exact(4) {
                let c = [chunk[0], chunk[1], chunk[2], chunk[3]];
                assert!(
                    c == ACCENT || c == FILL || c == TRANSPARENT,
                    "size {size} produced the unexpected colour {c:?}"
                );
            }
        }
    }

    /// The frame runs along all four edges and the dark square sits in the middle.
    /// This is the shape the icon is recognised by, at every size it is asked for.
    #[test]
    fn the_frame_wraps_the_edges_and_the_fill_holds_the_centre() {
        for size in SIZES {
            let rgba = icon_rgba(size);
            let last = size - 1;
            let mid = size / 2;

            for (x, y) in [(0, 0), (last, 0), (0, last), (last, last), (mid, 0), (0, mid)] {
                assert_eq!(px(&rgba, size, x, y), ACCENT, "size {size} has no frame at ({x}, {y})");
            }
            assert_eq!(px(&rgba, size, mid, mid), FILL, "size {size} has no fill at its centre");
        }
    }

    /// The four corner pockets — outside the viewfinder lines but inside the frame —
    /// stay clear. If they filled in, the icon would read as a solid block.
    #[test]
    fn the_corner_pockets_stay_transparent() {
        for size in SIZES {
            let rgba = icon_rgba(size);
            let t = size.div_ceil(32);
            let inset = size / 8;
            // Halfway between the frame and the viewfinder line: inside the pocket at
            // every size, given `inset` is 4x the thickness from 32×32 upwards.
            let p = t + (inset - t) / 2;
            assert_eq!(
                px(&rgba, size, p, p),
                TRANSPARENT,
                "size {size} filled the corner pocket at ({p}, {p})"
            );
        }
    }

    /// The viewfinder lines are what make it a viewfinder: they must cross the full
    /// width and height, not stop at the dark square.
    #[test]
    fn the_viewfinder_lines_run_edge_to_edge() {
        let size = 32;
        let rgba = icon_rgba(size);
        let inset = size / 8;
        for v in 0..size {
            assert_eq!(px(&rgba, size, v, inset), ACCENT, "horizontal line breaks at x={v}");
            assert_eq!(px(&rgba, size, inset, v), ACCENT, "vertical line breaks at y={v}");
        }
    }

    /// The design is symmetric, so the buffer must be too. Catches an off-by-one in the
    /// `size - inset - t` half of the bounds, which the centre and edge checks would miss.
    #[test]
    fn the_icon_is_symmetric_on_both_axes() {
        for size in SIZES {
            let rgba = icon_rgba(size);
            for y in 0..size {
                for x in 0..size {
                    let p = px(&rgba, size, x, y);
                    assert_eq!(p, px(&rgba, size, size - 1 - x, y), "size {size} breaks left/right at ({x}, {y})");
                    assert_eq!(p, px(&rgba, size, x, size - 1 - y), "size {size} breaks top/bottom at ({x}, {y})");
                }
            }
        }
    }

    /// A 1 px icon has no room for a frame, a line and a square, but it must still hand
    /// back a well-formed buffer instead of panicking on the `size - inset - t` subtraction.
    #[test]
    fn the_smallest_sizes_do_not_underflow() {
        for size in 1..=8 {
            assert_eq!(icon_rgba(size).len(), (size * size * 4) as usize);
        }
    }
}
