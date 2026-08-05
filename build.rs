// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 juanre7

//! Embeds the Ruuutu icon and the version metadata into `ruuutu.exe`.
//!
//! The icon is not an asset on disk: it is drawn by `src/icon.rs`, the very same code the tray
//! uses at runtime, so the shell icon and the tray icon can never drift apart. This script
//! rasterizes it at the sizes Windows asks for, packs them into an `.ico` under `OUT_DIR`, and
//! hands a generated `.rc` to `embed-resource`.
//!
//! The same `.rc` carries a `VERSIONINFO` block, which is what fills in the Details tab of the
//! file's properties in Explorer and what installers and IT tooling read to identify a build.
//! Its numbers come from `CARGO_PKG_VERSION`, so bumping the version in `Cargo.toml` is enough.

#[path = "src/icon.rs"]
mod icon;

use image::ImageEncoder;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

/// Sizes Windows picks from: tray/titlebar (16), list views (24, 32), tiles (48, 64) and the
/// extra-large / jumbo views (128, 256).
const SIZES: &[u32] = &[16, 24, 32, 48, 64, 128, 256];

/// Above this size the entries are stored PNG-compressed instead of as raw DIBs — a 256×256 BMP
/// entry alone would be 256 KB. Windows Vista and later read PNG entries natively.
const PNG_FROM: u32 = 128;

fn main() {
    println!("cargo:rerun-if-changed=src/icon.rs");
    println!("cargo:rerun-if-changed=build.rs");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));

    let ico_path = out_dir.join("ruuutu.ico");
    fs::write(&ico_path, build_ico()).expect("write ruuutu.ico");

    // rc.exe wants the path escaped like a C string.
    let rc_path = out_dir.join("ruuutu.rc");
    let mut rc = fs::File::create(&rc_path).expect("create ruuutu.rc");
    writeln!(rc, "1 ICON \"{}\"", ico_path.display().to_string().replace('\\', "\\\\")).expect("write ruuutu.rc");
    write!(rc, "{}", version_info()).expect("write ruuutu.rc");
    drop(rc);

    embed_resource::compile(&rc_path, embed_resource::NONE)
        .manifest_required()
        .expect("embed icon resource");
}

/// The `VERSIONINFO` resource, built from `CARGO_PKG_VERSION`.
///
/// The binary numbers are four 16-bit fields, so the crate version is padded with a trailing
/// zero for the unused build component. A non-numeric suffix such as `1.0.1-rc1` has no place
/// in them and is simply dropped from the numeric fields; the readable strings keep it.
///
/// Every string here is deliberately ASCII. Which compiler `embed-resource` ends up calling
/// (`rc.exe` or `windres`) and which code page it assumes for a file without a BOM is not
/// something this script controls, and a mangled accent in the properties dialog is a poor
/// trade for the two characters it would buy.
fn version_info() -> String {
    let version = std::env::var("CARGO_PKG_VERSION").expect("CARGO_PKG_VERSION");

    let mut parts = version
        .split(['.', '-', '+'])
        .map(|p| p.parse::<u16>().unwrap_or(0));
    let (major, minor, patch) = (
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
    );

    format!(
        r#"
1 VERSIONINFO
FILEVERSION {major},{minor},{patch},0
PRODUCTVERSION {major},{minor},{patch},0
FILEOS 0x40004L
FILETYPE 0x1L
BEGIN
    BLOCK "StringFileInfo"
    BEGIN
        BLOCK "040904B0"
        BEGIN
            VALUE "CompanyName", "juanre7"
            VALUE "FileDescription", "Ruuutu - Captura de pantalla para Windows"
            VALUE "FileVersion", "{version}"
            VALUE "InternalName", "ruuutu"
            VALUE "LegalCopyright", "Copyright (C) 2026 juanre7 - GPL-3.0-or-later"
            VALUE "OriginalFilename", "ruuutu.exe"
            VALUE "ProductName", "Ruuutu"
            VALUE "ProductVersion", "{version}"
        END
    END
    BLOCK "VarFileInfo"
    BEGIN
        VALUE "Translation", 0x409, 1200
    END
END
"#
    )
}

