// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 juanre7

use anyhow::Result;
use softbuffer::{Context, Surface};
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowAttributes};

// Included wholesale but used in part: each tool needs a slice of the module, and
// `#[path]` brings in all of it. The unused half is not dead code, it is the rest of
// the application. Scoped to the include so the tool's own code stays linted.
#[path = "../font.rs"]
#[allow(dead_code)]
mod font;

use font::{Canvas, draw_consolas_bold_text, draw_svg_icon, measure_consolas_bold_width, IconType};

#[derive(Debug, Clone)]
pub struct TrayMenuTheme {
    pub menu_padding_x: usize,
    pub menu_padding_y: usize,
    pub menu_border_radius: usize,
    pub menu_bg_color: u32,
    pub menu_border_color: u32,
    pub item_height: usize,
    pub item_hover_bg: u32,
    pub icon_left_margin: usize,
    pub icon_size: u32,
    pub icon_text_gap: usize,
    pub font_size: f32,
    pub text_color: u32,
    pub shortcut_color: u32,
    pub separator_color: u32,
    pub separator_margin_y: usize,
}

impl Default for TrayMenuTheme {
    fn default() -> Self {
        Self {
            menu_padding_x: 6,
            menu_padding_y: 6,
            menu_border_radius: 8,
            menu_bg_color: 0x0F172A,      // Slate 900
            menu_border_color: 0x334155,  // Slate 700
            item_height: 30,
            item_hover_bg: 0x1E293B,      // Slate 800
            icon_left_margin: 8,
            icon_size: 16,
            icon_text_gap: 10,
            font_size: 14.0,
            text_color: 0xF8FAFC,        // Slate 50
            shortcut_color: 0x94A3B8,    // Slate 400
            separator_color: 0x334155,
            separator_margin_y: 4,
        }
    }
}

struct SliderControl {
    label: &'static str,
    val_min: f32,
    val_max: f32,
    curr_val: f32,
}

struct CategoryAccordion {
    name: &'static str,
    collapsed: bool,
    start_idx: usize,
    end_idx: usize,
}

struct TrayEditorApp {
    window: Option<Arc<Window>>,
    surface: Option<Surface<Arc<Window>, Arc<Window>>>,
    theme: TrayMenuTheme,
    sliders: Vec<SliderControl>,
    categories: Vec<CategoryAccordion>,
    active_slider: Option<usize>,
    hovered_item: Option<usize>,
    mouse_pos: (i32, i32),
    _last_frame_time: Instant,
}

