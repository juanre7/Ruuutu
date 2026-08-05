use anyhow::{Context, Result};
use softbuffer::{Context as SbContext, Surface};
use std::num::NonZeroU32;
use std::sync::Arc;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, Event, KeyEvent, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::WindowAttributes;

#[path = "../font.rs"]
mod font;

use font::{draw_consolas_bold_text, draw_svg_icon, measure_consolas_bold_width, IconType};

#[derive(Debug, Clone)]
struct MarginPreset {
    id: usize,
    name: &'static str,
    pad_left: usize,
    icon_gap: usize,
    pad_right: usize,
    pad_vert: usize,
    btn_spacing: usize,
}

fn get_presets() -> Vec<MarginPreset> {
    vec![
        MarginPreset { id: 1,  name: "Ultra Compacto",      pad_left: 4,  icon_gap: 4, pad_right: 4,  pad_vert: 4, btn_spacing: 4 },
        MarginPreset { id: 2,  name: "Compacto Simétrico",  pad_left: 6,  icon_gap: 4, pad_right: 6,  pad_vert: 5, btn_spacing: 4 },
        MarginPreset { id: 3,  name: "Ajustado Moderno",    pad_left: 6,  icon_gap: 6, pad_right: 6,  pad_vert: 6, btn_spacing: 6 },
        MarginPreset { id: 4,  name: "Cuadrado Estilizado", pad_left: 8,  icon_gap: 4, pad_right: 8,  pad_vert: 6, btn_spacing: 6 },
        MarginPreset { id: 5,  name: "Clásico Equilibrado", pad_left: 8,  icon_gap: 6, pad_right: 8,  pad_vert: 6, btn_spacing: 6 },
        MarginPreset { id: 6,  name: "Espaciado Medio",     pad_left: 8,  icon_gap: 8, pad_right: 8,  pad_vert: 7, btn_spacing: 6 },
        MarginPreset { id: 7,  name: "Aireado Ligero",      pad_left: 10, icon_gap: 6, pad_right: 10, pad_vert: 6, btn_spacing: 8 },
        MarginPreset { id: 8,  name: "Simétrico Estándar",  pad_left: 10, icon_gap: 8, pad_right: 10, pad_vert: 8, btn_spacing: 8 },
        MarginPreset { id: 9,  name: "Cápsula Holgada",     pad_left: 12, icon_gap: 6, pad_right: 12, pad_vert: 6, btn_spacing: 8 },
        MarginPreset { id: 10, name: "Horizontal Ancho",    pad_left: 12, icon_gap: 8, pad_right: 12, pad_vert: 7, btn_spacing: 8 },
        MarginPreset { id: 11, name: "Espaciado Generoso",  pad_left: 12, icon_gap: 10,pad_right: 12, pad_vert: 8, btn_spacing: 10 },
        MarginPreset { id: 12, name: "Vertical Alto",       pad_left: 8,  icon_gap: 6, pad_right: 8,  pad_vert: 10,btn_spacing: 6 },
        MarginPreset { id: 13, name: "Amplio Moderno",      pad_left: 14, icon_gap: 8, pad_right: 14, pad_vert: 8, btn_spacing: 8 },
        MarginPreset { id: 14, name: "Separado Grupal",     pad_left: 10, icon_gap: 8, pad_right: 10, pad_vert: 8, btn_spacing: 12 },
        MarginPreset { id: 15, name: "Maxi Horizontal",     pad_left: 16, icon_gap: 8, pad_right: 16, pad_vert: 8, btn_spacing: 8 },
        MarginPreset { id: 16, name: "Ultra Holgado",       pad_left: 16, icon_gap: 10,pad_right: 16, pad_vert: 10,btn_spacing: 10 },
    ]
}