/// Pack every size into an ICO container: 6-byte header, one 16-byte directory entry per image,
/// then the image payloads back to back.
fn build_ico() -> Vec<u8> {
    let images: Vec<(u32, Vec<u8>)> = SIZES
        .iter()
        .map(|&size| {
            let rgba = icon::icon_rgba(size);
            let payload = if size >= PNG_FROM {
                encode_png(&rgba, size)
            } else {
                encode_dib(&rgba, size)
            };
            (size, payload)
        })
        .collect();

    let mut out = Vec::new();
    out.extend_from_slice(&0u16.to_le_bytes()); // reserved
    out.extend_from_slice(&1u16.to_le_bytes()); // type: icon
    out.extend_from_slice(&(images.len() as u16).to_le_bytes());

    // Payloads start right after the directory.
    let mut offset = 6 + 16 * images.len() as u32;
    for (size, payload) in &images {
        // 256 is encoded as 0 in the single-byte width/height fields.
        let dim = if *size >= 256 { 0u8 } else { *size as u8 };
        out.push(dim);
        out.push(dim);
        out.push(0); // palette entries (0 = truecolor)
        out.push(0); // reserved
        out.extend_from_slice(&1u16.to_le_bytes()); // color planes
        out.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
        out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
        offset += payload.len() as u32;
    }

    for (_, payload) in &images {
        out.extend_from_slice(payload);
    }

    out
}

/// A classic icon image: BITMAPINFOHEADER, BGRA pixels bottom-up, then the 1-bpp AND mask.
///
/// The header lies about the height on purpose — the format requires `2 * height`, counting the
/// mask as if it were a second bitmap stacked below. The mask itself is left all-zero (fully
/// opaque); transparency comes from the alpha channel, but the rows must still be there or the
/// shell reads the payload as truncated.
fn encode_dib(rgba: &[u8], size: u32) -> Vec<u8> {
    let mask_stride = (size.div_ceil(32) * 4) as usize;
    let mask_len = mask_stride * size as usize;
    let pixels_len = (size * size * 4) as usize;

    let mut out = Vec::with_capacity(40 + pixels_len + mask_len);

    out.extend_from_slice(&40u32.to_le_bytes()); // biSize
    out.extend_from_slice(&(size as i32).to_le_bytes()); // biWidth
    out.extend_from_slice(&((size * 2) as i32).to_le_bytes()); // biHeight (image + mask)
    out.extend_from_slice(&1u16.to_le_bytes()); // biPlanes
    out.extend_from_slice(&32u16.to_le_bytes()); // biBitCount
    out.extend_from_slice(&0u32.to_le_bytes()); // biCompression = BI_RGB
    out.extend_from_slice(&((pixels_len + mask_len) as u32).to_le_bytes()); // biSizeImage
    out.extend_from_slice(&0i32.to_le_bytes()); // biXPelsPerMeter
    out.extend_from_slice(&0i32.to_le_bytes()); // biYPelsPerMeter
    out.extend_from_slice(&0u32.to_le_bytes()); // biClrUsed
    out.extend_from_slice(&0u32.to_le_bytes()); // biClrImportant

    for y in (0..size).rev() {
        for x in 0..size {
            let i = ((y * size + x) * 4) as usize;
            out.extend_from_slice(&[rgba[i + 2], rgba[i + 1], rgba[i], rgba[i + 3]]);
        }
    }

    out.resize(out.len() + mask_len, 0);

    out
}

fn encode_png(rgba: &[u8], size: u32) -> Vec<u8> {
    let mut out = std::io::Cursor::new(Vec::new());
    image::codecs::png::PngEncoder::new(&mut out)
        .write_image(rgba, size, size, image::ExtendedColorType::Rgba8)
        .expect("encode icon png");
    out.into_inner()
}
