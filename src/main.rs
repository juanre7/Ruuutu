// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 juanre7

// GUI subsystem: double-clicking ruuutu.exe goes straight to the tray without flashing a
// console window. `console::attach_parent_console()` reattaches stdout/stderr when the binary
// *is* launched from a terminal, so `cargo run` and `ruuutu.exe --debug` still print.
#![windows_subsystem = "windows"]

use anyhow::Result;
use global_hotkey::{GlobalHotKeyEvent, HotKeyState};
use image::RgbaImage;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use tray_icon::menu::MenuEvent;
use tray_icon::{MouseButton, MouseButtonState, TrayIconEvent};
use windows_sys::Win32::System::Ole::OleInitialize;
use windows_sys::Win32::UI::HiDpi::{
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::WindowId;

mod capture;
mod clipboard;
mod config;
mod console;
mod font;
mod hotkey;
mod icon;
mod overlay;
mod save_dialog;
mod storage;
mod tray;

use capture::capture_desktop;
use clipboard::copy_to_clipboard;
use config::{set_autostart_enabled, AppConfig};
use hotkey::{HotkeyManager, PRTSCN_TRIGGERED};
use overlay::{CaptureAction, SaveHint, SelectionOverlay};
use storage::save_image;
use tray::SystemTray;

struct RuuutuApp {
    is_cli_mode: bool,
    debug_mode: bool,
    config: AppConfig,
    hotkey_mgr: Option<HotkeyManager>,
    tray_mgr: Option<SystemTray>,
    overlay: Option<SelectionOverlay>,
    pending_action: Option<(RgbaImage, CaptureAction)>,
}

impl ApplicationHandler for RuuutuApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.is_cli_mode && self.overlay.is_none() && self.pending_action.is_none() {
            if let Err(e) = self.spawn_overlay(event_loop) {
                eprintln!("Error starting CLI capture: {:?}", e);
                event_loop.exit();
            }
            return;
        }

        if self.hotkey_mgr.is_none() {
            match HotkeyManager::new(self.config.hotkey) {
                Ok(mgr) => self.hotkey_mgr = Some(mgr),
                Err(e) => eprintln!("Failed to initialize hotkeys: {:?}", e),
            }
        }
        if self.tray_mgr.is_none() {
            match SystemTray::new(&self.config) {
                Ok(tray) => self.tray_mgr = Some(tray),
                Err(e) => eprintln!("Failed to initialize system tray: {:?}", e),
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // Process pending action AFTER overlay window has been dropped/destroyed cleanly!
        if let Some((selection_img, action)) = self.pending_action.take() {
            if self.debug_mode {
                println!("[DEBUG] Executing pending action: {:?}", action);
            }

            // "Guardar (S)" picks its own format, quality and scale inside the native
            // dialog, so it keeps the original pixels and resamples afterwards.
            if action == CaptureAction::SaveOnly {
                self.run_save_dialog(&selection_img);
                if self.is_cli_mode {
                    event_loop.exit();
                }
                return;
            }

            // Everything else resamples once, up front, so the saved file and the
            // clipboard get identical pixels and "Ambos" does not run Lanczos3 twice.
            let mut opts = self.config.save_options();
            let scale_percent = opts.scale_percent;
            let selection_img =
                storage::downscaled(&selection_img, scale_percent).unwrap_or(selection_img);
            opts.scale_percent = 100;

            if self.debug_mode && scale_percent < 100 {
                println!("[DEBUG] Selection rescaled to {}% -> {}x{}", scale_percent, selection_img.width(), selection_img.height());
            }

            match action {
                CaptureAction::SaveOnly => unreachable!("handled above"),
                CaptureAction::CopyOnly => {
                    if self.debug_mode {
                        println!("[DEBUG] Calling copy_to_clipboard (Dimensions: {}x{})...", selection_img.width(), selection_img.height());
                    }
                    match copy_to_clipboard(&selection_img) {
                        Ok(_) => println!("[OK] Image copied to system clipboard!"),
                        Err(e) => eprintln!("[ERROR] Clipboard copy failed: {:?}", e),
                    }
                }
                CaptureAction::SaveAndCopy => {
                    // "Ambos (Enter)" saves automatically to default Ruuutu folder WITHOUT dialog, using configured format!
                    let saved_path = save_image(&selection_img, &opts).ok();
                    let copy_res = copy_to_clipboard(&selection_img);
                    println!("[OK] Auto-saved ({:?} q{} scale {}%) to default folder {:?} and copied to clipboard (Result: {:?}).", opts.format, opts.quality, scale_percent, saved_path, copy_res);
                }
                CaptureAction::Cancel => {
                    if self.debug_mode {
                        println!("[DEBUG] Capture cancelled by user (ESC).");
                    }
                }
            }

            if self.is_cli_mode {
                event_loop.exit();
                return;
            }
        }

        // Check System Tray Left-Click Event (Triggers Capture immediately!)
        if self.overlay.is_none() && self.pending_action.is_none() {
            while let Ok(event) = TrayIconEvent::receiver().try_recv() {
                if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                    if self.debug_mode {
                        println!("[DEBUG] Left-click on system tray icon! Spawning overlay capture.");
                    }
                    if let Err(e) = self.spawn_overlay(event_loop) {
                        eprintln!("Error starting capture flow: {:?}", e);
                    }
                }
            }
        }

        // Check Hotkeys (ONLY when no overlay is open)
        if self.overlay.is_none() && self.pending_action.is_none() {
            // Check Low-Level PrtScn hook trigger (suppresses Windows Snipping Tool)
            if PRTSCN_TRIGGERED.swap(false, Ordering::SeqCst) {
                if self.debug_mode {
                    println!("[DEBUG] PrtScn hook intercepted! Suppressed Windows Snipping Tool & spawning Ruuutu overlay.");
                }
                if let Err(e) = self.spawn_overlay(event_loop) {
                    eprintln!("Error starting capture flow: {:?}", e);
                }
            }

            // Check global hotkeys
            let hotkey_rx = GlobalHotKeyEvent::receiver();
            while let Ok(event) = hotkey_rx.try_recv() {
                if event.state == HotKeyState::Pressed {
                    if let Some(ref mgr) = self.hotkey_mgr {
                        if mgr.matches(event.id) {
                            if self.debug_mode {
                                println!("[DEBUG] Hotkey triggered! Spawning overlay.");
                            }
                            if let Err(e) = self.spawn_overlay(event_loop) {
                                eprintln!("Error starting capture flow: {:?}", e);
                            }
                            break;
                        }
                    }
                }
            }

            // Check System Tray Context Menu Events
            let menu_rx = MenuEvent::receiver();
            while let Ok(event) = menu_rx.try_recv() {
                if let Some(ref tray) = self.tray_mgr {
                    if event.id == tray.menu_capture.id() {
                        if self.debug_mode {
                            println!("[DEBUG] System Tray menu 'Tomar Captura' triggered! Spawning overlay.");
                        }
                        if let Err(e) = self.spawn_overlay(event_loop) {
                            eprintln!("Error starting capture flow: {:?}", e);
                        }
                        break;
                    // One branch per settings group, not per option: the tray owns the
                    // id -> value table for each submenu. Adding an option is adding a
                    // variant to its `ALL` list in `config.rs`; nothing here changes.
                    //
                    // None of these rebuild the tray. Only the format needs more than a
                    // re-check, because the quality wording depends on it, and that is
                    // what `refresh_for_format` does in place.
                    } else if let Some(format) = tray.format_choice(&event.id) {
                        self.config.format = format;
                        let _ = self.config.save();
                        tray.refresh_for_format(format);
                        tray.update_checks(&self.config);
                        println!("[CONFIG] Selected format: {}", format.display_name());
                    } else if let Some(quality) = tray.quality_choice(&event.id) {
                        self.config.quality = quality;
                        let _ = self.config.save();
                        tray.update_checks(&self.config);
                        println!("[CONFIG] Selected quality: {}", quality.label_for(self.config.format));
                    } else if let Some(scale) = tray.scale_choice(&event.id) {
                        self.config.scale = scale;
                        let _ = self.config.save();
                        tray.update_checks(&self.config);
                        println!("[CONFIG] Save scale: {}%", scale.percent());
                    // Takes effect on the next capture, since the overlay is built fresh
                    // each time.
                    } else if let Some(text_scale) = tray.text_scale_choice(&event.id) {
                        self.config.text_scale = text_scale;
                        let _ = self.config.save();
                        tray.update_checks(&self.config);
                        println!("[CONFIG] Overlay text scale: {}%", text_scale.percent());
                    } else if let Some(hotkey) = tray.hotkey_choice(&event.id) {
                        self.config.hotkey = hotkey;
                        if let Some(ref mut mgr) = self.hotkey_mgr { let _ = mgr.set_preset(hotkey); }
                        let _ = self.config.save();
                        tray.update_checks(&self.config);
                        println!("[CONFIG] Selected hotkey: {}", hotkey.label());
                    } else if event.id == tray.chk_autostart.id() {
                        let new_autostart = !self.config.autostart;
                        self.config.autostart = new_autostart;
                        let _ = set_autostart_enabled(new_autostart);
                        let _ = self.config.save();
                        tray.update_checks(&self.config);
                        println!("[CONFIG] Autostart with Windows: {}", new_autostart);
                    } else if event.id == tray.menu_reset.id() {
                        self.config = AppConfig::default();
                        let _ = set_autostart_enabled(false);
                        if let Some(ref mut mgr) = self.hotkey_mgr {
                            let _ = mgr.set_preset(self.config.hotkey);
                        }
                        let _ = self.config.save();
                        tray.refresh_for_format(self.config.format);
                        tray.update_checks(&self.config);
                        println!("[CONFIG] Restored default settings!");
                    } else if event.id == tray.menu_folder.id() {
                        if let Ok(dir) = storage::get_screenshots_dir() {
                            let _ = std::process::Command::new("explorer").arg(dir).spawn();
                        }
                    } else if event.id == tray.menu_quit.id() {
                        println!("Exiting Ruuutu.");
                        event_loop.exit();
                        return;
                    }
                }
            }
        }

        // The overlay is not repainted from here. `redraw()` already calls
        // `request_redraw()` while a button animation or a hover delay is pending, so
        // the frames arrive as `RedrawRequested` and stop by themselves once everything
        // has settled. Painting unconditionally on every poll meant copying the whole
        // desktop-sized buffer and presenting it as fast as the CPU allowed, even with
        // nothing moving on screen.
        //
        // The tray, hotkey and menu channels are not winit events, so the loop still has
        // to come back periodically to drain them: a ~15 ms tick keeps the shortcut
        // latency imperceptible while leaving the process asleep in between, instead of
        // spinning on `Poll`.
        event_loop.set_control_flow(ControlFlow::WaitUntil(
            Instant::now() + Duration::from_millis(15),
        ));
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, window_id: WindowId, event: WindowEvent) {
        let mut finished = false;
        let mut result = None;

        if let Some(ref mut overlay) = self.overlay {
            if window_id == overlay.window_id() {
                overlay.handle_event(&event);
                if overlay.finished {
                    finished = true;
                    result = overlay.result.take();
                }
            }
        }

        if finished {
            if self.debug_mode {
                println!("[DEBUG] Window finished. Destroying overlay window.");
            }
            self.overlay = None;
            self.pending_action = result.or(Some((RgbaImage::new(1, 1), CaptureAction::Cancel)));

            if self.is_cli_mode && self.pending_action.as_ref().map(|(_, a)| *a) == Some(CaptureAction::Cancel) {
                event_loop.exit();
            }
        }
    }
}

