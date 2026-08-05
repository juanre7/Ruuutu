// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 juanre7

use anyhow::{Context, Result};
use tray_icon::menu::{CheckMenuItem, Menu, MenuId, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

use crate::config::{AppConfig, HotkeyPreset, ImageFormatChoice, QualityChoice, ScaleChoice, TextScaleChoice};
use crate::icon::icon_rgba;

/// One radio-style group of the tray menu: the check items, paired with the value each
/// one selects.
///
/// Every settings submenu has this exact shape — a list of mutually exclusive options
/// differing only in their value — so they are all built, re-checked and dispatched
/// through the same three methods instead of one struct field and one `else if` branch
/// per option.
struct MenuGroup<T: Copy + PartialEq> {
    items: Vec<(CheckMenuItem, T)>,
}

impl<T: Copy + PartialEq> MenuGroup<T> {
    /// Builds the submenu and its items, checking whichever one matches `current`.
    fn new(
        title: &str,
        values: &[T],
        label: impl Fn(T) -> &'static str,
        current: T,
    ) -> (Submenu, Self) {
        let submenu = Submenu::new(title, true);
        let items: Vec<(CheckMenuItem, T)> = values
            .iter()
            .map(|&value| {
                let item = CheckMenuItem::new(label(value), true, value == current, None);
                let _ = submenu.append(&item);
                (item, value)
            })
            .collect();

        (submenu, Self { items })
    }

    /// The value a menu id selects, if the id belongs to this group.
    fn choice(&self, id: &MenuId) -> Option<T> {
        self.items
            .iter()
            .find(|(item, _)| item.id() == id)
            .map(|(_, value)| *value)
    }

    /// Moves the check mark to `current`.
    fn set_checked(&self, current: T) {
        for (item, value) in &self.items {
            item.set_checked(*value == current);
        }
    }

    /// Rewrites every label in place. Used only by the quality group, whose wording
    /// depends on the active format.
    fn relabel(&self, label: impl Fn(T) -> &'static str) {
        for (item, value) in &self.items {
            item.set_text(label(*value));
        }
    }
}

pub struct SystemTray {
    _tray_icon: TrayIcon,
    pub menu_capture: MenuItem,
    pub menu_folder: MenuItem,
    pub menu_reset: MenuItem,
    pub menu_quit: MenuItem,
    formats: MenuGroup<ImageFormatChoice>,
    qualities: MenuGroup<QualityChoice>,
    /// Kept because its title names the active format ("Calidad (WebP)" /
    /// "Nivel de Compresión (PNG)") and has to be rewritten along with the items.
    quality_submenu: Submenu,
    scales: MenuGroup<ScaleChoice>,
    text_scales: MenuGroup<TextScaleChoice>,
    hotkeys: MenuGroup<HotkeyPreset>,
    // Autostart is a lone toggle, not a group.
    pub chk_autostart: CheckMenuItem,
}

impl SystemTray {
    pub fn new(cfg: &AppConfig) -> Result<Self> {
        let menu_capture = MenuItem::new("⚡ Tomar Captura", true, None);
        let menu_folder = MenuItem::new("📁 Abrir Carpeta de Capturas", true, None);
        let menu_reset = MenuItem::new("🔄 Restaurar Ajustes por Defecto", true, None);
        let menu_quit = MenuItem::new("❌ Salir de Ruuutu", true, None);

        let (format_submenu, formats) = MenuGroup::new(
            "🎨 Formato de Imagen",
            &ImageFormatChoice::ALL,
            |f| f.menu_label(),
            cfg.format,
        );

        // Calidad / Compresión: the wording depends on what the active format can
        // actually do, which is why this group gets relabelled in `refresh_for_format`.
        // Both paths read `label_for`, so there is no second copy of the wording.
        let (quality_submenu, qualities) = MenuGroup::new(
            cfg.format.quality_menu_title(),
            &QualityChoice::ALL,
            |q| q.label_for(cfg.format),
            cfg.quality,
        );

        let (scale_submenu, scales) = MenuGroup::new(
            "📐 Escala de Guardado",
            &ScaleChoice::ALL,
            |s| s.label(),
            cfg.scale,
        );

        let (text_scale_submenu, text_scales) = MenuGroup::new(
            "🔠 Escala del Texto (captura)",
            &TextScaleChoice::ALL,
            |t| t.label(),
            cfg.text_scale,
        );

        let (hotkey_submenu, hotkeys) = MenuGroup::new(
            "⌨️ Atajo de Teclado",
            &HotkeyPreset::ALL,
            |h| h.label(),
            cfg.hotkey,
        );

        // Checkbox Autostart
        let chk_autostart = CheckMenuItem::new("🚀 Iniciar con Windows", true, cfg.autostart, None);

        // Cuatro bloques, uno por categoría: acciones inmediatas, ajustes con submenú,
        // preferencias del sistema, y salir.
        let menu = Menu::new();
        let _ = menu.append(&menu_capture);
        let _ = menu.append(&menu_folder);
        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&format_submenu);
        let _ = menu.append(&quality_submenu);
        let _ = menu.append(&scale_submenu);
        let _ = menu.append(&text_scale_submenu);
        let _ = menu.append(&hotkey_submenu);
        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&chk_autostart);
        let _ = menu.append(&menu_reset);
        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&menu_quit);

        compact_menu_gutter(&menu);

        let icon = create_tray_icon()?;

        let tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("Ruuutu Captura de Pantalla")
            .with_icon(icon)
            .build()
            .context("Failed to build tray icon")?;

        Ok(Self {
            _tray_icon: tray_icon,
            menu_capture,
            menu_folder,
            menu_reset,
            menu_quit,
            formats,
            qualities,
            quality_submenu,
            scales,
            text_scales,
            hotkeys,
            chk_autostart,
        })
    }

    /// The setting a menu id selects, if the id belongs to that group. `main.rs`
    /// dispatches the whole menu through these instead of one branch per option.
    pub fn format_choice(&self, id: &MenuId) -> Option<ImageFormatChoice> {
        self.formats.choice(id)
    }

    pub fn quality_choice(&self, id: &MenuId) -> Option<QualityChoice> {
        self.qualities.choice(id)
    }

    pub fn scale_choice(&self, id: &MenuId) -> Option<ScaleChoice> {
        self.scales.choice(id)
    }

    pub fn text_scale_choice(&self, id: &MenuId) -> Option<TextScaleChoice> {
        self.text_scales.choice(id)
    }

    pub fn hotkey_choice(&self, id: &MenuId) -> Option<HotkeyPreset> {
        self.hotkeys.choice(id)
    }

    /// Rewrites the quality group for a newly selected format: "Sin pérdidas (VP8L)"
    /// becomes "Máxima (nivel 9, más lento)" for PNG, and the submenu title follows.
    ///
    /// This exists so changing the format does **not** rebuild the whole `SystemTray`.
    /// A rebuild creates a new hidden window and a new `Shell_NotifyIconW` uID, so the
    /// shell sees a different notification-area icon rather than the same one updated,
    /// and whether it keeps the user's visibility preference across that is undocumented
    /// and differs between Windows 10 and 11. Relabelling in place depends on none of it.
    pub fn refresh_for_format(&self, format: ImageFormatChoice) {
        self.qualities.relabel(|q| q.label_for(format));
        self.quality_submenu.set_text(format.quality_menu_title());
    }

    pub fn update_checks(&self, cfg: &AppConfig) {
        self.formats.set_checked(cfg.format);
        self.qualities.set_checked(cfg.quality);
        self.scales.set_checked(cfg.scale);
        self.text_scales.set_checked(cfg.text_scale);
        self.hotkeys.set_checked(cfg.hotkey);
        self.chk_autostart.set_checked(cfg.autostart);
    }
}

