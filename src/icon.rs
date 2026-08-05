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
