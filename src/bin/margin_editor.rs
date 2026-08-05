// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 juanre7

use anyhow::{Context, Result};
use softbuffer::{Context as SbContext, Surface};
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Instant;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, Event, KeyEvent, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::WindowAttributes;

#[path = "../font.rs"]
mod font;

use font::{draw_consolas_bold_text, draw_consolas_bold_text_clipped, draw_svg_icon, measure_consolas_bold_width, IconType};

#[derive(Debug, Clone)]
struct EditableProp {
    id: &'static str,
    name: &'static str,
    val: f32,
    min: f32,
    max: f32,
    step: f32,
    unit: &'static str,
}

#[derive(Debug, Clone)]
struct PropCategory {
    name: &'static str,
    expanded: bool,
    props: Vec<EditableProp>,
}

fn default_categories() -> Vec<PropCategory> {
    vec![
        PropCategory {
            name: "1. MODO Y ANIMACIÓN HOVER ⚡",
            expanded: true,
            props: vec![
                EditableProp { id: "icon_only",     name: "Modo Solo Iconos (Cuadrados)",  val: 1.0,  min: 0.0,  max: 1.0,  step: 1.0, unit: " (0=Off,1=On)" },
                EditableProp { id: "hover_delay",   name: "Delay para Abrir (Retardo)",    val: 100.0,min: 0.0,  max: 600.0,step: 25.0,unit: "ms" },
                EditableProp { id: "anim_speed",    name: "Velocidad Transición (Suavidad)",val: 0.20, min: 0.05, max: 0.80, step: 0.05,unit: " f" },
                EditableProp { id: "hover_lift_y",  name: "Elevación Y al hacer Hover",     val: -2.0, min: -10.0,max: 10.0, step: 1.0, unit: "px" },
                EditableProp { id: "hover_scale",   name: "Escala / Tamaño Extra Hover",    val: 0.0,  min: -4.0, max: 8.0,  step: 1.0, unit: "px" },
            ],
        },
        PropCategory {
            name: "2. TAMAÑOS DE ICONOS VECTORIALES 🎨",
            expanded: true,
            props: vec![
                EditableProp { id: "icon_sz_copy",  name: "Tamaño Icono Copiar",          val: 20.0, min: 10.0, max: 36.0, step: 1.0, unit: "px" },
                EditableProp { id: "icon_sz_save",  name: "Tamaño Icono Guardar",         val: 20.0, min: 10.0, max: 36.0, step: 1.0, unit: "px" },
                EditableProp { id: "icon_sz_combo", name: "Tamaño Icono Ambos",           val: 20.0, min: 10.0, max: 36.0, step: 1.0, unit: "px" },
                EditableProp { id: "icon_sz_cancel",name: "Tamaño Icono Cancelar",        val: 20.0, min: 10.0, max: 36.0, step: 1.0, unit: "px" },
                EditableProp { id: "icon_off_y",    name: "Offset Vert Iconos",           val: 0.0,  min: -15.0,max: 15.0, step: 1.0, unit: "px" },
            ],
        },
        PropCategory {
            name: "3. TIPOGRAFÍA Y TEXTO 🔤",
            expanded: true,
            props: vec![
                EditableProp { id: "font_sz",       name: "Tamaño de Fuente (Consolas)",  val: 17.0, min: 10.0, max: 28.0, step: 0.5, unit: "px" },
                EditableProp { id: "text_off_y",    name: "Offset Vert Texto",           val: 0.0,  min: -15.0,max: 15.0, step: 1.0, unit: "px" },
            ],
        },
        PropCategory {
            name: "4. BOTONES Y PADDINGS 📐",
            expanded: true,
            props: vec![
                EditableProp { id: "pad_l",         name: "Margen Izquierdo (Desplegado)",  val: 8.0,  min: 0.0,  max: 30.0, step: 1.0, unit: "px" },
                EditableProp { id: "icon_gap",      name: "Espacio Icono <-> Texto",        val: 6.0,  min: 0.0,  max: 30.0, step: 1.0, unit: "px" },
                EditableProp { id: "pad_r",         name: "Margen Derecho (Desplegado)",    val: 10.0, min: 0.0,  max: 30.0, step: 1.0, unit: "px" },
                EditableProp { id: "pad_v",         name: "Padding Vertical Botón",         val: 7.0,  min: 0.0,  max: 25.0, step: 1.0, unit: "px" },
                EditableProp { id: "btn_spacing",   name: "Separación entre Botones",       val: 6.0,  min: 0.0,  max: 30.0, step: 1.0, unit: "px" },
            ],
        },
        PropCategory {
            name: "5. MARCO Y DIMENSIONES 📦",
            expanded: true,
            props: vec![
                EditableProp { id: "btn_gap_y",     name: "Distancia a la Selección",       val: 6.0,  min: 0.0,  max: 30.0, step: 1.0, unit: "px" },
                EditableProp { id: "dim_pad_h",     name: "Dimensión Margen Horiz",         val: 10.0, min: 0.0,  max: 30.0, step: 1.0, unit: "px" },
                EditableProp { id: "dim_pad_v",     name: "Dimensión Margen Vert",          val: 6.0,  min: 0.0,  max: 20.0, step: 1.0, unit: "px" },
                EditableProp { id: "border_w",      name: "Grosor Borde Selección",         val: 1.0,  min: 1.0,  max: 6.0,  step: 1.0, unit: "px" },
            ],
        },
    ]
}