impl ApplicationHandler for TrayEditorApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let attrs = WindowAttributes::default()
            .with_title("Ruuutu - System Tray Menu Geometry & Margin Studio Editor")
            .with_inner_size(PhysicalSize::new(1040, 720))
            .with_resizable(false);

        let window = Arc::new(event_loop.create_window(attrs).unwrap());
        let context = Context::new(window.clone()).unwrap();
        let mut surface = Surface::new(&context, window.clone()).unwrap();

        surface.resize(NonZeroU32::new(1040).unwrap(), NonZeroU32::new(720).unwrap()).unwrap();

        self.window = Some(window);
        self.surface = Some(surface);
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(ref window) = self.window {
            window.request_redraw();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _window_id: winit::window::WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::CursorMoved { position, .. } => {
                let px = position.x as i32;
                let py = position.y as i32;
                self.mouse_pos = (px, py);

                if let Some(idx) = self.active_slider {
                    let s = &mut self.sliders[idx];
                    let slider_left = 680;
                    let slider_width = 300.0;
                    let rel_x = ((px - slider_left) as f32).clamp(0.0, slider_width);
                    s.curr_val = s.val_min + (rel_x / slider_width) * (s.val_max - s.val_min);
                    self.sync_theme_from_sliders();
                }

                // Check menu hover preview item
                let menu_x = 80;
                let menu_y = 130;
                let item_h = self.theme.item_height as i32;
                if px >= menu_x && px < menu_x + 340 && py >= menu_y {
                    let rel_y = py - menu_y - self.theme.menu_padding_y as i32;
                    if rel_y >= 0 {
                        let idx = (rel_y / item_h) as usize;
                        if idx < 7 {
                            self.hovered_item = Some(idx);
                        } else {
                            self.hovered_item = None;
                        }
                    }
                } else {
                    self.hovered_item = None;
                }
            }
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. } => {
                // Check slider click
                let mut clicked_slider = None;
                let mut slider_y = 120;

                for cat in self.categories.iter() {
                    slider_y += 32;

                    if cat.collapsed {
                        continue;
                    }

                    for i in cat.start_idx..cat.end_idx {
                        let track_y = slider_y + 16;
                        if (self.mouse_pos.1 - track_y).abs() <= 12 && self.mouse_pos.0 >= 650 && self.mouse_pos.0 <= 980 {
                            clicked_slider = Some(i);
                            break;
                        }
                        slider_y += 38;
                    }
                }

                if let Some(idx) = clicked_slider {
                    self.active_slider = Some(idx);
                    let s = &mut self.sliders[idx];
                    let rel_x = ((self.mouse_pos.0 - 655) as f32).clamp(0.0, 320.0);
                    s.curr_val = s.val_min + (rel_x / 320.0) * (s.val_max - s.val_min);
                    self.sync_theme_from_sliders();
                }

                // Check Category Accordion Toggle
                let mut toggle_cat = None;
                let mut acc_y = 120;
                for (cat_idx, cat) in self.categories.iter().enumerate() {
                    if self.mouse_pos.0 >= 645 && self.mouse_pos.0 <= 995 && self.mouse_pos.1 >= acc_y && self.mouse_pos.1 < acc_y + 26 {
                        toggle_cat = Some(cat_idx);
                        break;
                    }
                    acc_y += 32;
                    if !cat.collapsed {
                        acc_y += ((cat.end_idx - cat.start_idx) * 38) as i32;
                    }
                }
                if let Some(c_idx) = toggle_cat {
                    self.categories[c_idx].collapsed = !self.categories[c_idx].collapsed;
                }

                // Check Preset Buttons
                if self.mouse_pos.1 >= 640 && self.mouse_pos.1 <= 680 {
                    if self.mouse_pos.0 >= 80 && self.mouse_pos.0 <= 220 {
                        self.apply_preset_compact();
                    } else if self.mouse_pos.0 >= 240 && self.mouse_pos.0 <= 380 {
                        self.apply_preset_win11();
                    } else if self.mouse_pos.0 >= 400 && self.mouse_pos.0 <= 540 {
                        self.apply_preset_ultra();
                    }
                }
            }
            WindowEvent::MouseInput { state: ElementState::Released, button: MouseButton::Left, .. } => {
                self.active_slider = None;
            }
            WindowEvent::RedrawRequested => {
                self.redraw();
            }
            _ => {}
        }
    }
}

impl TrayEditorApp {
    fn sync_theme_from_sliders(&mut self) {
        self.theme.menu_padding_x = self.sliders[0].curr_val as usize;
        self.theme.menu_padding_y = self.sliders[1].curr_val as usize;
        self.theme.icon_left_margin = self.sliders[2].curr_val as usize;
        self.theme.icon_size = self.sliders[3].curr_val as u32;
        self.theme.icon_text_gap = self.sliders[4].curr_val as usize;
        self.theme.item_height = self.sliders[5].curr_val as usize;
        self.theme.font_size = self.sliders[6].curr_val;
        self.theme.menu_border_radius = self.sliders[7].curr_val as usize;
    }

    fn apply_preset_compact(&mut self) {
        self.sliders[0].curr_val = 4.0;
        self.sliders[1].curr_val = 4.0;
        self.sliders[2].curr_val = 4.0;
        self.sliders[3].curr_val = 14.0;
        self.sliders[4].curr_val = 6.0;
        self.sliders[5].curr_val = 26.0;
        self.sliders[6].curr_val = 13.0;
        self.sliders[7].curr_val = 6.0;
        self.sync_theme_from_sliders();
    }

    fn apply_preset_win11(&mut self) {
        self.sliders[0].curr_val = 8.0;
        self.sliders[1].curr_val = 8.0;
        self.sliders[2].curr_val = 10.0;
        self.sliders[3].curr_val = 16.0;
        self.sliders[4].curr_val = 10.0;
        self.sliders[5].curr_val = 32.0;
        self.sliders[6].curr_val = 14.0;
        self.sliders[7].curr_val = 10.0;
        self.sync_theme_from_sliders();
    }

