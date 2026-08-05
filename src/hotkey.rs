// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 juanre7

//! Global shortcuts.
//!
//! Two mechanisms, for two different reasons:
//!
//! - `global-hotkey` (`RegisterHotKey` underneath) registers the configured combination.
//! - A `WH_KEYBOARD_LL` hook exists purely to *suppress* PrtScn, because Windows routes it
//!   to the Snipping Tool before any hotkey registration gets a look in.
//!
//! The hook is the invasive one: it sees every keystroke in the session, so it is installed
//! only when the active preset actually needs PrtScn, and it consumes a key only when the
//! modifiers match that preset. Under `Ctrl + Shift + S` there is no hook at all and PrtScn
//! keeps doing whatever Windows does with it.

use anyhow::{Context, Result};
use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::GlobalHotKeyManager;
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU8, Ordering};
use windows_sys::Win32::Foundation::HMODULE;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_CONTROL, VK_MENU, VK_SHIFT,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, SetWindowsHookExW, UnhookWindowsHookEx, HHOOK, HC_ACTION, KBDLLHOOKSTRUCT,
    WH_KEYBOARD_LL, WM_KEYDOWN, WM_SYSKEYDOWN,
};

use crate::config::HotkeyPreset;

const VK_SNAPSHOT: u32 = 0x2C;

pub static PRTSCN_TRIGGERED: AtomicBool = AtomicBool::new(false);

/// The installed hook, shared with the callback. Atomic rather than `static mut`: the
/// callback runs during message dispatch and reads what `set_preset` writes, which as a
/// plain mutable static is a data race (and a hard error from edition 2024 on).
static HOOK_HANDLE: AtomicIsize = AtomicIsize::new(0);

/// Which preset the hook should honour, as [`PresetCode`].
static HOOK_PRESET: AtomicU8 = AtomicU8::new(PresetCode::PRTSCN_ALT_A);

/// `HotkeyPreset` flattened into something an atomic can carry across to the callback.
struct PresetCode;

impl PresetCode {
    const PRTSCN_ALT_A: u8 = 0;
    const CTRL_SHIFT_S: u8 = 1;
    const ALT_PRTSCN: u8 = 2;
    const SHIFT_PRTSCN: u8 = 3;

    fn of(preset: HotkeyPreset) -> u8 {
        match preset {
            HotkeyPreset::PrtScnAltA => Self::PRTSCN_ALT_A,
            HotkeyPreset::CtrlShiftS => Self::CTRL_SHIFT_S,
            HotkeyPreset::AltPrtScn => Self::ALT_PRTSCN,
            HotkeyPreset::ShiftPrtScn => Self::SHIFT_PRTSCN,
        }
    }
}

/// Is `vk` currently held down?
///
/// `GetAsyncKeyState` and not `GetKeyState`: inside a low-level hook the thread's own key
/// state is not the one the user is typing against.
unsafe fn is_down(vk: i32) -> bool {
    unsafe { (GetAsyncKeyState(vk) as u16 & 0x8000) != 0 }
}

unsafe extern "system" fn low_level_keyboard_proc(
    code: i32,
    w_param: usize,
    l_param: isize,
) -> isize {
    unsafe {
        let hook = HOOK_HANDLE.load(Ordering::Relaxed);

        if code == HC_ACTION as i32 && l_param != 0 {
            let kbd = &*(l_param as *const KBDLLHOOKSTRUCT);
            let is_key_down = w_param as u32 == WM_KEYDOWN || w_param as u32 == WM_SYSKEYDOWN;

            if kbd.vkCode == VK_SNAPSHOT && is_key_down {
                let alt = is_down(VK_MENU as i32);
                let shift = is_down(VK_SHIFT as i32);
                let ctrl = is_down(VK_CONTROL as i32);

                // Only the exact combination the user configured is ours. Anything else
                // falls through untouched — Alt+PrtScn still copies the active window
                // unless that is precisely the shortcut Ruuutu was told to use.
                let is_ours = match HOOK_PRESET.load(Ordering::Relaxed) {
                    PresetCode::PRTSCN_ALT_A => !alt && !shift && !ctrl,
                    PresetCode::ALT_PRTSCN => alt && !shift && !ctrl,
                    PresetCode::SHIFT_PRTSCN => shift && !alt && !ctrl,
                    _ => false,
                };

                if is_ours {
                    PRTSCN_TRIGGERED.store(true, Ordering::SeqCst);
                    // Nonzero swallows the key, which is the whole point: it stops
                    // Windows from opening the Snipping Tool on top of the overlay.
                    return 1;
                }
            }
        }

        CallNextHookEx(hook, code, w_param, l_param)
    }
}