/// Collapse the left gutter of the popup menu.
///
/// By default Win32 reserves *two* separate columns to the left of every item: one for the
/// check mark and one for the item bitmap. Nothing in this menu uses item bitmaps (the
/// pictograms are just characters inside the label), so the bitmap column is dead space —
/// that is the wide left margin. `MNS_CHECKORBMP` makes the check mark share the bitmap
/// column instead of getting its own, which removes it. `MIM_APPLYTOSUBMENUS` pushes the
/// style down the whole tree, so this single call on the root popup also covers the
/// submenus of formato/calidad/escala/atajo, which are the ones that actually draw checks.
///
/// Must run after every `append`: the flag is applied to the submenus attached at call time.
fn compact_menu_gutter(menu: &Menu) {
    use tray_icon::menu::ContextMenu;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SetMenuInfo, MENUINFO, MIM_APPLYTOSUBMENUS, MIM_STYLE, MNS_CHECKORBMP,
    };

    let hpopupmenu = menu.hpopupmenu();
    if hpopupmenu == 0 {
        return;
    }

    let info = MENUINFO {
        cbSize: std::mem::size_of::<MENUINFO>() as u32,
        fMask: MIM_STYLE | MIM_APPLYTOSUBMENUS,
        dwStyle: MNS_CHECKORBMP,
        cyMax: 0,
        hbrBack: 0,
        dwContextHelpID: 0,
        dwMenuData: 0,
    };

    unsafe { SetMenuInfo(hpopupmenu, &info) };
}

/// The drawing itself lives in `icon.rs`, shared with `build.rs` so the tray and the executable
/// show the same icon.
fn create_tray_icon() -> Result<Icon> {
    const SIZE: u32 = 32;
    Icon::from_rgba(icon_rgba(SIZE), SIZE, SIZE).context("Failed to create tray icon")
}