impl RuuutuApp {
    /// Runs the native save dialog and honours whatever was chosen inside it.
    ///
    /// `img` must be the selection at full resolution: the scale combo lives in the
    /// dialog, so resampling only happens once the user has confirmed.
    fn run_save_dialog(&mut self, img: &RgbaImage) {
        let dir = storage::get_screenshots_dir().unwrap_or_else(|_| PathBuf::from("."));
        let default_name = storage::default_file_name(self.config.format.to_output_format());

        let outcome = save_dialog::show_save_dialog(
            &default_name,
            &dir,
            self.config.format,
            self.config.quality,
            self.config.scale,
        );

        match outcome {
            Ok(Some(choice)) => {
                // Encode with what the dialog returned, not with the tray defaults.
                let effective = AppConfig {
                    format: choice.format,
                    quality: choice.quality,
                    scale: choice.scale,
                    ..self.config.clone()
                };

                match storage::save_image_to(img, &choice.path, &effective.save_options()) {
                    Ok(()) => println!(
                        "[OK] Saved via dialog to {:?} ({:?}, calidad {:?}, escala {}%).",
                        choice.path, choice.format, choice.quality, choice.scale.percent()
                    ),
                    Err(e) => eprintln!("[ERROR] Failed to save image: {:?}", e),
                }

                if choice.remember {
                    self.config.format = choice.format;
                    self.config.quality = choice.quality;
                    self.config.scale = choice.scale;
                    let _ = self.config.save();
                    // Rebuild: the quality submenu is relabelled per format.
                    self.tray_mgr = SystemTray::new(&self.config).ok();
                    println!("[CONFIG] Ajustes del diálogo guardados como predeterminados.");
                }
            }
            Ok(None) => {
                if self.debug_mode {
                    println!("[DEBUG] Save dialog cancelled by user.");
                }
            }
            Err(e) => eprintln!("[ERROR] Save dialog failed: {:?}", e),
        }
    }