#[derive(Debug, Clone)]
struct AnimatedButtonState {
    curr_w: f32,
    target_w: f32,
    curr_lift_y: f32,
    target_lift_y: f32,
    hover_enter_time: Option<Instant>,
}

fn main() -> Result<()> {
    let mut categories = default_categories();
    let width = 1360u32;
    let height = 940u32;

    let event_loop = EventLoop::new().context("Failed to create event loop")?;
    let attrs = WindowAttributes::default()
        .with_title("Ruuutu - Studio Editor (Cuadrados Perfectos, Centrado & Delay Editable)")
        .with_inner_size(PhysicalSize::new(width, height))
        .with_position(PhysicalPosition::new(20, 20));

    #[allow(deprecated)]
    let window = Arc::new(event_loop.create_window(attrs).context("Failed to create window")?);
    let context = SbContext::new(window.clone()).map_err(|e| anyhow::anyhow!("Context error: {:?}", e))?;
    let mut surface = Surface::new(&context, window.clone()).map_err(|e| anyhow::anyhow!("Surface error: {:?}", e))?;

    surface
        .resize(NonZeroU32::new(width).unwrap(), NonZeroU32::new(height).unwrap())
        .unwrap();

    let mut selected_cat = 0usize;
    let mut selected_prop = 0usize;
    let mut shift_pressed = false;
    let mut click_pos = (0i32, 0i32);

    let mut anim_states = vec![
        AnimatedButtonState { curr_w: 34.0, target_w: 34.0, curr_lift_y: 0.0, target_lift_y: 0.0, hover_enter_time: None };
        4
    ];

    #[allow(deprecated)]
    event_loop.run(move |event, elwh: &ActiveEventLoop| {
        elwh.set_control_flow(ControlFlow::Poll);

        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => elwh.exit(),
                WindowEvent::KeyboardInput { event: key_event, .. } => {
                    let is_pressed = key_event.state == ElementState::Pressed;
                    match key_event.logical_key {
                        Key::Named(NamedKey::Shift) => {
                            shift_pressed = is_pressed;
                        }
                        Key::Named(NamedKey::Escape) if is_pressed => elwh.exit(),
                        Key::Named(NamedKey::ArrowUp) if is_pressed => {
                            if selected_prop > 0 {
                                selected_prop -= 1;
                            } else if selected_cat > 0 {
                                selected_cat -= 1;
                                selected_prop = categories[selected_cat].props.len().saturating_sub(1);
                            }
                            window.request_redraw();
                        }
                        Key::Named(NamedKey::ArrowDown) if is_pressed => {
                            if selected_prop + 1 < categories[selected_cat].props.len() {
                                selected_prop += 1;
                            } else if selected_cat + 1 < categories.len() {
                                selected_cat += 1;
                                selected_prop = 0;
                            }
                            window.request_redraw();
                        }
                        Key::Named(NamedKey::ArrowLeft) if is_pressed => {
                            let mult = if shift_pressed { 5.0 } else { 1.0 };
                            let p = &mut categories[selected_cat].props[selected_prop];
                            p.val = (p.val - p.step * mult).clamp(p.min, p.max);
                            window.request_redraw();
                        }
                        Key::Named(NamedKey::ArrowRight) if is_pressed => {
                            let mult = if shift_pressed { 5.0 } else { 1.0 };
                            let p = &mut categories[selected_cat].props[selected_prop];
                            p.val = (p.val + p.step * mult).clamp(p.min, p.max);
                            window.request_redraw();
                        }
                        Key::Character(ref s) if is_pressed && s == "r" => {
                            categories = default_categories();
                            window.request_redraw();
                        }
                        _ => {}
                    }
                }
                WindowEvent::CursorMoved { position, .. } => {
                    click_pos = (position.x as i32, position.y as i32);
                    window.request_redraw();
                }
                WindowEvent::MouseInput {
                    state: ElementState::Pressed,
                    button: MouseButton::Left,
                    ..
                } => {
                    let cx = click_pos.0;
                    let cy = click_pos.1;

                    // Click left panel controls
                    if cx >= 15 && cx <= 570 {
                        let mut curr_y = 80i32;
                        let item_h = 36i32;

                        for (c_idx, cat) in categories.iter_mut().enumerate() {
                            if cy >= curr_y && cy < curr_y + 34 {
                                cat.expanded = !cat.expanded;
                                selected_cat = c_idx;
                                selected_prop = 0;
                                window.request_redraw();
                                break;
                            }
                            curr_y += 36;

                            if cat.expanded {
                                for (p_idx, p) in cat.props.iter_mut().enumerate() {
                                    if cy >= curr_y && cy < curr_y + item_h {
                                        selected_cat = c_idx;
                                        selected_prop = p_idx;

                                        if cx >= 440 && cx <= 480 {
                                            let mult = if shift_pressed { 5.0 } else { 1.0 };
                                            p.val = (p.val - p.step * mult).clamp(p.min, p.max);
                                        } else if cx >= 490 && cx <= 530 {
                                            let mult = if shift_pressed { 5.0 } else { 1.0 };
                                            p.val = (p.val + p.step * mult).clamp(p.min, p.max);
                                        }
                                        window.request_redraw();
                                        break;
                                    }
                                    curr_y += item_h;
                                }
                            }
                        }
                    }
                }
                WindowEvent::RedrawRequested => {
                    let now = Instant::now();

                    let mut buffer = surface.buffer_mut().unwrap();
                    let buf_w = width as usize;
                    let buf_h = height as usize;

                    fill_rect(&mut buffer, buf_w, buf_h, 0, 0, buf_w, buf_h, 0x0F172A);

                    draw_consolas_bold_text(
                        &mut buffer, buf_w, buf_h,
                        "RUUUTU - STUDIO EDITOR (CUADRADOS PERFECTOS, CENTRADO & DELAY MS)",
                        24, 16, 0x38BDF8, 19.0,
                    );
                    draw_consolas_bold_text(
                        &mut buffer, buf_w, buf_h,
                        "Botones cerrados = Cuadrados perfectos con icono centrado. Expansión fluida tras delay.",
                        24, 42, 0x94A3B8, 12.5,
                    );

                    let get_val = |id: &str| -> f32 {
                        for cat in &categories {
                            for p in &cat.props {
                                if p.id == id { return p.val; }
                            }
                        }
                        0.0
                    };

                    let icon_only_mode = get_val("icon_only") > 0.5;
                    let hover_delay_ms = get_val("hover_delay") as u128;
                    let anim_speed     = get_val("anim_speed").clamp(0.01, 1.0);
                    let hover_lift_y   = get_val("hover_lift_y");
                    let hover_scale    = get_val("hover_scale");
                    let icon_sz_copy   = get_val("icon_sz_copy") as u32;
                    let icon_sz_save   = get_val("icon_sz_save") as u32;
                    let icon_sz_combo  = get_val("icon_sz_combo") as u32;
                    let icon_sz_cancel = get_val("icon_sz_cancel") as u32;
                    let font_sz        = get_val("font_sz");
                    let pad_l          = get_val("pad_l") as usize;
                    let icon_gap       = get_val("icon_gap") as usize;
                    let pad_r          = get_val("pad_r") as usize;
                    let pad_v          = get_val("pad_v") as usize;
                    let icon_off_y     = get_val("icon_off_y") as i32;
                    let text_off_y     = get_val("text_off_y") as i32;
                    let btn_spacing    = get_val("btn_spacing") as usize;
                    let btn_gap_y      = get_val("btn_gap_y") as usize;
                    let dim_pad_h      = get_val("dim_pad_h") as usize;
                    let dim_pad_v      = get_val("dim_pad_v") as usize;
                    let border_w       = get_val("border_w") as usize;

                    // LEFT PANEL
                    let panel_w = 540usize;
                    fill_rect(&mut buffer, buf_w, buf_h, 15, 70, panel_w, buf_h - 130, 0x1E293B);
                    draw_border(&mut buffer, buf_w, buf_h, 15, 70, panel_w, buf_h - 130, 0x334155);

                    let mut render_y = 75usize;

                    for (c_idx, cat) in categories.iter().enumerate() {
                        let is_cat_sel = c_idx == selected_cat;

                        let header_bg = if is_cat_sel { 0x0284C7 } else { 0x334155 };
                        fill_rect(&mut buffer, buf_w, buf_h, 20, render_y, panel_w - 10, 30, header_bg);

                        let arrow = if cat.expanded { "▼ " } else { "► " };
                        let header_text = format!("{}{}", arrow, cat.name);
                        draw_consolas_bold_text(&mut buffer, buf_w, buf_h, &header_text, 28, render_y + 7, 0xFFFFFF, 13.5);

                        render_y += 34;

                        if cat.expanded {
                            for (p_idx, p) in cat.props.iter().enumerate() {
                                let is_prop_sel = is_cat_sel && p_idx == selected_prop;
                                let row_bg = if is_prop_sel { 0x475569 } else { 0x1E293B };
                                fill_rect(&mut buffer, buf_w, buf_h, 25, render_y, panel_w - 20, 32, row_bg);

                                let label_col = if is_prop_sel { 0xFACC15 } else { 0xE2E8F0 };
                                draw_consolas_bold_text(&mut buffer, buf_w, buf_h, p.name, 35, render_y + 8, label_col, 12.5);

                                let val_str = if p.step < 1.0 { format!("{:.2}{}", p.val, p.unit) } else { format!("{:.0}{}", p.val, p.unit) };
                                draw_consolas_bold_text(&mut buffer, buf_w, buf_h, &val_str, 355, render_y + 8, 0x38BDF8, 12.5);

                                fill_rect(&mut buffer, buf_w, buf_h, 440, render_y + 4, 34, 24, 0x334155);
                                draw_consolas_bold_text(&mut buffer, buf_w, buf_h, "-", 453, render_y + 7, 0xFFFFFF, 14.0);

                                fill_rect(&mut buffer, buf_w, buf_h, 485, render_y + 4, 34, 24, 0x334155);
                                draw_consolas_bold_text(&mut buffer, buf_w, buf_h, "+", 497, render_y + 7, 0xFFFFFF, 14.0);

                                render_y += 36;
                            }
                        }
                    }

                    // RIGHT PANEL
                    let prev_x = 575usize;
                    let prev_y = 70usize;
                    let prev_w = buf_w - prev_x - 15;
                    let prev_h = buf_h - 130;

                    fill_rect(&mut buffer, buf_w, buf_h, prev_x, prev_y, prev_w, prev_h, 0x020617);
                    draw_border(&mut buffer, buf_w, buf_h, prev_x, prev_y, prev_w, prev_h, 0x1E293B);

                    let mode_desc = if icon_only_mode {
                        format!("MODO ACTIVO: BOTONES CUADRADOS PERFECTOS | DELAY {} MS", hover_delay_ms)
                    } else {
                        "MODO ACTIVO: ICONO + TEXTO SIEMPRE VISIBLE".to_string()
                    };
                    draw_consolas_bold_text(&mut buffer, buf_w, buf_h, &mode_desc, prev_x + 20, prev_y + 18, 0x38BDF8, 13.0);

                    // Simulated selection box
                    let sel_x = prev_x + 60;
                    let sel_y = prev_y + 130;
                    let sel_w = 600usize;
                    let sel_h = 380usize;

                    fill_rect(&mut buffer, buf_w, buf_h, sel_x, sel_y, sel_w, sel_h, 0x0F172A);

                    for b in 0..border_w {
                        draw_border(&mut buffer, buf_w, buf_h, sel_x - b, sel_y - b, sel_w + b * 2, sel_h + b * 2, 0x00A2FF);
                    }

                    // Dimension Badge
                    let dim_text = format!("{} x {} px", sel_w, sel_h);
                    let dim_text_w = measure_consolas_bold_width(&dim_text, font_sz);
                    let badge_w = dim_pad_h * 2 + dim_text_w;
                    let badge_h = (font_sz as usize + dim_pad_v * 2).max(24);
                    let badge_x = sel_x;
                    let badge_y = if sel_y >= badge_h + 6 { sel_y - badge_h - 6 } else { sel_y + 6 };

                    fill_rect(&mut buffer, buf_w, buf_h, badge_x, badge_y, badge_w, badge_h, 0x1E293B);
                    let dim_text_y = (badge_y as i32 + dim_pad_v as i32 + text_off_y).max(0) as usize;
                    draw_consolas_bold_text(&mut buffer, buf_w, buf_h, &dim_text, badge_x + dim_pad_h, dim_text_y, 0xFFFFFF, font_sz);

                    // Action Buttons with Perfect Square Collapsed State & Centered Icons
                    let buttons_data = [
                        ("Copiar (C)",       IconType::Clipboard, icon_sz_copy,   0x16A34A, 0x22C55E),
                        ("Guardar (S)",      IconType::Save,      icon_sz_save,   0x2563EB, 0x3B82F6),
                        ("Ambos (Enter)",    IconType::Combo,     icon_sz_combo,  0x0284C7, 0x38BDF8),
                        ("Cancelar (Esc)",   IconType::Cancel,    icon_sz_cancel, 0xDC2626, 0xEF4444),
                    ];

                    let max_icon_sz = [icon_sz_copy, icon_sz_save, icon_sz_combo, icon_sz_cancel].into_iter().max().unwrap_or(20);
                    let base_button_h = (max_icon_sz as usize + pad_v * 2).max(28);

                    // Collapsed state is a PERFECT SQUARE: width == height == base_button_h
                    let square_w = base_button_h as f32;

                    let btn_start_y = sel_y + sel_h + btn_gap_y;

                    // Calculate layout positions dynamically using current animated widths
                    let total_btns_w: f32 = anim_states.iter().map(|s| s.curr_w + btn_spacing as f32).sum::<f32>() - btn_spacing as f32;
                    let btn_start_x = (sel_x + sel_w) as f32 - total_btns_w;

                    let mut cur_btn_x = btn_start_x;
                    let mut needs_anim_redraw = false;

                    for (idx, (label, icon_t, icon_sz, bg_normal, bg_hover)) in buttons_data.iter().enumerate() {
                        let st = &mut anim_states[idx];

                        // Check mouse hover
                        let is_hovered = click_pos.0 >= cur_btn_x as i32 && click_pos.0 < (cur_btn_x + st.curr_w) as i32 &&
                                         click_pos.1 >= (btn_start_y as i32 - 15) && click_pos.1 < (btn_start_y as i32 + base_button_h as i32 + 15);

                        let text_w = measure_consolas_bold_width(label, font_sz);
                        let expanded_w = (pad_l + *icon_sz as usize + icon_gap + text_w + pad_r) as f32;

                        // Handle hover delay logic
                        if is_hovered {
                            if st.hover_enter_time.is_none() {
                                st.hover_enter_time = Some(now);
                            }
                        } else {
                            st.hover_enter_time = None;
                        }

                        let delay_passed = match st.hover_enter_time {
                            Some(t) => now.duration_since(t).as_millis() >= hover_delay_ms,
                            None => false,
                        };

                        let should_expand = !icon_only_mode || (is_hovered && delay_passed);

                        st.target_w = if should_expand { expanded_w + hover_scale } else { square_w };
                        st.target_lift_y = if is_hovered { hover_lift_y } else { 0.0 };

                        // Interpolate 60fps smooth easing
                        let diff_w = st.target_w - st.curr_w;
                        let diff_y = st.target_lift_y - st.curr_lift_y;

                        if diff_w.abs() > 0.1 || diff_y.abs() > 0.1 {
                            st.curr_w += diff_w * anim_speed;
                            st.curr_lift_y += diff_y * anim_speed;
                            needs_anim_redraw = true;
                        } else {
                            st.curr_w = st.target_w;
                            st.curr_lift_y = st.target_lift_y;
                        }

                        // Also trigger redraw while waiting for delay timer to expire!
                        if is_hovered && !delay_passed {
                            needs_anim_redraw = true;
                        }

                        let draw_w = st.curr_w.max(10.0) as usize;
                        let draw_h = (base_button_h as f32 + if is_hovered { hover_scale } else { 0.0 }).max(10.0) as usize;
                        let draw_y = (btn_start_y as i32 + st.curr_lift_y as i32).max(0) as usize;
                        let draw_x = cur_btn_x.max(0.0) as usize;

                        let bg_col = if is_hovered { *bg_hover } else { *bg_normal };

                        // Draw button container box
                        fill_rect(&mut buffer, buf_w, buf_h, draw_x, draw_y, draw_w, draw_h, bg_col);

                        // Icon alignment: PERFECTLY CENTERED when square, glides to pad_l when expanding!
                        let t_expand = ((st.curr_w - square_w) / (expanded_w - square_w).max(1.0)).clamp(0.0, 1.0);
                        let center_icon_offset_x = (square_w - *icon_sz as f32) / 2.0;
                        let aligned_icon_offset_x = pad_l as f32;
                        let anim_icon_offset_x = center_icon_offset_x * (1.0 - t_expand) + aligned_icon_offset_x * t_expand;

                        let icon_x = (draw_x as f32 + anim_icon_offset_x).max(0.0) as usize;
                        let icon_center_y = (draw_y as f32 + (draw_h as f32 - *icon_sz as f32) / 2.0 + icon_off_y as f32).max(0.0) as usize;
                        draw_svg_icon(&mut buffer, buf_w, buf_h, *icon_t, icon_x, icon_center_y, *icon_sz);

                        // Draw Consolas Bold Mono Text CLIPPED to button right boundary
                        let max_text_x = (draw_x + draw_w).saturating_sub(pad_r);
                        let text_x = (draw_x as f32 + anim_icon_offset_x + *icon_sz as f32 + icon_gap as f32).max(0.0) as usize;
                        let text_y = (draw_y as i32 + pad_v as i32 + text_off_y).max(0) as usize;

                        if max_text_x > text_x && t_expand > 0.05 {
                            draw_consolas_bold_text_clipped(&mut buffer, buf_w, buf_h, label, text_x, text_y, max_text_x, 0xFFFFFF, font_sz);
                        }

                        cur_btn_x += st.curr_w + btn_spacing as f32;
                    }

                    // Bottom code bar
                    fill_rect(&mut buffer, buf_w, buf_h, 15, buf_h - 45, buf_w - 30, 36, 0x020617);
                    draw_border(&mut buffer, buf_w, buf_h, 15, buf_h - 45, buf_w - 30, 36, 0x38BDF8);

                    let code_str = format!(
                        "CONFIG -> icon_only:{} | delay_ms:{} | anim_speed:{:.2} | lift_y:{:.1} | copy_sz:{} | font_sz:{:.1} | pad_l:{} | gap:{} | pad_r:{} | pad_v:{}",
                        if icon_only_mode { 1 } else { 0 }, hover_delay_ms, anim_speed, hover_lift_y, icon_sz_copy, font_sz, pad_l, icon_gap, pad_r, pad_v
                    );
                    draw_consolas_bold_text(&mut buffer, buf_w, buf_h, &code_str, 25, buf_h - 32, 0xFACC15, 12.5);

                    buffer.present().unwrap();

                    if needs_anim_redraw {
                        window.request_redraw();
                    }
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
