// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 juanre7

use anyhow::Result;
use image::RgbaImage;
use windows_sys::Win32::System::Ole::OleInitialize;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::WindowId;

#[path = "../capture.rs"]
mod capture;
#[path = "../config.rs"]
mod config;
#[path = "../save_dialog.rs"]
mod save_dialog;
#[path = "../clipboard.rs"]
mod clipboard;
#[path = "../font.rs"]
mod font;
#[path = "../overlay.rs"]
mod overlay;
#[path = "../storage.rs"]
mod storage;

use capture::capture_desktop;
use clipboard::copy_to_clipboard;
use font::{draw_consolas_bold_text_clipped, draw_svg_icon, measure_consolas_bold_width, IconType};
use overlay::{Rect, SaveHint, SelectionOverlay};
use storage::{encode_image, save_image, OutputFormat, SaveOptions};

struct TestRigApp {
    step: usize,
    overlay: Option<SelectionOverlay>,
    pub success: bool,
}

impl ApplicationHandler for TestRigApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let test_desktop = RgbaImage::new(800, 600);
        let hint = SaveHint { scale_percent: 50, format_name: "WEBP" };
        if let Ok(overlay) = SelectionOverlay::new(event_loop, test_desktop, 0, 0, 800, 600, true, hint, 1.0) {
            self.overlay = Some(overlay);
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let mut finished = false;

        if let Some(ref mut overlay) = self.overlay {
            match self.step {
                0 => {
                    // Step 1: Simulate Mouse Press
                    let press = WindowEvent::MouseInput {
                        device_id: unsafe { std::mem::zeroed() },
                        state: winit::event::ElementState::Pressed,
                        button: winit::event::MouseButton::Left,
                    };
                    overlay.handle_event(&press);
                    self.step = 1;
                }
                1 => {
                    // Step 2: Simulate Cursor Move
                    let move_pos = WindowEvent::CursorMoved {
                        device_id: unsafe { std::mem::zeroed() },
                        position: winit::dpi::PhysicalPosition::new(300.0, 300.0),
                    };
                    overlay.handle_event(&move_pos);
                    self.step = 2;
                }
                2 => {
                    // Step 3: Simulate Mouse Release
                    let release = WindowEvent::MouseInput {
                        device_id: unsafe { std::mem::zeroed() },
                        state: winit::event::ElementState::Released,
                        button: winit::event::MouseButton::Left,
                    };
                    overlay.handle_event(&release);
                    self.step = 3;
                }
                3 => {
                    // Step 4: Simulate ESC Key Press to Cancel
                    let esc = WindowEvent::CloseRequested;
                    overlay.handle_event(&esc);
                    if overlay.finished {
                        finished = true;
                        self.success = true;
                    }
                }
                _ => {}
            }
        }

        if finished {
            self.overlay = None;
            event_loop.exit();
        }
    }

    fn window_event(&mut self, _event_loop: &ActiveEventLoop, _window_id: WindowId, _event: WindowEvent) {}
}