fn main() -> Result<()> {
    let presets = get_presets();
    let width = 1200u32;
    let height = 960u32;

    let event_loop = EventLoop::new().context("Failed to create event loop")?;
    let attrs = WindowAttributes::default()
        .with_title("Ruuutu - Demostrador de 16 Combinaciones de Márgenes")
        .with_inner_size(PhysicalSize::new(width, height))
        .with_position(PhysicalPosition::new(100, 50));

    #[allow(deprecated)]
    let window = Arc::new(event_loop.create_window(attrs).context("Failed to create window")?);
    let context = SbContext::new(window.clone()).map_err(|e| anyhow::anyhow!("Context error: {:?}", e))?;
    let mut surface = Surface::new(&context, window.clone()).map_err(|e| anyhow::anyhow!("Surface error: {:?}", e))?;

    surface
        .resize(NonZeroU32::new(width).unwrap(), NonZeroU32::new(height).unwrap())
        .unwrap();

    let mut selected_id = 5usize; // Default: Opción 5
    let font_size = 17.0f32;
    let icon_size = 20u32;

    #[allow(deprecated)]
    event_loop.run(move |event, elwh: &ActiveEventLoop| {
        elwh.set_control_flow(ControlFlow::Wait);

        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => elwh.exit(),
                WindowEvent::KeyboardInput {
                    event:
                        KeyEvent {
                            logical_key,
                            state: ElementState::Pressed,
                            ..
                        },
                    ..
                } => match logical_key {
                    Key::Named(NamedKey::Escape) => elwh.exit(),
                    Key::Named(NamedKey::ArrowUp) => {
                        if selected_id > 1 {
                            selected_id -= 1;
                            window.request_redraw();
                        }
                    }
                    Key::Named(NamedKey::ArrowDown) => {
                        if selected_id < presets.len() {
                            selected_id += 1;
                            window.request_redraw();
                        }
                    }
                    _ => {}
                },
                WindowEvent::MouseInput {
                    state: ElementState::Pressed,
                    button: MouseButton::Left,
                    ..
                } => {
                    // Check click location
                    window.request_redraw();
                }
                WindowEvent::RedrawRequested => {
                    let mut buffer = surface.buffer_mut().unwrap();
                    let buf_w = width as usize;
                    let buf_h = height as usize;

                    // Fill background
                    fill_rect(&mut buffer, buf_w, buf_h, 0, 0, buf_w, buf_h, 0x0F172A);

                    // Header Title
                    draw_consolas_bold_text(
                        &mut buffer, buf_w, buf_h,
                        "RUUUTU - COMPARA LAS 16 COMBINACIONES DE MÁRGENES",
                        24, 20, 0x38BDF8, 20.0,
                    );
                    draw_consolas_bold_text(
                        &mut buffer, buf_w, buf_h,
                        "Usa las flechas arriba/abajo o haz clic para seleccionar la opción que prefieras",
                        24, 48, 0x94A3B8, 14.0,
                    );

                    let card_h = 50usize;
                    let start_y = 80usize;

                    for (idx, preset) in presets.iter().enumerate() {
                        let y = start_y + idx * (card_h + 3);
                        if y + card_h > buf_h {
                            break;
                        }

                        let is_selected = preset.id == selected_id;
                        let card_bg = if is_selected { 0x1E293B } else { 0x1E293B / 2 };
                        let border_color = if is_selected { 0x38BDF8 } else { 0x334155 };

                        fill_rect(&mut buffer, buf_w, buf_h, 20, y, buf_w - 40, card_h, card_bg);
                        draw_border(&mut buffer, buf_w, buf_h, 20, y, buf_w - 40, card_h, border_color);

                        // Preset Number & Name
                        let title_color = if is_selected { 0xFACC15 } else { 0xE2E8F0 };
                        let title_text = format!("{:2}. {}", preset.id, preset.name);
                        draw_consolas_bold_text(&mut buffer, buf_w, buf_h, &title_text, 34, y + 16, title_color, 15.0);

                        // Margin specs details
                        let spec_text = format!(
                            "Izq:{}px | Gap:{}px | Der:{}px | Vert:{}px | Sep:{}px",
                            preset.pad_left, preset.icon_gap, preset.pad_right, preset.pad_vert, preset.btn_spacing
                        );
                        draw_consolas_bold_text(&mut buffer, buf_w, buf_h, &spec_text, 280, y + 17, 0x94A3B8, 13.0);

                        // Render Live Buttons Preview for this preset!
                        let btns_x = 680usize;
                        let btn_h = (icon_size as usize + preset.pad_vert * 2).max(28);
                        let btn_y = y + (card_h.saturating_sub(btn_h)) / 2;

                        let buttons_data = [
                            ("Copiar (C)", IconType::Clipboard, 0x16A34A),
                            ("Guardar (S)", IconType::Save, 0x2563EB),
                            ("Ambos (Enter)", IconType::Combo, 0x0284C7),
                            ("Cancelar (Esc)", IconType::Cancel, 0xDC2626),
                        ];

                        let mut cur_x = btns_x;
                        for (label, icon_type, bg_col) in buttons_data {
                            let text_w = measure_consolas_bold_width(label, font_size);
                            let btn_w = preset.pad_left + icon_size as usize + preset.icon_gap + text_w + preset.pad_right;

                            if cur_x + btn_w > buf_w - 30 {
                                break;
                            }

                            // Draw button container
                            fill_rect(&mut buffer, buf_w, buf_h, cur_x, btn_y, btn_w, btn_h, bg_col);

                            // Draw SVG Icon
                            let icon_x = cur_x + preset.pad_left;
                            let icon_y = btn_y + preset.pad_vert;
                            draw_svg_icon(&mut buffer, buf_w, buf_h, icon_type, icon_x, icon_y, icon_size);

                            // Draw Consolas Bold Mono text
                            let text_x = icon_x + icon_size as usize + preset.icon_gap;
                            let text_y = btn_y + preset.pad_vert;
                            draw_consolas_bold_text(&mut buffer, buf_w, buf_h, label, text_x, text_y, 0xFFFFFF, font_size);

                            cur_x += btn_w + preset.btn_spacing;
                        }
                    }

                    buffer.present().unwrap();
                }
                _ => {}
            },
            _ => {}
        }
    }).map_err(|e| anyhow::anyhow!("Event loop error: {:?}", e))?;

    Ok(())
}

fn fill_rect(buffer: &mut [u32], buf_w: usize, buf_h: usize, x: usize, y: usize, w: usize, h: usize, color: u32) {
    let x2 = (x + w).min(buf_w);
    let y2 = (y + h).min(buf_h);
    for py in y..y2 {
        for px in x..x2 {
            buffer[py * buf_w + px] = color;
        }
    }
}

fn draw_border(buffer: &mut [u32], buf_w: usize, buf_h: usize, x: usize, y: usize, w: usize, h: usize, color: u32) {
    let x2 = (x + w).min(buf_w);
    let y2 = (y + h).min(buf_h);
    for px in x..x2 {
        if y < buf_h { buffer[y * buf_w + px] = color; }
        if y2 > 0 && y2 - 1 < buf_h { buffer[(y2 - 1) * buf_w + px] = color; }
    }
    for py in y..y2 {
        if x < buf_w { buffer[py * buf_w + x] = color; }
        if x2 > 0 && x2 - 1 < buf_w { buffer[py * buf_w + x2 - 1] = color; }
    }
}
