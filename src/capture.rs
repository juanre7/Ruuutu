// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 juanre7

use anyhow::{Context, Result};
use image::RgbaImage;
use windows_sys::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits,
    GetDC, ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
    DIB_RGB_COLORS, SRCCOPY,
};
use windows_sys::Win32::UI::WindowsAndMessaging::GetDesktopWindow;

use windows_sys::Win32::System::StationsAndDesktops::{
    CloseDesktop, GetThreadDesktop, OpenInputDesktop, SetThreadDesktop,
};
use windows_sys::Win32::System::Threading::GetCurrentThreadId;

pub fn capture_desktop() -> Result<(RgbaImage, i32, i32, u32, u32)> {
    unsafe {
        let orig_desk = GetThreadDesktop(GetCurrentThreadId());
        let hdesk = OpenInputDesktop(0, 0, 0x0100); // DESKTOP_SWITCHDESKTOP
        if hdesk != 0 {
            SetThreadDesktop(hdesk);
        }

        // `OpenInputDesktop` hands out a handle that has to be given back. Ruuutu lives in
        // the tray for days, so leaking one per capture is a slow handle leak. It can only
        // be closed once the thread is no longer standing on it, hence the ordering.
        let release_desktop = || {
            if orig_desk != 0 {
                SetThreadDesktop(orig_desk);
            }
            if hdesk != 0 {
                CloseDesktop(hdesk);
            }
        };

        let hwnd_desktop = GetDesktopWindow();
        let hdc_screen = GetDC(hwnd_desktop);
        if hdc_screen == 0 {
            release_desktop();
            anyhow::bail!("Failed to GetDC for desktop window");
        }

        let min_x = windows_sys::Win32::UI::WindowsAndMessaging::GetSystemMetrics(windows_sys::Win32::UI::WindowsAndMessaging::SM_XVIRTUALSCREEN);
        let min_y = windows_sys::Win32::UI::WindowsAndMessaging::GetSystemMetrics(windows_sys::Win32::UI::WindowsAndMessaging::SM_YVIRTUALSCREEN);
        let total_w = windows_sys::Win32::UI::WindowsAndMessaging::GetSystemMetrics(windows_sys::Win32::UI::WindowsAndMessaging::SM_CXVIRTUALSCREEN) as u32;
        let total_h = windows_sys::Win32::UI::WindowsAndMessaging::GetSystemMetrics(windows_sys::Win32::UI::WindowsAndMessaging::SM_CYVIRTUALSCREEN) as u32;

        let total_w = if total_w == 0 { 1920 } else { total_w };
        let total_h = if total_h == 0 { 1080 } else { total_h };

        let hdc_mem = CreateCompatibleDC(hdc_screen);
        if hdc_mem == 0 {
            ReleaseDC(hwnd_desktop, hdc_screen);
            release_desktop();
            anyhow::bail!("Failed to CreateCompatibleDC");
        }

        let hbmp = CreateCompatibleBitmap(hdc_screen, total_w as i32, total_h as i32);
        if hbmp == 0 {
            DeleteDC(hdc_mem);
            ReleaseDC(hwnd_desktop, hdc_screen);
            release_desktop();
            anyhow::bail!("Failed to CreateCompatibleBitmap");
        }

        let old_bmp = SelectObject(hdc_mem, hbmp);
        if old_bmp == 0 || old_bmp == -1 {
            DeleteObject(hbmp);
            DeleteDC(hdc_mem);
            ReleaseDC(hwnd_desktop, hdc_screen);
            release_desktop();
            anyhow::bail!("Failed to SelectObject bitmap into DC");
        }

        let blit_ok = BitBlt(
            hdc_mem,
            0,
            0,
            total_w as i32,
            total_h as i32,
            hdc_screen,
            min_x,
            min_y,
            SRCCOPY,
        ) != 0;

        let mut bmi: BITMAPINFO = std::mem::zeroed();
        bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = total_w as i32;
        bmi.bmiHeader.biHeight = -(total_h as i32); // Top-down
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 32;
        bmi.bmiHeader.biCompression = BI_RGB;

        // Checked: a virtual desktop large enough to overflow this would wrap silently
        // and hand `GetDIBits` a buffer far smaller than the bitmap it writes.
        let pixel_bytes = match (total_w as usize)
            .checked_mul(total_h as usize)
            .and_then(|px| px.checked_mul(4))
        {
            Some(n) => n,
            None => {
                SelectObject(hdc_mem, old_bmp);
                DeleteObject(hbmp);
                DeleteDC(hdc_mem);
                ReleaseDC(hwnd_desktop, hdc_screen);
                release_desktop();
                anyhow::bail!("Virtual desktop too large to capture ({}x{})", total_w, total_h);
            }
        };

        let mut raw_pixels: Vec<u8> = vec![0; pixel_bytes];

        let res = GetDIBits(
            hdc_mem,
            hbmp,
            0,
            total_h,
            raw_pixels.as_mut_ptr() as *mut _,
            &mut bmi,
            DIB_RGB_COLORS,
        );

        SelectObject(hdc_mem, old_bmp);
        DeleteObject(hbmp);
        DeleteDC(hdc_mem);
        ReleaseDC(hwnd_desktop, hdc_screen);
        release_desktop();

        if !blit_ok {
            anyhow::bail!("BitBlt of the virtual desktop failed");
        }
        if res == 0 {
            anyhow::bail!("GetDIBits failed");
        }

        // Convert BGRA (Win32 GetDIBits default) to RGBA
        let mut rgba_pixels = vec![0u8; pixel_bytes];
        for i in 0..(total_w * total_h) as usize {
            let b = raw_pixels[i * 4];
            let g = raw_pixels[i * 4 + 1];
            let r = raw_pixels[i * 4 + 2];
            let a = raw_pixels[i * 4 + 3];

            rgba_pixels[i * 4] = r;
            rgba_pixels[i * 4 + 1] = g;
            rgba_pixels[i * 4 + 2] = b;
            rgba_pixels[i * 4 + 3] = if a == 0 { 255 } else { a };
        }

        let img = RgbaImage::from_raw(total_w, total_h, rgba_pixels)
            .context("Failed to construct RgbaImage from GDI capture")?;

        Ok((img, min_x, min_y, total_w, total_h))
    }
}