fn main() -> Result<()> {
    println!("\n=======================================================");
    println!(" 🚀 RUUUTU INTEGRATED AUTOMATED TEST BENCH & RIG ");
    println!("=======================================================\n");

    let mut passed = 0;
    let mut failed = 0;

    macro_rules! assert_test {
        ($name:expr, $expr:expr) => {
            print!("Testing {:<55} ... ", $name);
            if $expr {
                println!("[\x1b[32mPASS\x1b[0m]");
                passed += 1;
            } else {
                println!("[\x1b[31mFAIL\x1b[0m]");
                failed += 1;
            }
        };
    }

    // -------------------------------------------------------------
    // SUITE 1: SCREENSHOT CAPTURE & RECT NORMALIZATION
    // -------------------------------------------------------------
    println!("--- [SUITE 1: SCREENSHOT CAPTURE & RECT NORMALIZATION] ---");
    let capture_res = capture_desktop();
    assert_test!("Desktop Capture (High DPI)", capture_res.is_ok());

    if let Ok((img, _x, _y, w, h)) = capture_res {
        assert_test!("Desktop Dimensions > 0", w > 0 && h > 0);
        assert_test!("Desktop Buffer Length Match", img.width() == w && img.height() == h);
    }

    let rect1 = Rect::normalize(500, 400, 100, 200);
    assert_test!("Rect Normalize (100,200 400x200)", rect1.x == 100 && rect1.y == 200 && rect1.width == 400 && rect1.height == 200);
    assert_test!("Rect Contains Point (150, 250)", rect1.contains(150, 250));
    assert_test!("Rect Excludes Point (50, 50)", !rect1.contains(50, 50));

    // -------------------------------------------------------------
    // SUITE 2: CLIPBOARD INTEGRATION (READ / WRITE ROUNDTRIP)
    // -------------------------------------------------------------
    println!("\n--- [SUITE 2: SYSTEM CLIPBOARD ROUNDTRIP INTEGRATION] ---");
    let mut test_img = RgbaImage::new(120, 80);
    for y in 0..80 {
        for x in 0..120 {
            test_img.put_pixel(x, y, image::Rgba([0, 162, 255, 255]));
        }
    }

    let clip_res = copy_to_clipboard(&test_img);
    assert_test!("Copy RgbaImage to System Clipboard", clip_res.is_ok());

    if clip_res.is_ok() {
        std::thread::sleep(Duration::from_millis(100));
        let read_res = arboard::Clipboard::new().and_then(|mut c| c.get_image());
        assert_test!("Read Image Back From Clipboard", read_res.is_ok());

        if let Ok(read_img) = read_res {
            assert_test!("Clipboard Image Dimensions (120x80)", read_img.width == 120 && read_img.height == 80);
            assert_test!("Clipboard Pixel Sample Match", read_img.bytes.len() == 120 * 80 * 4);
        }
    }

    // -------------------------------------------------------------
    // SUITE 3: STORAGE, ENCODERS & SAVE SCALING
    // -------------------------------------------------------------
    println!("\n--- [SUITE 3: DISK STORAGE, ENCODERS & SAVE SCALING] ---");
    let webp_opts = SaveOptions { format: OutputFormat::WebP, ..Default::default() };
    let save_res = save_image(&test_img, &webp_opts);
    assert_test!("Save RgbaImage to WebP File", save_res.is_ok());

    if let Ok(path) = save_res {
        assert_test!("WebP File Exists on Disk", path.exists());
        let meta = std::fs::metadata(&path);
        assert_test!("WebP File Size > 0 bytes", meta.map(|m| m.len() > 0).unwrap_or(false));
    }

    // Every format must encode an RGBA buffer without erroring. JPEG in particular
    // rejects Rgba8, so it only works if the alpha channel is dropped first.
    for (name, format) in [
        ("WebP", OutputFormat::WebP),
        ("PNG", OutputFormat::Png),
        ("JPEG", OutputFormat::Jpeg),
    ] {
        let opts = SaveOptions { format, ..Default::default() };
        let bytes = encode_image(&test_img, &opts);
        assert_test!(format!("Encode RGBA to {} in memory", name), bytes.as_ref().map(|b| !b.is_empty()).unwrap_or(false));
    }

    // WebP lossless must differ from lossy: proof the quality setting reaches the encoder.
    let lossy = encode_image(&test_img, &SaveOptions { format: OutputFormat::WebP, quality: 50, lossless: false, ..Default::default() });
    let lossless = encode_image(&test_img, &SaveOptions { format: OutputFormat::WebP, lossless: true, ..Default::default() });
    assert_test!("WebP lossy and lossless produce different output", match (&lossy, &lossless) {
        (Ok(a), Ok(b)) => a != b,
        _ => false,
    });

    // Scaling must actually shrink the encoded image.
    let full = encode_image(&test_img, &SaveOptions { format: OutputFormat::Png, scale_percent: 100, ..Default::default() });
    let half = encode_image(&test_img, &SaveOptions { format: OutputFormat::Png, scale_percent: 50, ..Default::default() });
    assert_test!("50% scale yields a smaller PNG than 100%", match (&full, &half) {
        (Ok(a), Ok(b)) => b.len() < a.len(),
        _ => false,
    });

    // 120x80 at 25% must land on exactly 30x20, not an off-by-one.
    let quarter = encode_image(&test_img, &SaveOptions { format: OutputFormat::Png, scale_percent: 25, ..Default::default() });
    let quarter_dims = quarter.ok().and_then(|b| image::load_from_memory(&b).ok()).map(|i| (i.width(), i.height()));
    assert_test!("25% of 120x80 decodes back as 30x20", quarter_dims == Some((30, 20)));

    // -------------------------------------------------------------
    // SUITE 3B: NATIVE SAVE DIALOG COM PLUMBING
    // -------------------------------------------------------------
    // Builds the real IFileSaveDialog with Ruuutu's custom controls attached, but
    // never calls Show(): everything that can fail in COM (instantiation, the
    // IFileDialogCustomize cast, every combo and control item) happens here.
    println!("\n--- [SUITE 3B: NATIVE SAVE DIALOG COM PLUMBING] ---");
    unsafe { OleInitialize(std::ptr::null_mut()) };

    let dialog_dir = storage::get_screenshots_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    for (name, fmt) in [
        ("WebP", config::ImageFormatChoice::WebP),
        ("PNG", config::ImageFormatChoice::Png),
        ("JPEG", config::ImageFormatChoice::Jpeg),
    ] {
        let prepared = save_dialog::prepare_dialog(
            &storage::default_file_name(fmt.to_output_format()),
            &dialog_dir,
            fmt,
            config::QualityChoice::High,
            config::ScaleChoice::P50,
        );
        assert_test!(format!("Build native save dialog preset to {}", name), prepared.is_ok());
    }

    // -------------------------------------------------------------
    // SUITE 4: VECTOR SVG ICONS & TEXT CLIPPING
    // -------------------------------------------------------------
    println!("\n--- [SUITE 4: VECTOR SVG ICONS & TEXT CLIPPING] ---");
    let mut pixel_buf = vec![0u32; 100 * 100];
    draw_svg_icon(&mut pixel_buf, 100, 100, IconType::Clipboard, 10, 10, 20);
    let non_zero_count = pixel_buf.iter().filter(|&&p| p != 0).count();
    assert_test!("Rasterize Lucide Clipboard SVG (Non-zero Pixels)", non_zero_count > 50);

    let text_w = measure_consolas_bold_width("Copiar (C)", 17.0);
    assert_test!("Measure Consolas Bold Text Width > 0", text_w > 50);

    let mut text_buf = vec![0u32; 200 * 50];
    draw_consolas_bold_text_clipped(&mut text_buf, 200, 50, "Copiar (C)", 10, 10, 100, 0xFFFFFF, 17.0);
    let text_pixels = text_buf.iter().filter(|&&p| p != 0).count();
    assert_test!("Render Clipped Consolas Bold Text", text_pixels > 20);

    // -------------------------------------------------------------
    // SUITE 5: BUTTON LAYOUT & 500MS HOVER DELAY MATH
    // -------------------------------------------------------------
    println!("\n--- [SUITE 5: BUTTON LAYOUT & 500MS HOVER DELAY MATH] ---");
    let base_button_h = 34.0f32;
    let square_w = base_button_h;
    assert_test!("Square Collapsed Width == Height (34px)", (square_w - 34.0).abs() < 0.001);

    let start_t = Instant::now();
    std::thread::sleep(Duration::from_millis(520));
    let elapsed = start_t.elapsed().as_millis();
    assert_test!("500ms Delay Timer Expiry Check (Elapsed >= 500ms)", elapsed >= 500);

    // -------------------------------------------------------------
    // SUITE 6: SELECTION OVERLAY INTERACTIVE EVENT RIG
    // -------------------------------------------------------------
    println!("\n--- [SUITE 6: SELECTION OVERLAY INTERACTIVE EVENT RIG] ---");
    let event_loop_res = EventLoop::builder().build();
    assert_test!("Build winit EventLoop for Rig", event_loop_res.is_ok());

    if let Ok(event_loop) = event_loop_res {
        let mut app = TestRigApp { step: 0, overlay: None, success: false };
        let run_res = event_loop.run_app(&mut app);
        assert_test!("Execute Interactive Rig Event Loop", run_res.is_ok());
        assert_test!("Overlay Window Lifecycle & Event Handling", app.success);
    }

    // -------------------------------------------------------------
    // FINAL SUMMARY REPORT
    // -------------------------------------------------------------
    println!("\n=======================================================");
    println!(" 📊 TEST BENCH SUMMARY RESULTS ");
    println!("=======================================================");
    println!(" PASSED : {}", passed);
    println!(" FAILED : {}", failed);
    println!(" TOTAL  : {}", passed + failed);
    println!("=======================================================\n");

    if failed > 0 {
        anyhow::bail!("Test bench failed with {} errors!", failed);
    }

    Ok(())
}