    fn apply_preset_ultra(&mut self) {
        self.sliders[0].curr_val = 2.0;
        self.sliders[1].curr_val = 4.0;
        self.sliders[2].curr_val = 6.0;
        self.sliders[3].curr_val = 16.0;
        self.sliders[4].curr_val = 8.0;
        self.sliders[5].curr_val = 28.0;
        self.sliders[6].curr_val = 13.5;
        self.sliders[7].curr_val = 8.0;
        self.sync_theme_from_sliders();
    }

    fn redraw(&mut self) {
        let surface = self.surface.as_mut().unwrap();
        let mut buffer = surface.buffer_mut().unwrap();

        // Fill background dark slate
        buffer.fill(0x090D16);

        let buf_w = 1040;
        let buf_h = 720;
        let mut canvas = Canvas::new(&mut buffer, buf_w, buf_h);

        // Draw Studio Header
        draw_filled_rect(&mut canvas, 0, 0, buf_w, 50, 0x0F172A);
        draw_consolas_bold_text(&mut canvas, "RUUUTU STUDIO: System Tray Menu Geometry & Margin Inspector", 30, 16, 0x00A2FF, 17.0);

        // Draw Left Panel Preview Area
        draw_filled_rect(&mut canvas, 30, 70, 580, 620, 0x020617);
        draw_border_rect(&mut canvas, 30, 70, 580, 620, 0x1E293B);

        draw_consolas_bold_text(&mut canvas, "LIVE SYSTEM TRAY MENU PREVIEW", 50, 88, 0x94A3B8, 14.0);

        // -------------------------------------------------------------
        // RENDER LIVE SYSTEM TRAY MOCKUP MENU
        // -------------------------------------------------------------
        let menu_x = 80usize;
        let menu_y = 130usize;
        let menu_w = 340usize;

        let items = [
            ("⚡ Tomar Captura", "PrtScn / Alt+A", IconType::Clipboard),
            ("🎨 Formato de Imagen", "WebP ►", IconType::Save),
            ("⚙️ Calidad / Compresión", "Alta (90%) ►", IconType::Combo),
            ("⌨️ Atajo de Teclado", "PrtScn ►", IconType::Clipboard),
            ("🚀 Iniciar con Windows", "[✓]", IconType::Save),
            ("📁 Abrir Carpeta de Capturas", "", IconType::Save),
            ("❌ Salir de Ruuutu", "", IconType::Cancel),
        ];

        let total_item_h = items.len() * self.theme.item_height;
        let menu_h = self.theme.menu_padding_y * 2 + total_item_h;

        // Menu Card Background
        draw_filled_rect(&mut canvas, menu_x, menu_y, menu_w, menu_h, self.theme.menu_bg_color);
        draw_border_rect(&mut canvas, menu_x, menu_y, menu_w, menu_h, self.theme.menu_border_color);

        let mut curr_item_y = menu_y + self.theme.menu_padding_y;

        for (idx, (label, shortcut, icon)) in items.iter().enumerate() {
            let is_hovered = self.hovered_item == Some(idx);
            let item_x = menu_x + self.theme.menu_padding_x;
            let item_w = menu_w - self.theme.menu_padding_x * 2;
            let item_h = self.theme.item_height;

            if is_hovered {
                draw_filled_rect(&mut canvas, item_x, curr_item_y, item_w, item_h, self.theme.item_hover_bg);
                draw_border_rect(&mut canvas, item_x, curr_item_y, item_w, item_h, 0x00A2FF);
            }

            // Leading Icon / Margin
            let icon_x = item_x + self.theme.icon_left_margin;
            let icon_y = curr_item_y + (item_h - self.theme.icon_size as usize) / 2;

            draw_svg_icon(&mut canvas, *icon, icon_x, icon_y, self.theme.icon_size);

            // Label Text
            let text_x = icon_x + self.theme.icon_size as usize + self.theme.icon_text_gap;
            let text_y = curr_item_y + (item_h - self.theme.font_size as usize) / 2;

            let col = if is_hovered { 0x00A2FF } else { self.theme.text_color };
            draw_consolas_bold_text(&mut canvas, label, text_x, text_y, col, self.theme.font_size);

            // Shortcut / Arrow text on right
            if !shortcut.is_empty() {
                let sc_w = measure_consolas_bold_width(shortcut, self.theme.font_size - 1.5);
                let sc_x = (item_x + item_w).saturating_sub(sc_w + 10);
                draw_consolas_bold_text(&mut canvas, shortcut, sc_x, text_y + 1, self.theme.shortcut_color, self.theme.font_size - 1.5);
            }

            // Draw Visual Margin Guidelines in Magenta
            draw_border_rect(&mut canvas, icon_x, icon_y, self.theme.icon_size as usize, self.theme.icon_size as usize, 0xFF00FF);

            curr_item_y += item_h;
        }

        // Draw Telemetry Measurement Box Below Preview
        draw_filled_rect(&mut canvas, 50, 480, 540, 140, 0x0B132B);
        draw_border_rect(&mut canvas, 50, 480, 540, 140, 0x1E293B);

        draw_consolas_bold_text(&mut canvas, "📐 GEOMETRY MEASUREMENTS", 65, 495, 0x00A2FF, 13.5);
        let m1 = format!("Menu Padding X : {} px  |  Padding Y : {} px", self.theme.menu_padding_x, self.theme.menu_padding_y);
        let m2 = format!("Icon Left Margin: {} px  |  Icon Size : {} px", self.theme.icon_left_margin, self.theme.icon_size);
        let m3 = format!("Icon-Text Gap   : {} px  |  Item Height: {} px", self.theme.icon_text_gap, self.theme.item_height);

        draw_consolas_bold_text(&mut canvas, &m1, 65, 520, 0xE2E8F0, 12.5);
        draw_consolas_bold_text(&mut canvas, &m2, 65, 540, 0xE2E8F0, 12.5);
        draw_consolas_bold_text(&mut canvas, &m3, 65, 560, 0xE2E8F0, 12.5);

        // Draw Preset Buttons
        draw_filled_rect(&mut canvas, 80, 640, 140, 36, 0x1E293B);
        draw_border_rect(&mut canvas, 80, 640, 140, 36, 0x00A2FF);
        draw_consolas_bold_text(&mut canvas, "Compacto", 110, 650, 0xFFFFFF, 13.0);

        draw_filled_rect(&mut canvas, 240, 640, 140, 36, 0x1E293B);
        draw_border_rect(&mut canvas, 240, 640, 140, 36, 0x00A2FF);
        draw_consolas_bold_text(&mut canvas, "Windows 11", 265, 650, 0xFFFFFF, 13.0);

        draw_filled_rect(&mut canvas, 400, 640, 140, 36, 0x1E293B);
        draw_border_rect(&mut canvas, 400, 640, 140, 36, 0x00A2FF);
        draw_consolas_bold_text(&mut canvas, "Ultra Clean", 425, 650, 0xFFFFFF, 13.0);

        // -------------------------------------------------------------
        // RENDER RIGHT CONTROL PANEL SLIDERS
        // -------------------------------------------------------------
        draw_filled_rect(&mut canvas, 630, 70, 380, 620, 0x020617);
        draw_border_rect(&mut canvas, 630, 70, 380, 620, 0x1E293B);

        draw_consolas_bold_text(&mut canvas, "CONTROLES DE MÁRGENES Y GEOMETRÍA", 650, 88, 0xFACC15, 14.0);

        let mut slider_y = 120;
        for cat in self.categories.iter() {
            let cat_icon = if cat.collapsed { "►" } else { "▼" };
            let cat_title = format!("{} {}", cat_icon, cat.name);
            draw_filled_rect(&mut canvas, 645, slider_y, 350, 26, 0x0F172A);
            draw_consolas_bold_text(&mut canvas, &cat_title, 655, slider_y + 5, 0x38BDF8, 13.0);
            slider_y += 32;

            if cat.collapsed {
                continue;
            }

            for idx in cat.start_idx..cat.end_idx {
                let s = &self.sliders[idx];
                let is_active = self.active_slider == Some(idx);

                draw_consolas_bold_text(&mut canvas, s.label, 655, slider_y, 0xCBD5E1, 12.0);

                let val_str = format!("{:.1}", s.curr_val);
                draw_consolas_bold_text(&mut canvas, &val_str, 955, slider_y, 0x00A2FF, 12.0);

                // Slider Track
                let track_x = 655;
                let track_y = slider_y + 16;
                let track_w = 320;
                draw_filled_rect(&mut canvas, track_x, track_y, track_w, 4, 0x334155);

                // Active Fill
                let fill_pct = ((s.curr_val - s.val_min) / (s.val_max - s.val_min)).clamp(0.0, 1.0);
                let fill_w = (track_w as f32 * fill_pct) as usize;
                let fill_col = if is_active { 0x22C55E } else { 0x00A2FF };
                draw_filled_rect(&mut canvas, track_x, track_y, fill_w, 4, fill_col);

                // Thumb Knob
                let knob_x = track_x + fill_w.saturating_sub(6);
                draw_filled_rect(&mut canvas, knob_x, track_y - 4, 12, 12, 0xFFFFFF);
                draw_border_rect(&mut canvas, knob_x, track_y - 4, 12, 12, fill_col);

                slider_y += 38;
            }
        }

        buffer.present().unwrap();
    }
}

