use anyhow::Result;
use image::RgbaImage;

#[path = "../clipboard.rs"]
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
