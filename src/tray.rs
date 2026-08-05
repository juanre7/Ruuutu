use anyhow::{Context, Result};
use tray_icon::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

use crate::config::{AppConfig, HotkeyPreset, ImageFormatChoice, QualityChoice, ScaleChoice};
use crate::icon::icon_rgba;

pub struct SystemTray {
    _tray_icon: TrayIcon,
    pub menu_capture: MenuItem,
    pub menu_folder: MenuItem,
    pub menu_reset: MenuItem,
    pub menu_quit: MenuItem,
    // Format options
    pub fmt_webp: CheckMenuItem,
    pub fmt_png: CheckMenuItem,
    pub fmt_jpeg: CheckMenuItem,
    // Quality options (labels depend on the active format)
    pub q_max: CheckMenuItem,
    pub q_high: CheckMenuItem,
    pub q_medium: CheckMenuItem,
    pub q_low: CheckMenuItem,
    // Save scale options
    pub sc_full: CheckMenuItem,
    pub sc_75: CheckMenuItem,
    pub sc_50: CheckMenuItem,
    pub sc_25: CheckMenuItem,
    // Hotkey options
    pub hk_prtscn_alta: CheckMenuItem,
    pub hk_ctrl_shift_s: CheckMenuItem,
    pub hk_alt_prtscn: CheckMenuItem,
    pub hk_shift_prtscn: CheckMenuItem,
    // Autostart
    pub chk_autostart: CheckMenuItem,
}

impl SystemTray {
    pub fn new(cfg: &AppConfig) -> Result<Self> {
        let menu_capture = MenuItem::new("⚡ Tomar Captura", true, None);
        let menu_folder = MenuItem::new("📁 Abrir Carpeta de Capturas", true, None);
        let menu_reset = MenuItem::new("🔄 Restaurar Ajustes por Defecto", true, None);
        let menu_quit = MenuItem::new("❌ Salir de Ruuutu", true, None);

        // Submenu Formato
        let fmt_webp = CheckMenuItem::new("WebP (Recomendado)", true, cfg.format == ImageFormatChoice::WebP, None);
        let fmt_png = CheckMenuItem::new("PNG (Alta fidelidad)", true, cfg.format == ImageFormatChoice::Png, None);
        let fmt_jpeg = CheckMenuItem::new("JPEG (Ligero)", true, cfg.format == ImageFormatChoice::Jpeg, None);

        let format_submenu = Submenu::new("🎨 Formato de Imagen", true);
        let _ = format_submenu.append(&fmt_webp);
        let _ = format_submenu.append(&fmt_png);
        let _ = format_submenu.append(&fmt_jpeg);

        // Submenu Calidad / Compresión: wording depends on what the format can actually do.
        let q_max = CheckMenuItem::new(QualityChoice::Max.label_for(cfg.format), true, cfg.quality == QualityChoice::Max, None);
        let q_high = CheckMenuItem::new(QualityChoice::High.label_for(cfg.format), true, cfg.quality == QualityChoice::High, None);
        let q_medium = CheckMenuItem::new(QualityChoice::Medium.label_for(cfg.format), true, cfg.quality == QualityChoice::Medium, None);
        let q_low = CheckMenuItem::new(QualityChoice::Low.label_for(cfg.format), true, cfg.quality == QualityChoice::Low, None);

        let quality_submenu = Submenu::new(cfg.format.quality_menu_title(), true);
        let _ = quality_submenu.append(&q_max);
        let _ = quality_submenu.append(&q_high);
        let _ = quality_submenu.append(&q_medium);
        let _ = quality_submenu.append(&q_low);

        // Submenu Escala de guardado
        let sc_full = CheckMenuItem::new(ScaleChoice::Full.label(), true, cfg.scale == ScaleChoice::Full, None);
        let sc_75 = CheckMenuItem::new(ScaleChoice::P75.label(), true, cfg.scale == ScaleChoice::P75, None);
        let sc_50 = CheckMenuItem::new(ScaleChoice::P50.label(), true, cfg.scale == ScaleChoice::P50, None);
        let sc_25 = CheckMenuItem::new(ScaleChoice::P25.label(), true, cfg.scale == ScaleChoice::P25, None);

        let scale_submenu = Submenu::new("📐 Escala de Guardado", true);
        let _ = scale_submenu.append(&sc_full);
        let _ = scale_submenu.append(&sc_75);
        let _ = scale_submenu.append(&sc_50);
        let _ = scale_submenu.append(&sc_25);

        // Submenu Atajo de Teclado
        let hk_prtscn_alta = CheckMenuItem::new("PrtScn / Alt + A", true, cfg.hotkey == HotkeyPreset::PrtScnAltA, None);
        let hk_ctrl_shift_s = CheckMenuItem::new("Ctrl + Shift + S", true, cfg.hotkey == HotkeyPreset::CtrlShiftS, None);
        let hk_alt_prtscn = CheckMenuItem::new("Alt + PrtScn", true, cfg.hotkey == HotkeyPreset::AltPrtScn, None);
        let hk_shift_prtscn = CheckMenuItem::new("Shift + PrtScn", true, cfg.hotkey == HotkeyPreset::ShiftPrtScn, None);

        let hotkey_submenu = Submenu::new("⌨️ Atajo de Teclado", true);
        let _ = hotkey_submenu.append(&hk_prtscn_alta);
        let _ = hotkey_submenu.append(&hk_ctrl_shift_s);
        let _ = hotkey_submenu.append(&hk_alt_prtscn);
        let _ = hotkey_submenu.append(&hk_shift_prtscn);

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
            fmt_webp,
            fmt_png,
            fmt_jpeg,
            q_max,
            q_high,
            q_medium,
            q_low,
            sc_full,
            sc_75,
            sc_50,
            sc_25,
            hk_prtscn_alta,
            hk_ctrl_shift_s,
            hk_alt_prtscn,
            hk_shift_prtscn,
            chk_autostart,
        })
    }

    pub fn update_checks(&self, cfg: &AppConfig) {
        self.fmt_webp.set_checked(cfg.format == ImageFormatChoice::WebP);
        self.fmt_png.set_checked(cfg.format == ImageFormatChoice::Png);
        self.fmt_jpeg.set_checked(cfg.format == ImageFormatChoice::Jpeg);

        self.q_max.set_checked(cfg.quality == QualityChoice::Max);
        self.q_high.set_checked(cfg.quality == QualityChoice::High);
        self.q_medium.set_checked(cfg.quality == QualityChoice::Medium);
        self.q_low.set_checked(cfg.quality == QualityChoice::Low);

        self.sc_full.set_checked(cfg.scale == ScaleChoice::Full);
        self.sc_75.set_checked(cfg.scale == ScaleChoice::P75);
        self.sc_50.set_checked(cfg.scale == ScaleChoice::P50);
        self.sc_25.set_checked(cfg.scale == ScaleChoice::P25);

        self.hk_prtscn_alta.set_checked(cfg.hotkey == HotkeyPreset::PrtScnAltA);
        self.hk_ctrl_shift_s.set_checked(cfg.hotkey == HotkeyPreset::CtrlShiftS);
        self.hk_alt_prtscn.set_checked(cfg.hotkey == HotkeyPreset::AltPrtScn);
        self.hk_shift_prtscn.set_checked(cfg.hotkey == HotkeyPreset::ShiftPrtScn);

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