    fn spawn_overlay(&mut self, event_loop: &ActiveEventLoop) -> Result<()> {
        println!("Capturing desktop...");
        let (desktop_img, min_x, min_y, total_w, total_h) = capture_desktop()?;

        println!(
            "Desktop captured ({}x{} at {},{}). Opening selection overlay...",
            total_w, total_h, min_x, min_y
        );

        let hint = SaveHint {
            scale_percent: self.config.scale.percent(),
            format_name: self.config.format.display_name(),
        };

        let overlay = SelectionOverlay::new(
            event_loop,
            desktop_img,
            min_x,
            min_y,
            total_w,
            total_h,
            self.debug_mode,
            hint,
            self.config.text_scale.factor(),
        )?;
        self.overlay = Some(overlay);
        Ok(())
    }
}

fn main() -> Result<()> {
    // Before any println!: borrows the terminal's console if we were launched from one.
    console::attach_parent_console();

    // High-DPI awareness & OleInitialize for System Tray and Clipboard
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        OleInitialize(std::ptr::null_mut());
    }

    let args: Vec<String> = std::env::args().collect();
    let is_cli_mode = args.iter().any(|arg| arg == "--capture");
    let debug_mode = args.iter().any(|arg| arg == "--debug" || arg == "-d");
    let config = AppConfig::load();

    if debug_mode {
        println!("=======================================================");
        println!(" 🐛 RUUUTU DEBUG MODE (--debug) ");
        println!(" Telemetria activada en consola y HUD visual en pantalla.");
        println!(" Configuración cargada: Formato {:?}, Calidad {:?}, Escala {}%, Hotkey: {:?}, Autostart: {}", config.format, config.quality, config.scale.percent(), config.hotkey, config.autostart);
        println!("=======================================================");
    }

    if !is_cli_mode {
        println!("Starting Ruuutu service in background (System Tray & Hotkeys)...");
    }

    let event_loop = EventLoop::builder().build().map_err(|e| anyhow::anyhow!("EventLoop error: {:?}", e))?;
    // Re-armed on every `about_to_wait`; see the note there.
    event_loop.set_control_flow(ControlFlow::WaitUntil(
        Instant::now() + Duration::from_millis(15),
    ));

    let mut app = RuuutuApp {
        is_cli_mode,
        debug_mode,
        config,
        hotkey_mgr: None,
        tray_mgr: None,
        overlay: None,
        pending_action: None,
    };

    event_loop.run_app(&mut app).map_err(|e| anyhow::anyhow!("Application error: {:?}", e))?;
    Ok(())
}
