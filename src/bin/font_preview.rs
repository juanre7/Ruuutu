use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use anyhow::Result;
use softbuffer::{Context as SbContext, Surface};
use std::num::NonZeroU32;
use std::sync::Arc;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, Event, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::WindowAttributes;

// Import pixel fonts from main crate font module
#[path = "../font.rs"]
mod font;
use font::draw_consolas_bold_text;

struct FontOption {
    id: usize,
    name: &'static str,
    file_name: Option<&'static str>,
}

fn main() -> Result<()> {
    println!("Starting Ruuutu Typography Preview Mode...");

    let options = [
        FontOption { id: 3,  name: "3. Segoe UI Regular (Nativa Windows)", file_name: Some("segoeui.ttf") },
        FontOption { id: 4,  name: "4. Segoe UI Bold",                     file_name: Some("segoeuib.ttf") },
        FontOption { id: 5,  name: "5. Segoe UI Semibold / Light",         file_name: Some("segoeuisl.ttf") },
        FontOption { id: 6,  name: "6. Consolas Developer Mono",           file_name: Some("consolas.ttf") },
        FontOption { id: 7,  name: "7. Consolas Bold Mono",                file_name: Some("consolasb.ttf") },
        FontOption { id: 8,  name: "8. Lucida Console System",             file_name: Some("lucon.ttf") },
        FontOption { id: 9,  name: "9. Tahoma Regular UI",                 file_name: Some("tahoma.ttf") },
        FontOption { id: 10, name: "10. Tahoma Bold UI",                   file_name: Some("tahomabd.ttf") },
        FontOption { id: 11, name: "11. Verdana High-Legibility",         file_name: Some("verdana.ttf") },
        FontOption { id: 12, name: "12. Verdana Bold",                     file_name: Some("verdanab.ttf") },
        FontOption { id: 13, name: "13. Arial Universal Sans",             file_name: Some("arial.ttf") },
        FontOption { id: 14, name: "14. Arial Bold",                       file_name: Some("arialbd.ttf") },
        FontOption { id: 15, name: "15. Trebuchet MS Clean",               file_name: Some("trebuc.ttf") },
        FontOption { id: 16, name: "16. Calibri Modern Sans",              file_name: Some("calibri.ttf") },
    ];

    let width = 1200u32;
    let height = 960u32;

    let event_loop = EventLoop::new()?;
    let attrs = WindowAttributes::default()
        .with_title("Ruuutu - Demostrador de 16 Tipografías (Elige tu favorita)")
        .with_inner_size(PhysicalSize::new(width, height))
        .with_resizable(false);

    #[allow(deprecated)]
    let window = Arc::new(event_loop.create_window(attrs)?);
    let sb_context = SbContext::new(window.clone()).map_err(|e| anyhow::anyhow!("{:?}", e))?;
    let mut surface = Surface::new(&sb_context, window.clone()).map_err(|e| anyhow::anyhow!("{:?}", e))?;

    surface.resize(NonZeroU32::new(width).unwrap(), NonZeroU32::new(height).unwrap()).unwrap();

    println!("\n========================================================");
    println!("  RUUTU TYPOGRAPHY SHOWCASE (16 TIPOGRAFÍAS EN PANTALLA)");
    println!("========================================================");
    println!("Revisa la ventana en pantalla con los 16 estilos numerados del 1 al 16.");
    println!("Dime por aquí con qué número (del 1 al 16) quieres que nos quedemos.\n");

    #[allow(deprecated)]
    event_loop.run(move |event, elwh: &ActiveEventLoop| {
        elwh.set_control_flow(ControlFlow::Wait);

        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => elwh.exit(),
                WindowEvent::KeyboardInput {
                    event: KeyEvent { logical_key, state: ElementState::Pressed, .. },
                    ..
                } => {
                    if logical_key == Key::Named(NamedKey::Escape) {
                        elwh.exit();
                    }
                }
                WindowEvent::RedrawRequested => {
                    let mut buffer = surface.buffer_mut().unwrap();
                    let buf_w = width as usize;
                    let buf_h = height as usize;

                    // Dark theme background #0F172A
                    draw_filled_rect(&mut buffer, buf_w, buf_h, 0, 0, buf_w, buf_h, 0x0F172A);

                    // Header title
                    let header_title = "RUUTU - DEMOSTRADOR DE 16 TIPOGRAMAS (ELIGE TU NUMERO DEL 1 AL 16)";
                    draw_consolas_bold_text(&mut buffer, buf_w, buf_h, header_title, 20, 16, 0x38BDF8, 15.0);

                    let start_y = 50;
                    let row_h = 55;

                    for (idx, opt) in options.iter().enumerate() {
                        let y = start_y + idx * row_h;
                        if y + 45 > buf_h {
                            break;
                        }

                        // Row background card #1E293B
                        draw_filled_rect(&mut buffer, buf_w, buf_h, 15, y, buf_w - 30, 48, 0x1E293B);

                        // Option label (Title)
                        draw_consolas_bold_text(&mut buffer, buf_w, buf_h, opt.name, 25, y + 15, 0xE2E8F0, 15.0);

                        // Sample UI Elements rendered in target font!
                        let btn_x_start = 420;

                        // Render Sample Button 1: Copiar (C) - Green
                        draw_filled_rect(&mut buffer, buf_w, buf_h, btn_x_start, y + 8, 120, 32, 0x16A34A);
                        render_sample_text(&mut buffer, buf_w, buf_h, "Copiar (C)", btn_x_start + 10, y + 15, 0xFFFFFF, opt);

                        // Render Sample Button 2: Guardar (S) - Blue
                        draw_filled_rect(&mut buffer, buf_w, buf_h, btn_x_start + 130, y + 8, 120, 32, 0x2563EB);
                        render_sample_text(&mut buffer, buf_w, buf_h, "Guardar (S)", btn_x_start + 140, y + 15, 0xFFFFFF, opt);

                        // Render Sample Button 3: Ambos (Enter) - Cyan
                        draw_filled_rect(&mut buffer, buf_w, buf_h, btn_x_start + 260, y + 8, 140, 32, 0x0284C7);
                        render_sample_text(&mut buffer, buf_w, buf_h, "Ambos (Enter)", btn_x_start + 270, y + 15, 0xFFFFFF, opt);

                        // Render Sample Dimension Tag: 1920 x 1080 px
                        draw_filled_rect(&mut buffer, buf_w, buf_h, btn_x_start + 410, y + 8, 150, 32, 0x334155);
                        render_sample_text(&mut buffer, buf_w, buf_h, "1920 x 1080 px", btn_x_start + 420, y + 15, 0x38BDF8, opt);
                    }

                    buffer.present().unwrap();
                }
                _ => {}
            },
            _ => {}
        }
    })?;

    Ok(())
}

