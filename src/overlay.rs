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

/// The overlay's pixel metrics at a given interface scale.
///
/// Pure and window-free on purpose: it is the whole of the scaling arithmetic, so it can
/// be unit tested without a desktop session. Everything here is the 1x design multiplied
/// by the user's text scale — the glyphs cannot grow alone, they would clip against a
/// fixed 34 px button, so the boxes that hold the text scale with it.
#[derive(Debug, Clone, Copy, PartialEq)]
struct OverlayMetrics {
    font_size: f32,
    hover_lift_y: f32,
    icon_size: u32,
    pad_l: usize,
    icon_gap: usize,
    pad_r: usize,
    pad_v: usize,
    btn_spacing: usize,
    btn_gap_y: usize,
    dim_pad_h: usize,
    dim_pad_v: usize,
    min_dim_label_h: usize,
    base_button_h: usize,
}

impl OverlayMetrics {
    /// `text_scale` is a multiplier; anything outside 0.5x..=2x is clamped, so a corrupt
    /// config cannot produce a zero-sized or absurd interface.
    fn new(text_scale: f32) -> Self {
        let s = if text_scale.is_finite() { text_scale.clamp(0.5, 2.0) } else { 1.0 };
        // Rounded, then floored at 1: at 0.5x several of these would otherwise land on 0
        // and collapse the padding entirely.
        let px = |v: f32| ((v * s).round() as usize).max(1);

        let icon_size = ((20.0 * s).round() as u32).max(1);
        let pad_v = px(7.0);

        Self {
            font_size: 17.0 * s,
            hover_lift_y: -2.0 * s,
            icon_size,
            pad_l: px(8.0),
            icon_gap: px(6.0),
            pad_r: px(10.0),
            pad_v,
            btn_spacing: px(6.0),
            btn_gap_y: px(6.0),
            dim_pad_h: px(10.0),
            dim_pad_v: px(6.0),
            min_dim_label_h: px(24.0),
            // Tall enough for the icon with its padding, never below the scaled minimum.
            base_button_h: (icon_size as usize + pad_v * 2).max(px(34.0)),
        }
    }
}

/// One button's geometry for a frame, produced by [`SelectionOverlay::button_layouts`].
///
/// **The single source of truth for the button row.** Painting, hover and click all read
/// it, so they cannot drift apart. They used to be three independent calculations, and
/// the hover test had already diverged: it carried ±10 px of vertical slack and ignored
/// the hover lift, so a band above and below each button lit the button up while a click
/// there missed it, fell through to "start a new selection" and wiped the selection the
/// user had just made.
#[derive(Debug, Clone, Copy)]
struct ButtonLayout {
    /// The box painted on screen, hover lift included.
    draw: Rect,
    /// The box hover and click test against. Always covers `draw`.
    hit: Rect,
    idx: usize,
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
    /// Floor for the dimension label's height, so small text still gets a readable chip.
    min_dim_label_h: usize,
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
        text_scale: f32,
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

        // `capture.rs` substitutes 1920x1080 when the metrics come back as zero, so this
        // holds today — but the guarantee lives in another module, so it is checked here
        // rather than unwrapped.
        let (Some(nz_w), Some(nz_h)) = (NonZeroU32::new(total_w), NonZeroU32::new(total_h)) else {
            anyhow::bail!("Refusing to open an overlay on an empty desktop ({}x{})", total_w, total_h);
        };

        surface
            .resize(nz_w, nz_h)
            .map_err(|e| anyhow::anyhow!("Resize error: {:?}", e))?;

        // Precompute darkened background (50% dimming).
        //
        // Walks the raw RGBA bytes instead of calling `get_pixel` per pixel: this runs
        // between the hotkey and the overlay appearing, so it is felt directly as latency,
        // and it is two million bounds-checked calls at 1080p — eight at 4K.
        let mut bg_buffer: Vec<u32> = Vec::with_capacity((total_w * total_h) as usize);
        bg_buffer.extend(desktop_img.as_raw().chunks_exact(4).map(|px| {
            let r = (px[0] as u32) / 2;
            let g = (px[1] as u32) / 2;
            let b = (px[2] as u32) / 2;
            (r << 16) | (g << 8) | b
        }));
        // `redraw` copies this straight into the softbuffer, which requires the exact
        // length; padding here keeps a mismatched capture from panicking every frame.
        bg_buffer.resize((total_w * total_h) as usize, 0);

