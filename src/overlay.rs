use anyhow::{Context, Result};
use image::RgbaImage;
use softbuffer::{Context as SbContext, Surface};
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Instant;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, KeyEvent, MouseButton, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowAttributes};

use crate::font::{draw_consolas_bold_text, draw_consolas_bold_text_clipped, draw_svg_icon, measure_consolas_bold_width, IconType};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureAction {
    SaveAndCopy,
    SaveOnly,
    CopyOnly,
    Cancel,
}

/// Read-only summary of the active save settings, drawn on the selection label.
///
/// Deliberately built from primitives rather than `AppConfig`: `overlay.rs` is
/// pulled into `test_bench` by `#[path]` and must not drag in the config module.
#[derive(Debug, Clone, Copy)]
pub struct SaveHint {
    /// 100 means no rescaling, and the label then shows a single size.
    pub scale_percent: u32,
    /// Short uppercase format name, e.g. "WEBP".
    pub format_name: &'static str,
}

impl Default for SaveHint {
    fn default() -> Self {
        Self {
            scale_percent: 100,
            format_name: "WEBP",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    pub fn normalize(x1: i32, y1: i32, x2: i32, y2: i32) -> Self {
        let min_x = x1.min(x2);
        let min_y = y1.min(y2);
        let max_x = x1.max(x2);
        let max_y = y1.max(y2);
        Self {
            x: min_x,
            y: min_y,
            width: (max_x - min_x) as u32,
            height: (max_y - min_y) as u32,
        }
    }

    pub fn contains(&self, px: i32, py: i32) -> bool {
        px >= self.x && px < self.x + self.width as i32 && py >= self.y && py < self.y + self.height as i32
    }
}

#[derive(Debug, Clone)]
struct Button {
    label: &'static str,
    action: CaptureAction,
    icon: IconType,
    icon_size: u32,
    bg_color: u32,
    hover_color: u32,
}

#[derive(Debug, Clone)]
struct AnimatedButtonState {
    curr_w: f32,
    target_w: f32,
    curr_lift_y: f32,
    target_lift_y: f32,
    hover_enter_time: Option<Instant>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InteractionMode {
    None,
    Creating { start_x: i32, start_y: i32 },
    Moving { offset_x: i32, offset_y: i32, width: u32, height: u32 },
}

pub struct SelectionOverlay {
    window: Arc<Window>,
    surface: Surface<Arc<Window>, Arc<Window>>,
    desktop_img: Arc<RgbaImage>,
    bg_buffer: Vec<u32>,
    _min_x: i32,
    _min_y: i32,
    total_w: u32,
    total_h: u32,
    mode: InteractionMode,
    current_pos: (i32, i32),
    active_rect: Option<Rect>,
    buttons_def: Vec<Button>,
    anim_states: Vec<AnimatedButtonState>,
    hovered_button: Option<usize>,
    show_buttons: bool,
    pub result: Option<(RgbaImage, CaptureAction)>,
    pub finished: bool,
    pub debug_mode: bool,
    hint: SaveHint,
    font_size: f32,
    hover_delay_ms: u128,
    anim_speed: f32,
    hover_lift_y: f32,
    pad_l: usize,
    icon_gap: usize,
    pad_r: usize,
    pad_v: usize,
    btn_spacing: usize,
    btn_gap_y: usize,
    dim_pad_h: usize,
    dim_pad_v: usize,
    base_button_h: usize,
    square_w: f32,
}

impl SelectionOverlay {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        event_loop: &ActiveEventLoop,
        desktop_img: RgbaImage,
        min_x: i32,
        min_y: i32,
        total_w: u32,
        total_h: u32,
        debug_mode: bool,
        hint: SaveHint,
    ) -> Result<Self> {
        let desktop_img = Arc::new(desktop_img);

        let attrs = WindowAttributes::default()
            .with_title("Ruuutu Selection")
            .with_decorations(false)
            .with_position(PhysicalPosition::new(min_x, min_y))
            .with_inner_size(PhysicalSize::new(total_w, total_h))
            .with_resizable(false)
            .with_active(true);

        #[allow(deprecated)]
        let window = Arc::new(event_loop.create_window(attrs).context("Failed to create overlay window")?);
        let context = SbContext::new(window.clone()).map_err(|e| anyhow::anyhow!("Context error: {:?}", e))?;
        let mut surface = Surface::new(&context, window.clone()).map_err(|e| anyhow::anyhow!("Surface error: {:?}", e))?;

        surface
            .resize(
                NonZeroU32::new(total_w).unwrap(),
                NonZeroU32::new(total_h).unwrap(),
            )
            .map_err(|e| anyhow::anyhow!("Resize error: {:?}", e))?;

        // Precompute darkened background (50% dimming)
        let mut bg_buffer: Vec<u32> = vec![0; (total_w * total_h) as usize];
        for y in 0..total_h {
            for x in 0..total_w {
                let px = desktop_img.get_pixel(x, y);
                let r = (px[0] as u32) / 2;
                let g = (px[1] as u32) / 2;
                let b = (px[2] as u32) / 2;
                bg_buffer[(y * total_w + x) as usize] = (r << 16) | (g << 8) | b;
            }
        }

        let buttons_def = vec![
            Button { label: "Copiar (C)",       action: CaptureAction::CopyOnly,    icon: IconType::Clipboard, icon_size: 20, bg_color: 0x16A34A, hover_color: 0x22C55E },
            Button { label: "Guardar (S)",      action: CaptureAction::SaveOnly,    icon: IconType::Save,      icon_size: 20, bg_color: 0x2563EB, hover_color: 0x3B82F6 },
            Button { label: "Ambos (Enter)",    action: CaptureAction::SaveAndCopy, icon: IconType::Combo,     icon_size: 20, bg_color: 0x0284C7, hover_color: 0x38BDF8 },
            Button { label: "Cancelar (Esc)",   action: CaptureAction::Cancel,      icon: IconType::Cancel,    icon_size: 20, bg_color: 0xDC2626, hover_color: 0xEF4444 },
        ];

        let pad_v = 7usize;
        let base_button_h = (20usize + pad_v * 2).max(34);
        let square_w = base_button_h as f32;

        let anim_states = vec![
            AnimatedButtonState { curr_w: square_w, target_w: square_w, curr_lift_y: 0.0, target_lift_y: 0.0, hover_enter_time: None };
            4
        ];

        if debug_mode {
            println!("[DEBUG] SelectionOverlay created ({}x{} at {},{}). Debug Mode ENABLED.", total_w, total_h, min_x, min_y);
        }

        Ok(Self {
            window,
            surface,
            desktop_img,
            bg_buffer,
            _min_x: min_x,
            _min_y: min_y,
            total_w,
            total_h,
            mode: InteractionMode::None,
            current_pos: (0, 0),
            active_rect: None,
            buttons_def,
            anim_states,
            hovered_button: None,
            show_buttons: false,
            result: None,
            finished: false,
            debug_mode,
            hint,
            font_size: 17.0,
            hover_delay_ms: 500,
            anim_speed: 0.20,
            hover_lift_y: -2.0,
            pad_l: 8,
            icon_gap: 6,
            pad_r: 10,
            pad_v,
            btn_spacing: 6,
            btn_gap_y: 6,
            dim_pad_h: 10,
            dim_pad_v: 6,
            base_button_h,
            square_w,
        })
    }

    pub fn window_id(&self) -> winit::window::WindowId {
        self.window.id()
    }

    /// Calculates button layout boxes for accurate hit testing.
    fn compute_button_rects(&self) -> Option<Vec<(Rect, usize)>> {
        let rect = self.active_rect?;
        if !self.show_buttons {
            return None;
        }

        let mut btn_start_y = rect.y + rect.height as i32 + self.btn_gap_y as i32;
        if btn_start_y + self.base_button_h as i32 > self.total_h as i32 {
            btn_start_y = (rect.y - self.base_button_h as i32 - self.btn_gap_y as i32).max(6);
        }

        let total_btns_w: f32 = self.anim_states.iter().map(|s| s.curr_w + self.btn_spacing as f32).sum::<f32>() - self.btn_spacing as f32;
        let btn_start_x = (rect.x + rect.width as i32) as f32 - total_btns_w;

        let mut cur_btn_x = btn_start_x;
        let mut list = Vec::with_capacity(4);

        for (idx, _btn) in self.buttons_def.iter().enumerate() {
            let st = &self.anim_states[idx];
            let draw_w = st.curr_w.max(10.0) as u32;
            let draw_h = self.base_button_h as u32;
            let draw_x = cur_btn_x as i32;
            let draw_y = btn_start_y + st.curr_lift_y as i32;

            list.push((Rect { x: draw_x, y: draw_y, width: draw_w, height: draw_h }, idx));
            cur_btn_x += st.curr_w + self.btn_spacing as f32;
        }

        Some(list)
    }

    /// Ends the overlay, cropping the selection out of the capture when there is one.
    ///
    /// A cancel, an empty selection or no selection at all all finish with no result;
    /// `main.rs` turns that into `CaptureAction::Cancel`.
    fn finish_with(&mut self, action: CaptureAction) {
        self.result = match self.active_rect {
            Some(rect)
                if action != CaptureAction::Cancel && rect.width > 0 && rect.height > 0 =>
            {
                let cropped =
                    crop_image(&self.desktop_img, rect.x, rect.y, rect.width, rect.height);
                Some((cropped, action))
            }
            _ => None,
        };
        self.finished = true;
    }

    pub fn handle_event(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::CloseRequested => {
                if self.debug_mode {
                    println!("[DEBUG] CloseRequested received. Finishing overlay.");
                }
                self.result = None;
                self.finished = true;
                true
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key,
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => match logical_key {
                Key::Named(NamedKey::Escape) => {
                    if self.debug_mode {
                        println!("[DEBUG] ESC key pressed. Cancelling capture.");
                    }
                    self.result = None;
                    self.finished = true;
                    true
                }
                Key::Character(ref s) if s.eq_ignore_ascii_case("c") => {
                    if self.debug_mode {
                        println!("[DEBUG] 'C' key pressed. Triggering CopyOnly.");
                    }
                    self.finish_with(CaptureAction::CopyOnly);
                    true
                }
                Key::Character(ref s) if s.eq_ignore_ascii_case("s") => {
                    if self.debug_mode {
                        println!("[DEBUG] 'S' key pressed. Triggering SaveOnly.");
                    }
                    self.finish_with(CaptureAction::SaveOnly);
                    true
                }
                Key::Named(NamedKey::Enter) => {
                    if self.debug_mode {
                        println!("[DEBUG] Enter key pressed. Triggering SaveAndCopy.");
                    }
                    self.finish_with(CaptureAction::SaveAndCopy);
                    true
                }
                _ => false,
            },
            WindowEvent::CursorMoved { position, .. } => {
                self.current_pos = (position.x as i32, position.y as i32);

                match self.mode {
                    InteractionMode::Creating { start_x, start_y } => {
                        self.active_rect = Some(Rect::normalize(start_x, start_y, self.current_pos.0, self.current_pos.1));
                        self.show_buttons = false;
                        self.hovered_button = None;
                    }
                    InteractionMode::Moving { offset_x, offset_y, width, height } => {
                        let max_x = (self.total_w as i32 - width as i32).max(0);
                        let max_y = (self.total_h as i32 - height as i32).max(0);
                        let new_x = (self.current_pos.0 - offset_x).clamp(0, max_x);
                        let new_y = (self.current_pos.1 - offset_y).clamp(0, max_y);
                        self.active_rect = Some(Rect { x: new_x, y: new_y, width, height });
                        self.show_buttons = false;
                        self.hovered_button = None;
                    }
                    InteractionMode::None => {}
                }

                self.window.request_redraw();
                true
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => match state {
                ElementState::Pressed => {
                    if self.debug_mode {
                        println!("[DEBUG] Left Mouse Pressed at ({}, {}). Mode: {:?}, ShowButtons: {}", self.current_pos.0, self.current_pos.1, self.mode, self.show_buttons);
                    }

                    // Direct mathematical hit testing for buttons!
                    if self.show_buttons {
                        if let Some(btn_rects) = self.compute_button_rects() {
                            for (b_rect, idx) in btn_rects {
                                if self.debug_mode {
                                    println!("[DEBUG] Checking Button {} ({}) box [{},{} -> {},{}]: Hit = {}", idx, self.buttons_def[idx].label, b_rect.x, b_rect.y, b_rect.x + b_rect.width as i32, b_rect.y + b_rect.height as i32, b_rect.contains(self.current_pos.0, self.current_pos.1));
                                }

                                if b_rect.contains(self.current_pos.0, self.current_pos.1) {
                                    let action = self.buttons_def[idx].action;
                                    if self.debug_mode {
                                        println!("[DEBUG] ==> BUTTON CLICKED! Index: {}, Action: {:?}", idx, action);
                                    }

                                    self.finish_with(action);
                                    return true;
                                }
                            }
                        }
                    }

                    if let Some(rect) = self.active_rect {
                        if rect.contains(self.current_pos.0, self.current_pos.1) {
                            if self.debug_mode {
                                println!("[DEBUG] Click inside active selection rect. Entering Moving mode.");
                            }
                            self.mode = InteractionMode::Moving {
                                offset_x: self.current_pos.0 - rect.x,
                                offset_y: self.current_pos.1 - rect.y,
                                width: rect.width,
                                height: rect.height,
                            };
                            self.show_buttons = false;
                            self.hovered_button = None;
                            self.window.request_redraw();
                            return true;
                        }
                    }

                    if self.debug_mode {
                        println!("[DEBUG] Click outside buttons and rect. Entering Creating mode.");
                    }
                    self.mode = InteractionMode::Creating {
                        start_x: self.current_pos.0,
                        start_y: self.current_pos.1,
                    };
                    self.active_rect = Some(Rect::normalize(self.current_pos.0, self.current_pos.1, self.current_pos.0, self.current_pos.1));
                    self.show_buttons = false;
                    self.hovered_button = None;
                    self.window.request_redraw();
                    true
                }
                ElementState::Released => {
                    if self.debug_mode {
                        println!("[DEBUG] Left Mouse Released at ({}, {}). Mode: {:?}", self.current_pos.0, self.current_pos.1, self.mode);
                    }

                    match self.mode {
                        InteractionMode::Creating { start_x, start_y } => {
                            let rect = Rect::normalize(start_x, start_y, self.current_pos.0, self.current_pos.1);
                            if rect.width > 5 && rect.height > 5 {
                                self.active_rect = Some(rect);
                                self.show_buttons = true;
                                if self.debug_mode {
                                    println!("[DEBUG] Valid selection created: {}x{} at {},{}. Showing buttons.", rect.width, rect.height, rect.x, rect.y);
                                }
                            } else {
                                self.active_rect = None;
                                self.show_buttons = false;
                            }
                        }
                        InteractionMode::Moving { .. } => {
                            if self.active_rect.is_some() {
                                self.show_buttons = true;
                                if self.debug_mode {
                                    println!("[DEBUG] Selection moved. Showing buttons.");
                                }
                            }
                        }
                        InteractionMode::None => {}
                    }
                    self.mode = InteractionMode::None;
                    self.window.request_redraw();
                    true
                }
            },
            WindowEvent::RedrawRequested => {
                self.redraw();
                true
            }
            _ => false,
        }
    }

    pub fn redraw(&mut self) {
        let now = Instant::now();
        let mut buffer = self.surface.buffer_mut().unwrap();

        buffer.copy_from_slice(&self.bg_buffer);

        if let Some(rect) = self.active_rect {
            let rx2 = (rect.x + rect.width as i32).min(self.total_w as i32);
            let ry2 = (rect.y + rect.height as i32).min(self.total_h as i32);
            let rx1 = rect.x.max(0) as u32;
            let ry1 = rect.y.max(0) as u32;
            let rx2 = rx2.max(0) as u32;
            let ry2 = ry2.max(0) as u32;

            for y in ry1..ry2 {
                for x in rx1..rx2 {
                    let px = self.desktop_img.get_pixel(x, y);
                    let rgb = ((px[0] as u32) << 16) | ((px[1] as u32) << 8) | (px[2] as u32);
                    buffer[(y * self.total_w + x) as usize] = rgb;
                }
            }

            let border_color = 0x00A2FF;
            for x in rx1..rx2 {
                buffer[(ry1 * self.total_w + x) as usize] = border_color;
                if ry2 > 0 {
                    buffer[((ry2 - 1) * self.total_w + x) as usize] = border_color;
                }
            }
            for y in ry1..ry2 {
                buffer[(y * self.total_w + rx1) as usize] = border_color;
                if rx2 > 0 {
                    buffer[(y * self.total_w + rx2 - 1) as usize] = border_color;
                }
            }

            // Dimension label: selection size, the size it will actually be saved at
            // when downscaling is active, and the target format.
            let dim_text = if self.hint.scale_percent >= 100 {
                format!("{} x {} px · {}", rect.width, rect.height, self.hint.format_name)
            } else {
                let out_w = (rect.width * self.hint.scale_percent / 100).max(1);
                let out_h = (rect.height * self.hint.scale_percent / 100).max(1);
                format!(
                    "{} x {} -> {} x {} px · {}",
                    rect.width, rect.height, out_w, out_h, self.hint.format_name
                )
            };
            let label_w = (measure_consolas_bold_width(&dim_text, self.font_size) + self.dim_pad_h * 2) as u32;
            let label_h = (self.font_size as usize + self.dim_pad_v * 2).max(24) as u32;
            let label_x = rx1;
            let label_y = if ry1 >= label_h + 6 { ry1 - label_h - 6 } else { ry1 + 6 };

            draw_filled_rect(&mut buffer, self.total_w as usize, self.total_h as usize, label_x as usize, label_y as usize, label_w as usize, label_h as usize, 0x1E293B);
            draw_consolas_bold_text(
                &mut buffer,
                self.total_w as usize,
                self.total_h as usize,
                &dim_text,
                label_x as usize + self.dim_pad_h,
                label_y as usize + self.dim_pad_v,
                0xFFFFFF,
                self.font_size,
            );

            // Action Buttons
            if self.show_buttons {
                let mut btn_start_y = rect.y + rect.height as i32 + self.btn_gap_y as i32;
                if btn_start_y + self.base_button_h as i32 > self.total_h as i32 {
                    btn_start_y = (rect.y - self.base_button_h as i32 - self.btn_gap_y as i32).max(6);
                }

                let total_btns_w: f32 = self.anim_states.iter().map(|s| s.curr_w + self.btn_spacing as f32).sum::<f32>() - self.btn_spacing as f32;
                let btn_start_x = (rect.x + rect.width as i32) as f32 - total_btns_w;

                let mut cur_btn_x = btn_start_x;
                let mut needs_anim_redraw = false;
                let mut found_hover = None;

                for (idx, btn) in self.buttons_def.iter().enumerate() {
                    let st = &mut self.anim_states[idx];

                    let is_hovered = self.current_pos.0 >= cur_btn_x as i32 && self.current_pos.0 < (cur_btn_x + st.curr_w) as i32 &&
                                     self.current_pos.1 >= (btn_start_y - 10) && self.current_pos.1 < (btn_start_y + self.base_button_h as i32 + 10);

                    if is_hovered {
                        found_hover = Some(idx);
                        if st.hover_enter_time.is_none() {
                            st.hover_enter_time = Some(now);
                        }
                    } else {
                        st.hover_enter_time = None;
                    }

                    let delay_passed = match st.hover_enter_time {
                        Some(t) => now.duration_since(t).as_millis() >= self.hover_delay_ms,
                        None => false,
                    };

                    let text_w = measure_consolas_bold_width(btn.label, self.font_size);
                    let expanded_w = (self.pad_l + btn.icon_size as usize + self.icon_gap + text_w + self.pad_r) as f32;

                    let should_expand = is_hovered && delay_passed;

                    st.target_w = if should_expand { expanded_w } else { self.square_w };
                    st.target_lift_y = if is_hovered { self.hover_lift_y } else { 0.0 };

                    let diff_w = st.target_w - st.curr_w;
                    let diff_y = st.target_lift_y - st.curr_lift_y;

                    if diff_w.abs() > 0.1 || diff_y.abs() > 0.1 {
                        st.curr_w += diff_w * self.anim_speed;
                        st.curr_lift_y += diff_y * self.anim_speed;
                        needs_anim_redraw = true;
                    } else {
                        st.curr_w = st.target_w;
                        st.curr_lift_y = st.target_lift_y;
                    }

                    if is_hovered && !delay_passed {
                        needs_anim_redraw = true;
                    }

                    let draw_w = st.curr_w.max(10.0) as usize;
                    let draw_h = self.base_button_h;
                    let draw_y = (btn_start_y + st.curr_lift_y as i32).max(0) as usize;
                    let draw_x = cur_btn_x.max(0.0) as usize;

                    let bg_col = if is_hovered { btn.hover_color } else { btn.bg_color };

                    draw_filled_rect(
                        &mut buffer,
                        self.total_w as usize,
                        self.total_h as usize,
                        draw_x,
                        draw_y,
                        draw_w,
                        draw_h,
                        bg_col,
                    );

                    // If DEBUG MODE: Draw bright magenta border around button hit box!
                    if self.debug_mode {
                        draw_border_rect(&mut buffer, self.total_w as usize, self.total_h as usize, draw_x, draw_y, draw_w, draw_h, 0xFF00FF);
                    }

                    let t_expand = ((st.curr_w - self.square_w) / (expanded_w - self.square_w).max(1.0)).clamp(0.0, 1.0);
                    let center_icon_offset_x = (self.square_w - btn.icon_size as f32) / 2.0;
                    let aligned_icon_offset_x = self.pad_l as f32;
                    let anim_icon_offset_x = center_icon_offset_x * (1.0 - t_expand) + aligned_icon_offset_x * t_expand;

                    let icon_x = (draw_x as f32 + anim_icon_offset_x).max(0.0) as usize;
                    let icon_center_y = (draw_y as f32 + (draw_h as f32 - btn.icon_size as f32) / 2.0).max(0.0) as usize;
                    draw_svg_icon(
                        &mut buffer,
                        self.total_w as usize,
                        self.total_h as usize,
                        btn.icon,
                        icon_x,
                        icon_center_y,
                        btn.icon_size,
                    );

                    let max_text_x = (draw_x + draw_w).saturating_sub(self.pad_r);
                    let text_x = (draw_x as f32 + anim_icon_offset_x + btn.icon_size as f32 + self.icon_gap as f32).max(0.0) as usize;
                    let text_y = (draw_y + self.pad_v).max(0);

                    if max_text_x > text_x && t_expand > 0.05 {
                        draw_consolas_bold_text_clipped(
                            &mut buffer,
                            self.total_w as usize,
                            self.total_h as usize,
                            btn.label,
                            text_x,
                            text_y,
                            max_text_x,
                            0xFFFFFF,
                            self.font_size,
                        );
                    }

                    cur_btn_x += st.curr_w + self.btn_spacing as f32;
                }

                self.hovered_button = found_hover;

                if needs_anim_redraw {
                    self.window.request_redraw();
                }
            }
        }

        // Draw ON-SCREEN DEBUG TELEMETRY HUD in top-left corner
        if self.debug_mode {
            let hud_w = 480usize;
            let hud_h = 115usize;
            draw_filled_rect(&mut buffer, self.total_w as usize, self.total_h as usize, 20, 20, hud_w, hud_h, 0x020617);
            draw_border_rect(&mut buffer, self.total_w as usize, self.total_h as usize, 20, 20, hud_w, hud_h, 0x22C55E);

            draw_consolas_bold_text(&mut buffer, self.total_w as usize, self.total_h as usize, "[DEBUG MODE ACTIVE]", 30, 26, 0x22C55E, 14.0);
            
            let pos_str = format!("Cursor: ({}, {}) | Mode: {:?}", self.current_pos.0, self.current_pos.1, self.mode);
            draw_consolas_bold_text(&mut buffer, self.total_w as usize, self.total_h as usize, &pos_str, 30, 48, 0xFACC15, 12.5);

            let btn_str = format!("ShowButtons: {} | HoveredBtn: {:?}", self.show_buttons, self.hovered_button);
            draw_consolas_bold_text(&mut buffer, self.total_w as usize, self.total_h as usize, &btn_str, 30, 68, 0x38BDF8, 12.5);

            let click_hint = "Haz clic en [Copiar] para ver telemetria en consola!";
            draw_consolas_bold_text(&mut buffer, self.total_w as usize, self.total_h as usize, click_hint, 30, 88, 0xE2E8F0, 12.0);
        }

        buffer.present().unwrap();
    }
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

fn draw_border_rect(buffer: &mut [u32], buf_w: usize, buf_h: usize, x: usize, y: usize, w: usize, h: usize, color: u32) {
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

/// Copies the selection out of the desktop capture.
///
/// The origin is clamped into the image and every span uses saturating arithmetic: the
/// callers derive `x`/`y` from an `i32` rectangle, so a negative coordinate would arrive
/// here as a huge `u32`, and `max_x - x` would then underflow straight into an
/// out-of-bounds `get_pixel`. With `panic = "abort"` in release that is a silent process
/// death, not a message.
///
/// Anything outside the source stays as the transparent pixels `RgbaImage::new` starts with.
fn crop_image(img: &RgbaImage, x: i32, y: i32, width: u32, height: u32) -> RgbaImage {
    let mut cropped = RgbaImage::new(width.max(1), height.max(1));

    // A negative origin means the selection starts off the top-left of the capture:
    // skip that many pixels into the destination instead of reading before the source.
    let skip_x = x.min(0).unsigned_abs();
    let skip_y = y.min(0).unsigned_abs();
    let src_x = x.max(0) as u32;
    let src_y = y.max(0) as u32;

    let copy_w = width
        .saturating_sub(skip_x)
        .min(img.width().saturating_sub(src_x));
    let copy_h = height
        .saturating_sub(skip_y)
        .min(img.height().saturating_sub(src_y));

    for cy in 0..copy_h {
        for cx in 0..copy_w {
            let pixel = *img.get_pixel(src_x + cx, src_y + cy);
            cropped.put_pixel(skip_x + cx, skip_y + cy, pixel);
        }
    }
    cropped
}