fn draw_filled_rect(canvas: &mut Canvas, x: usize, y: usize, w: usize, h: usize, color: u32) {
    let x2 = (x + w).min(canvas.w);
    let y2 = (y + h).min(canvas.h);
    for py in y..y2 {
        for px in x..x2 {
            canvas.pixels[py * canvas.w + px] = color;
        }
    }
}

fn draw_border_rect(canvas: &mut Canvas, x: usize, y: usize, w: usize, h: usize, color: u32) {
    let x2 = (x + w).min(canvas.w);
    let y2 = (y + h).min(canvas.h);
    for px in x..x2 {
        if y < canvas.h { canvas.pixels[y * canvas.w + px] = color; }
        if y2 > 0 && y2 - 1 < canvas.h { canvas.pixels[(y2 - 1) * canvas.w + px] = color; }
    }
    for py in y..y2 {
        if x < canvas.w { canvas.pixels[py * canvas.w + x] = color; }
        if x2 > 0 && x2 - 1 < canvas.w { canvas.pixels[py * canvas.w + x2 - 1] = color; }
    }
}

fn main() -> Result<()> {
    println!("\n=======================================================");
    println!(" 🚀 RUUUTU SYSTEM TRAY MENU GEOMETRY STUDIO EDITOR ");
    println!(" Ajusta en tiempo real margenes, iconos y espaciado.");
    println!("=======================================================\n");

    let event_loop = EventLoop::builder().build()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let theme = TrayMenuTheme::default();

    let sliders = vec![
        SliderControl { label: "Menu Padding X (Margen Horizontal)", val_min: 0.0, val_max: 30.0, curr_val: 6.0 },
        SliderControl { label: "Menu Padding Y (Margen Vertical)", val_min: 0.0, val_max: 30.0, curr_val: 6.0 },
        SliderControl { label: "Icon Left Margin (Izquierda a Icono)", val_min: 0.0, val_max: 30.0, curr_val: 8.0 },
        SliderControl { label: "Icon Size (Tamaño Icono)", val_min: 10.0, val_max: 32.0, curr_val: 16.0 },
        SliderControl { label: "Icon Text Gap (Distancia Icono-Texto)", val_min: 0.0, val_max: 30.0, curr_val: 10.0 },
        SliderControl { label: "Item Height (Alto de Fila)", val_min: 20.0, val_max: 50.0, curr_val: 30.0 },
        SliderControl { label: "Font Size (Tamaño de Texto)", val_min: 10.0, val_max: 20.0, curr_val: 14.0 },
        SliderControl { label: "Menu Border Radius (Redondeo)", val_min: 0.0, val_max: 20.0, curr_val: 8.0 },
    ];

    let categories = vec![
        CategoryAccordion { name: "Márgenes del Menú", collapsed: false, start_idx: 0, end_idx: 2 },
        CategoryAccordion { name: "Geometría de Icono", collapsed: false, start_idx: 2, end_idx: 5 },
        CategoryAccordion { name: "Filas y Tipografía", collapsed: false, start_idx: 5, end_idx: 8 },
    ];

    let mut app = TrayEditorApp {
        window: None,
        surface: None,
        theme,
        sliders,
        categories,
        active_slider: None,
        hovered_item: None,
        mouse_pos: (0, 0),
        _last_frame_time: Instant::now(),
    };

    event_loop.run_app(&mut app)?;
    Ok(())
}