        let m = OverlayMetrics::new(text_scale);

        let buttons_def = vec![
            Button { label: "Copiar (C)",       action: CaptureAction::CopyOnly,    icon: IconType::Clipboard, icon_size: m.icon_size, bg_color: 0x16A34A, hover_color: 0x22C55E },
            Button { label: "Guardar (S)",      action: CaptureAction::SaveOnly,    icon: IconType::Save,      icon_size: m.icon_size, bg_color: 0x2563EB, hover_color: 0x3B82F6 },
            Button { label: "Ambos (Enter)",    action: CaptureAction::SaveAndCopy, icon: IconType::Combo,     icon_size: m.icon_size, bg_color: 0x0284C7, hover_color: 0x38BDF8 },
            Button { label: "Cancelar (Esc)",   action: CaptureAction::Cancel,      icon: IconType::Cancel,    icon_size: m.icon_size, bg_color: 0xDC2626, hover_color: 0xEF4444 },
        ];

        let square_w = m.base_button_h as f32;

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
            font_size: m.font_size,
            // Timing, not geometry: the hover delay and the easing rate stay put.
            hover_delay_ms: 500,
            anim_speed: 0.20,
            hover_lift_y: m.hover_lift_y,
            pad_l: m.pad_l,
            icon_gap: m.icon_gap,
            pad_r: m.pad_r,
            pad_v: m.pad_v,
            btn_spacing: m.btn_spacing,
            btn_gap_y: m.btn_gap_y,
            dim_pad_h: m.dim_pad_h,
            dim_pad_v: m.dim_pad_v,
            min_dim_label_h: m.min_dim_label_h,
            base_button_h: m.base_button_h,
            square_w,
        })
    }

    pub fn window_id(&self) -> winit::window::WindowId {
        self.window.id()
    }

    /// Where each button is laid out for one frame.
    fn button_layouts(&self) -> Option<Vec<ButtonLayout>> {
        let rect = self.active_rect?;
        if !self.show_buttons {
            return None;
        }

        let row_y = buttons_row_y(rect, self.base_button_h, self.btn_gap_y, self.total_h);

        // The hit box spans the whole vertical travel of the lift, so it is the same box
        // whatever the animation is doing. A box that moved with the lift would shrink
        // away from a cursor sitting on the bottom edge, unhover it, drop it back and
        // hover it again, frame after frame.
        let lift_span = self.hover_lift_y.abs().ceil() as u32;
        let hit_y = row_y - lift_span as i32;
        let hit_h = self.base_button_h as u32 + lift_span;

        let total_btns_w: f32 = self.anim_states.iter().map(|s| s.curr_w + self.btn_spacing as f32).sum::<f32>() - self.btn_spacing as f32;
        let mut cur_btn_x = buttons_row_x(rect.x + rect.width as i32, total_btns_w, self.total_w);
        let mut list = Vec::with_capacity(self.buttons_def.len());

        for idx in 0..self.buttons_def.len() {
            let st = &self.anim_states[idx];
            let x = cur_btn_x as i32;
            let width = st.curr_w.max(10.0) as u32;

            list.push(ButtonLayout {
                draw: Rect {
                    x,
                    y: row_y + st.curr_lift_y as i32,
                    width,
                    height: self.base_button_h as u32,
                },
                hit: Rect { x, y: hit_y, width, height: hit_h },
                idx,
            });
            cur_btn_x += st.curr_w + self.btn_spacing as f32;
        }

        Some(list)
    }

    /// Index of the button under the cursor, from the boxes that were last painted.
    fn hovered_button_at(&self, layouts: &[ButtonLayout]) -> Option<usize> {
        layouts
            .iter()
            .find(|l| l.hit.contains(self.current_pos.0, self.current_pos.1))
            .map(|l| l.idx)
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

                    // Hit tested against the same boxes that were painted and hovered.
                    if let Some(layouts) = self.button_layouts() {
                        if self.debug_mode {
                            for l in &layouts {
                                println!("[DEBUG] Checking Button {} ({}) box [{},{} -> {},{}]: Hit = {}", l.idx, self.buttons_def[l.idx].label, l.hit.x, l.hit.y, l.hit.x + l.hit.width as i32, l.hit.y + l.hit.height as i32, l.hit.contains(self.current_pos.0, self.current_pos.1));
                            }
                        }

                        if let Some(idx) = self.hovered_button_at(&layouts) {
                            let action = self.buttons_def[idx].action;
                            if self.debug_mode {
                                println!("[DEBUG] ==> BUTTON CLICKED! Index: {}, Action: {:?}", idx, action);
                            }

                            self.finish_with(action);
                            return true;
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

        // Phase 1 — button state and layout, *before* the surface buffer exists.
        //
        // `buffer_mut()` borrows `self` mutably for as long as the returned buffer lives,
        // so past that point nothing taking `&self` can be called and only direct field
        // access works. That is exactly why this arithmetic used to sit inlined in the
        // painting code, duplicated from `button_layouts`. Doing it up front lets both
        // share one implementation.
        let mut needs_anim_redraw = false;
        let mut layouts: Vec<ButtonLayout> = Vec::new();
        let mut hovered = None;

        if self.show_buttons && self.active_rect.is_some() {
            // Hover is decided against the layout as it was last painted: that is the row
            // the user is actually pointing at.
            hovered = self
                .button_layouts()
                .map(|last| self.hovered_button_at(&last))
                .unwrap_or(None);

            for (idx, btn) in self.buttons_def.iter().enumerate() {
                let is_hovered = hovered == Some(idx);
                let expanded_w = expanded_button_w(btn, self.font_size, self.pad_l, self.icon_gap, self.pad_r);
                let st = &mut self.anim_states[idx];

                if is_hovered {
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

                st.target_w = if is_hovered && delay_passed { expanded_w } else { self.square_w };
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

                // Still counting down to the expansion: keep the frames coming, or the
                // delay would never elapse with the cursor held still.
                if is_hovered && !delay_passed {
                    needs_anim_redraw = true;
                }
            }

            // Laid out again so the frame about to be painted reflects this tick.
            layouts = self.button_layouts().unwrap_or_default();
            self.hovered_button = hovered;
        }

        // Phase 2 — painting.
        //
        // Both of these fail when the surface goes out from under us — a monitor
        // unplugged, a resolution change or an RDP session reconnecting while the overlay
        // is open. Skipping the frame lets the next one retry; unwrapping would abort the
        // process outright, and with `panic = "abort"` in release, silently.
        let Ok(mut buffer) = self.surface.buffer_mut() else {
            return;
        };

        if buffer.len() != self.bg_buffer.len() {
            return;
        }
        buffer.copy_from_slice(&self.bg_buffer);

        if let Some(rect) = self.active_rect {
            // Clamped to the capture as well as to the window: the two are the same size
            // in practice, but the row indexing below reads `desktop_img` and writes
            // `buffer`, so it must stay inside both.
            let limit_w = self.total_w.min(self.desktop_img.width());
            let limit_h = self.total_h.min(self.desktop_img.height());

            let rx1 = (rect.x.max(0) as u32).min(limit_w);
            let ry1 = (rect.y.max(0) as u32).min(limit_h);
            let rx2 = ((rect.x + rect.width as i32).max(0) as u32).min(limit_w);
            let ry2 = ((rect.y + rect.height as i32).max(0) as u32).min(limit_h);

            // Undim the selection by copying rows straight out of the capture. Row slices
            // rather than `get_pixel`: this is the per-frame cost of the selection, and a
            // full-screen drag was making a bounds-checked call per pixel per frame.
            let src = self.desktop_img.as_raw();
            for y in ry1..ry2 {
                let row = (y * self.total_w) as usize;
                for x in rx1..rx2 {
                    let i = (row + x as usize) * 4;
                    buffer[row + x as usize] =
                        ((src[i] as u32) << 16) | ((src[i + 1] as u32) << 8) | (src[i + 2] as u32);
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
            let label_h = (self.font_size as usize + self.dim_pad_v * 2).max(self.min_dim_label_h) as u32;
            let label_x = rx1;
            // The button row is the other thing that wants the strip above the selection,
            // so the label is placed around whatever band the row ended up occupying.
            let button_band = layouts.first().map(|l| (l.hit.y, l.hit.height as i32));
            let label_y = dimension_label_y(
                rect.y,
                label_h as i32,
                self.btn_gap_y as i32,
                button_band,
            )
            .max(0) as u32;

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

            // Action Buttons, from the layout computed in phase 1.
            for layout in &layouts {
                let btn = &self.buttons_def[layout.idx];
                let st = &self.anim_states[layout.idx];
                let is_hovered = hovered == Some(layout.idx);

                let draw_x = layout.draw.x.max(0) as usize;
                let draw_y = layout.draw.y.max(0) as usize;
                let draw_w = layout.draw.width as usize;
                let draw_h = layout.draw.height as usize;

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
                    draw_border_rect(&mut buffer, self.total_w as usize, self.total_h as usize, layout.hit.x.max(0) as usize, layout.hit.y.max(0) as usize, layout.hit.width as usize, layout.hit.height as usize, 0xFF00FF);
                }

                let expanded_w = expanded_button_w(btn, self.font_size, self.pad_l, self.icon_gap, self.pad_r);
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
                let text_y = draw_y + self.pad_v;

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

        if let Err(e) = buffer.present() {
            if self.debug_mode {
                println!("[DEBUG] Dropped a frame, present failed: {:?}", e);
            }
        }

        if needs_anim_redraw {
            self.window.request_redraw();
        }
    }
}

/// Left edge of the button row.
///
/// The row hangs off the **right** edge of the selection, so a selection that is narrower
/// than the row and sits near the left of the screen pushes it off-screen. It is slid back
/// inside rather than left to overflow.
///
/// This has to happen to the row as a whole. Clamping each button's own draw position was
/// the bug: every button with a negative x was pinned to 0 and they were all painted on
/// top of each other. Handling it here also means the hit boxes move with the drawn ones,
/// since both come out of `button_layouts`.
///
/// A row wider than the whole screen — not reachable with four buttons, but cheap to
/// define — starts at 0 and overflows to the right.
fn buttons_row_x(selection_right: i32, row_w: f32, total_w: u32) -> f32 {
    let max_start = (total_w as f32 - row_w).max(0.0);
    (selection_right as f32 - row_w).clamp(0.0, max_start)
}

/// Top edge of the button row, before each button's own hover lift.
///
/// Normally sits under the selection; flips above it when the selection reaches the
/// bottom of the screen and the row would not fit below.
fn buttons_row_y(rect: Rect, button_h: usize, gap: usize, total_h: u32) -> i32 {
    let below = rect.y + rect.height as i32 + gap as i32;
    if below + button_h as i32 > total_h as i32 {
        (rect.y - button_h as i32 - gap as i32).max(6)
    } else {
        below
    }
}

/// Top edge of the dimension label ("1920 x 1080 px · WEBP").
///
/// It normally sits just above the selection — but so does the button row whenever the
/// selection is close enough to the bottom of the screen to make the row flip up. Both
/// claimed that strip independently and the buttons were painted straight over the
/// label. When the row is up there, the label goes above the row instead.
///
/// `button_band` is the row's `(top, height)`, taken from the hit boxes so it covers the
/// buttons in every animation state, or `None` while there is no row (mid-drag).
///
/// Three candidate positions are tried in order of preference and the first one that is
/// on screen and clear of the row wins. A single "is the row above me?" test is not
/// enough: a selection only a few pixels tall at the *top* of the screen has no room
/// above either, and the label dropped inside it then spilled out of the bottom and
/// under a row sitting below.
fn dimension_label_y(
    sel_top: i32,
    label_h: i32,
    gap: i32,
    button_band: Option<(i32, i32)>,
) -> i32 {
    let clear = |y: i32| {
        y >= 0
            && button_band.is_none_or(|(row_y, row_h)| y + label_h <= row_y || y >= row_y + row_h)
    };

    // Only a row that flipped above the selection competes for the strip above it.
    let ceiling = button_band
        .filter(|(row_y, _)| *row_y < sel_top)
        .map_or(sel_top, |(row_y, _)| row_y);

    // Below the row: always on screen and always clear, so it is the guaranteed fallback.
    let below_row = button_band.map_or(sel_top + gap, |(row_y, row_h)| (row_y + row_h + gap).max(0));

    for candidate in [ceiling - label_h - gap, sel_top + gap] {
        if clear(candidate) {
            return candidate;
        }
    }
    below_row
}

/// Width a button needs once its label is showing: padding, icon, gap, text, padding.
fn expanded_button_w(btn: &Button, font_size: f32, pad_l: usize, icon_gap: usize, pad_r: usize) -> f32 {
    let text_w = measure_consolas_bold_width(btn.label, font_size);
    (pad_l + btn.icon_size as usize + icon_gap + text_w + pad_r) as f32
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The scales offered by the tray menu.
    const SCALES: [f32; 6] = [0.5, 0.75, 1.0, 1.25, 1.5, 2.0];

    /// 1x must still be the interface that was hand-tuned in `margin_editor`.
    #[test]
    fn one_x_matches_the_original_design() {
        let m = OverlayMetrics::new(1.0);
        assert_eq!(m.font_size, 17.0);
        assert_eq!(m.hover_lift_y, -2.0);
        assert_eq!(m.icon_size, 20);
        assert_eq!(m.pad_l, 8);
        assert_eq!(m.icon_gap, 6);
        assert_eq!(m.pad_r, 10);
        assert_eq!(m.pad_v, 7);
        assert_eq!(m.btn_spacing, 6);
        assert_eq!(m.btn_gap_y, 6);
        assert_eq!(m.dim_pad_h, 10);
        assert_eq!(m.dim_pad_v, 6);
        assert_eq!(m.min_dim_label_h, 24);
        assert_eq!(m.base_button_h, 34);
    }

    /// The text is drawn at `draw_y + pad_v` and is about `font_size` tall, so this is
    /// what keeps a scaled label inside its button instead of spilling over the edge.
    #[test]
    fn the_label_fits_inside_the_button_at_every_scale() {
        for s in SCALES {
            let m = OverlayMetrics::new(s);
            assert!(
                m.font_size.ceil() as usize + m.pad_v <= m.base_button_h,
                "text overflows the button at {}x: {} + {} > {}",
                s, m.font_size, m.pad_v, m.base_button_h
            );
            assert!(
                m.icon_size as usize + m.pad_v * 2 <= m.base_button_h,
                "icon overflows the button at {}x", s
            );
        }
    }

    /// No metric may collapse to zero, which is what a naive truncation would do to the
    /// paddings at 0.5x and would silently glue the icon to the button edge.
    #[test]
    fn no_metric_collapses_at_the_smallest_scale() {
        let m = OverlayMetrics::new(0.5);
        for (name, v) in [
            ("pad_l", m.pad_l), ("icon_gap", m.icon_gap), ("pad_r", m.pad_r),
            ("pad_v", m.pad_v), ("btn_spacing", m.btn_spacing), ("btn_gap_y", m.btn_gap_y),
            ("dim_pad_h", m.dim_pad_h), ("dim_pad_v", m.dim_pad_v),
            ("min_dim_label_h", m.min_dim_label_h), ("base_button_h", m.base_button_h),
        ] {
            assert!(v >= 1, "{} collapsed to {} at 0.5x", name, v);
        }
        assert!(m.icon_size >= 1);
        assert!(m.font_size > 0.0);
    }

    #[test]
    fn metrics_grow_monotonically_with_the_scale() {
        let mut prev = OverlayMetrics::new(SCALES[0]);
        for s in &SCALES[1..] {
            let m = OverlayMetrics::new(*s);
            assert!(m.font_size > prev.font_size, "font_size shrank at {}x", s);
            assert!(m.base_button_h >= prev.base_button_h, "button height shrank at {}x", s);
            assert!(m.icon_size >= prev.icon_size, "icon shrank at {}x", s);
            prev = m;
        }
    }

    /// A corrupt or absurd config must not produce a zero-sized or unusable interface.
    #[test]
    fn out_of_range_scales_are_clamped() {
        assert_eq!(OverlayMetrics::new(0.01), OverlayMetrics::new(0.5));
        assert_eq!(OverlayMetrics::new(99.0), OverlayMetrics::new(2.0));
        assert_eq!(OverlayMetrics::new(f32::NAN), OverlayMetrics::new(1.0));
        assert_eq!(OverlayMetrics::new(-3.0), OverlayMetrics::new(0.5));
    }

    /// The hit box must cover the drawn box in every animation state, or a cursor on the
    /// edge would unhover the button as it lifts and oscillate.
    #[test]
    fn the_hit_box_covers_the_full_lift_travel() {
        for s in SCALES {
            let m = OverlayMetrics::new(s);
            let lift_span = m.hover_lift_y.abs().ceil() as i32;
            let row_y = 500;

            // Resting, and fully lifted.
            for lift in [0, -lift_span] {
                let draw_top = row_y + lift;
                let draw_bottom = draw_top + m.base_button_h as i32;
                let hit_top = row_y - lift_span;
                let hit_bottom = hit_top + (m.base_button_h as i32 + lift_span);
                assert!(hit_top <= draw_top && hit_bottom >= draw_bottom,
                    "hit box misses the drawn box at {}x with lift {}", s, lift);
            }
        }
    }

    /// Width of the row when every button is at rest, which is its narrowest.
    fn resting_row_w(m: &OverlayMetrics) -> f32 {
        4.0 * m.base_button_h as f32 + 3.0 * m.btn_spacing as f32
    }

    /// The reported bug: at 2x, a small selection near the left edge pushed the
    /// right-anchored row off-screen, every button was clamped to x = 0 on its own, and
    /// they were all painted on top of each other.
    #[test]
    fn a_narrow_selection_on_the_left_does_not_push_the_row_off_screen() {
        for s in SCALES {
            let m = OverlayMetrics::new(s);
            let row_w = resting_row_w(&m);

            // Selection 100 px wide hard against the left edge.
            let x = buttons_row_x(100, row_w, 1920);
            assert!(x >= 0.0, "row starts off-screen at {}x: {}", s, x);
            assert!(x + row_w <= 1920.0, "row overflows to the right at {}x", s);
        }
    }

    /// Sliding the row back must not disturb the ordinary case: while it fits, the row
    /// stays anchored to the right edge of the selection.
    #[test]
    fn the_row_stays_anchored_to_the_selection_when_it_fits() {
        let m = OverlayMetrics::new(1.0);
        let row_w = resting_row_w(&m);

        for selection_right in [500, 900, 1400, 1920] {
            let x = buttons_row_x(selection_right, row_w, 1920);
            assert_eq!(x, selection_right as f32 - row_w, "row moved when it did not need to");
        }
    }

    /// Buttons are laid out left to right from the row origin, so none may start before
    /// the one before it ends. This is what "overlapping" meant.
    #[test]
    fn buttons_never_overlap_wherever_the_row_lands() {
        for s in SCALES {
            let m = OverlayMetrics::new(s);
            let widths = [m.base_button_h as f32; 4];
            let row_w = resting_row_w(&m);

            for selection_right in [50, 100, 300, 960, 1920] {
                let mut x = buttons_row_x(selection_right, row_w, 1920);
                let mut prev_end = f32::NEG_INFINITY;
                for w in widths {
                    assert!(x >= prev_end, "buttons overlap at {}x, selection right {}", s, selection_right);
                    prev_end = x + w;
                    x += w + m.btn_spacing as f32;
                }
            }
        }
    }

    /// A row wider than the screen has nowhere to go: it starts at 0 rather than at a
    /// negative offset.
    #[test]
    fn a_row_wider_than_the_screen_starts_at_zero() {
        assert_eq!(buttons_row_x(1920, 2500.0, 1920), 0.0);
        assert_eq!(buttons_row_x(10, 2500.0, 1920), 0.0);
    }

    /// The row drops below the selection, and flips above it when there is no room.
    #[test]
    fn the_row_flips_above_a_selection_at_the_bottom() {
        let m = OverlayMetrics::new(1.0);
        let h = m.base_button_h;
        let gap = m.btn_gap_y;

        let roomy = Rect { x: 0, y: 100, width: 200, height: 200 };
        assert_eq!(buttons_row_y(roomy, h, gap, 1080), 300 + gap as i32);

        // Selection running to the bottom edge: the row has to go above it.
        let tight = Rect { x: 0, y: 100, width: 200, height: 980 };
        let y = buttons_row_y(tight, h, gap, 1080);
        assert!(y < 100, "row should sit above the selection, got {}", y);
        assert!(y >= 6, "row should stay on screen, got {}", y);
    }

    /// Do two vertical bands `(top, height)` share a pixel?
    fn bands_overlap(a: (i32, i32), b: (i32, i32)) -> bool {
        a.0 < b.0 + b.1 && b.0 < a.0 + a.1
    }

    /// The reported bug: a small selection near the bottom makes the button row flip
    /// above the selection, where the dimension label already lived, and the buttons were
    /// painted straight over it.
    #[test]
    fn the_label_never_sits_under_the_button_row() {
        for s in SCALES {
            let m = OverlayMetrics::new(s);
            let label_h = (m.font_size as usize + m.dim_pad_v * 2).max(m.min_dim_label_h) as i32;
            let lift = m.hover_lift_y.abs().ceil() as i32;
            let row_h = m.base_button_h as i32 + lift;
            let gap = m.btn_gap_y as i32;

            // Swept rather than sampled: the first attempt at this only special-cased a
            // row that had flipped *above* the selection, and missed that a very short
            // selection at the top has no room above either.
            for sel_h in [6, 20, 60, 200, 600, 1080] {
                for sel_top in (0..=(1080 - sel_h).max(0)).step_by(20) {
                    let rect = Rect { x: 100, y: sel_top, width: 200, height: sel_h as u32 };
                    let row_y = buttons_row_y(rect, m.base_button_h, m.btn_gap_y, 1080) - lift;
                    let band = (row_y, row_h);

                    let label_y = dimension_label_y(sel_top, label_h, gap, Some(band));

                    assert!(
                        !bands_overlap((label_y, label_h), band),
                        "label and buttons overlap at {}x, selection top {} height {}: \
                         label [{}, {}) vs row [{}, {})",
                        s, sel_top, sel_h,
                        label_y, label_y + label_h, band.0, band.0 + band.1
                    );
                    assert!(label_y >= 0, "label off the top at {}x: {}", s, label_y);
                }
            }
        }
    }

    /// The ordinary case must not move: buttons below, label just above the selection.
    #[test]
    fn the_label_stays_above_the_selection_when_the_row_is_below() {
        let m = OverlayMetrics::new(1.0);
        let label_h = 29;
        let gap = m.btn_gap_y as i32;
        let sel_top = 400;
        // Row below the selection.
        let band = Some((sel_top + 200 + gap, m.base_button_h as i32));

        assert_eq!(dimension_label_y(sel_top, label_h, gap, band), sel_top - label_h - gap);
        // And identically with no row at all, mid-drag.
        assert_eq!(dimension_label_y(sel_top, label_h, gap, None), sel_top - label_h - gap);
    }

    /// A selection running the full height of the screen leaves nothing above it: the
    /// label drops inside rather than off the top edge.
    #[test]
    fn the_label_drops_inside_when_there_is_no_room_above() {
        let m = OverlayMetrics::new(1.0);
        let label_h = 29;
        let gap = m.btn_gap_y as i32;
        let row_h = m.base_button_h as i32;

        // Selection from the very top, row forced up to its floor of 6.
        let band = Some((6, row_h));
        let y = dimension_label_y(0, label_h, gap, band);
        assert!(y >= 0);
        assert!(!bands_overlap((y, label_h), (6, row_h)));

        // No row, no room above.
        assert_eq!(dimension_label_y(0, label_h, gap, None), gap);
    }

    /// Crop must never read outside the capture, whatever rectangle it is handed.
    #[test]
    fn crop_clamps_instead_of_reading_out_of_bounds() {
        let img = RgbaImage::new(40, 30);
        for (x, y, w, h) in [
            (0, 0, 40, 30), (35, 25, 20, 20), (-10, -10, 20, 20),
            (100, 100, 10, 10), (-50, 5, 10, 10), (0, 0, 1, 1),
        ] {
            let out = crop_image(&img, x, y, w, h);
            assert_eq!(out.width(), w.max(1));
            assert_eq!(out.height(), h.max(1));
        }
    }

    #[test]
    fn rect_normalize_is_orientation_independent() {
        let a = Rect::normalize(10, 20, 110, 220);
        let b = Rect::normalize(110, 220, 10, 20);
        assert_eq!(a, b);
        assert_eq!(a, Rect { x: 10, y: 20, width: 100, height: 200 });
        assert!(a.contains(10, 20));
        assert!(!a.contains(110, 220));
    }
}