pub struct HotkeyManager {
    manager: GlobalHotKeyManager,
    pub hotkey1: Option<HotKey>,
    pub hotkey2: Option<HotKey>,
    hook: HHOOK,
}

impl HotkeyManager {
    pub fn new(preset: HotkeyPreset) -> Result<Self> {
        let manager = GlobalHotKeyManager::new().context("Failed to init GlobalHotKeyManager")?;

        let mut mgr = Self {
            manager,
            hotkey1: None,
            hotkey2: None,
            hook: 0,
        };
        mgr.set_preset(preset)?;
        Ok(mgr)
    }

    pub fn set_preset(&mut self, preset: HotkeyPreset) -> Result<()> {
        if let Some(hk) = self.hotkey1 { let _ = self.manager.unregister(hk); }
        if let Some(hk) = self.hotkey2 { let _ = self.manager.unregister(hk); }

        let (h1, h2) = match preset {
            HotkeyPreset::PrtScnAltA => (
                Some(HotKey::new(None, Code::PrintScreen)),
                Some(HotKey::new(Some(Modifiers::ALT), Code::KeyA)),
            ),
            HotkeyPreset::CtrlShiftS => (
                Some(HotKey::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyS)),
                None,
            ),
            HotkeyPreset::AltPrtScn => (
                Some(HotKey::new(Some(Modifiers::ALT), Code::PrintScreen)),
                None,
            ),
            HotkeyPreset::ShiftPrtScn => (
                Some(HotKey::new(Some(Modifiers::SHIFT), Code::PrintScreen)),
                None,
            ),
        };

        // Registered even for the PrtScn presets, where the hook normally gets there
        // first: it is the fallback if the hook could not be installed.
        if let Some(hk) = h1 { let _ = self.manager.register(hk); }
        if let Some(hk) = h2 { let _ = self.manager.register(hk); }

        self.hotkey1 = h1;
        self.hotkey2 = h2;

        HOOK_PRESET.store(PresetCode::of(preset), Ordering::Relaxed);
        self.set_hook_installed(preset.uses_print_screen());

        Ok(())
    }

    /// Installs or removes the low-level hook so it is only live while a PrtScn preset is.
    fn set_hook_installed(&mut self, wanted: bool) {
        match (wanted, self.hook) {
            (true, 0) => {
                let hook = unsafe {
                    SetWindowsHookExW(WH_KEYBOARD_LL, Some(low_level_keyboard_proc), 0 as HMODULE, 0)
                };
                HOOK_HANDLE.store(hook, Ordering::Relaxed);
                self.hook = hook;
            }
            (false, h) if h != 0 => {
                unsafe { UnhookWindowsHookEx(h) };
                HOOK_HANDLE.store(0, Ordering::Relaxed);
                self.hook = 0;
                // A PrtScn queued just before unhooking would otherwise fire a capture
                // under a preset that no longer uses the key.
                PRTSCN_TRIGGERED.store(false, Ordering::SeqCst);
            }
            _ => {}
        }
    }

    pub fn matches(&self, event_id: u32) -> bool {
        if let Some(ref h1) = self.hotkey1 {
            if event_id == h1.id() { return true; }
        }
        if let Some(ref h2) = self.hotkey2 {
            if event_id == h2.id() { return true; }
        }
        false
    }
}

impl Drop for HotkeyManager {
    fn drop(&mut self) {
        self.set_hook_installed(false);
    }
}