fn render_sample_text(
    buffer: &mut [u32],
    buf_w: usize,
    buf_h: usize,
    text: &str,
    x: usize,
    y: usize,
    color: u32,
    opt: &FontOption,
) {
    if let Some(file_name) = opt.file_name {
        let font_path = format!("C:\\Windows\\Fonts\\{}", file_name);
        if let Ok(bytes) = std::fs::read(&font_path) {
            if let Ok(font) = FontRef::try_from_slice(&bytes) {
                let scale = PxScale::from(14.0);
                let scaled = font.as_scaled(scale);
                let mut cursor_x = x as f32;
                let baseline_y = y as f32 + scaled.ascent() - 2.0;

                let r = ((color >> 16) & 0xFF) as f32;
                let g = ((color >> 8) & 0xFF) as f32;
                let b = (color & 0xFF) as f32;

                for ch in text.chars() {
                    let glyph_id = font.glyph_id(ch);
                    let glyph = glyph_id.with_scale_and_position(scale, ab_glyph::point(cursor_x, baseline_y));
                    cursor_x += scaled.h_advance(glyph_id);

                    if let Some(outlined) = font.outline_glyph(glyph) {
                        let bounds = outlined.px_bounds();
                        outlined.draw(|gx, gy, coverage| {
                            let px = (bounds.min.x as i32 + gx as i32) as usize;
                            let py = (bounds.min.y as i32 + gy as i32) as usize;

                            if px < buf_w && py < buf_h {
                                let idx = py * buf_w + px;
                                let bg = buffer[idx];

                                let bg_r = ((bg >> 16) & 0xFF) as f32;
                                let bg_g = ((bg >> 8) & 0xFF) as f32;
                                let bg_b = (bg & 0xFF) as f32;

                                let alpha = coverage.clamp(0.0, 1.0);
                                let out_r = (r * alpha + bg_r * (1.0 - alpha)) as u32;
                                let out_g = (g * alpha + bg_g * (1.0 - alpha)) as u32;
                                let out_b = (b * alpha + bg_b * (1.0 - alpha)) as u32;

                                buffer[idx] = (out_r << 16) | (out_g << 8) | out_b;
                            }
                        });
                    }
                }
                return;
            }
        }
    }

    // Fallback: the font file was not present on this machine.
    draw_consolas_bold_text(buffer, buf_w, buf_h, text, x, y, color, 15.0);
}

fn draw_filled_rect(buffer: &mut [u32], buf_w: usize, buf_h: usize, x: usize, y: usize, w: usize, h: usize, color: u32) {
    let x2 = (x + w).min(buf_w);
    let y2 = (y + h).min(buf_h);
    for py in y..y2 {
        for px in x..x2 {
            buffer[py * buf_w + px] = color;
        }
    }
}
