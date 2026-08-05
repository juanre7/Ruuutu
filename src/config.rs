use anyhow::Result;
use std::fs;
use std::path::PathBuf;

use crate::storage::{OutputFormat, SaveOptions};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormatChoice {
    WebP,
    Png,
    Jpeg,
}

impl ImageFormatChoice {
    /// Token written to and read from `config.json`.
    ///
    /// Both halves of the persistence go through this pair instead of `{:?}`: the
    /// writer used to emit the `Debug` name (`Png`) while the reader looked for
    /// `PNG`, so PNG and JPEG silently reverted to WebP on every restart.
    fn config_key(&self) -> &'static str {
        match self {
            ImageFormatChoice::WebP => "WebP",
            ImageFormatChoice::Png => "Png",
            ImageFormatChoice::Jpeg => "Jpeg",
        }
    }

    fn from_config_key(key: &str) -> Option<Self> {
        match key {
            "WebP" => Some(ImageFormatChoice::WebP),
            "Png" => Some(ImageFormatChoice::Png),
            "Jpeg" => Some(ImageFormatChoice::Jpeg),
            _ => None,
        }
    }

    #[allow(dead_code)]
    pub fn extension(&self) -> &'static str {
        match self {
            ImageFormatChoice::WebP => "webp",
            ImageFormatChoice::Png => "png",
            ImageFormatChoice::Jpeg => "jpg",
        }
    }

    pub fn to_output_format(&self) -> OutputFormat {
        match self {
            ImageFormatChoice::WebP => OutputFormat::WebP,
            ImageFormatChoice::Png => OutputFormat::Png,
            ImageFormatChoice::Jpeg => OutputFormat::Jpeg,
        }
    }

    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_ascii_lowercase().as_str() {
            "webp" => Some(ImageFormatChoice::WebP),
            "png" => Some(ImageFormatChoice::Png),
            "jpg" | "jpeg" => Some(ImageFormatChoice::Jpeg),
            _ => None,
        }
    }

    /// Short uppercase name shown on the overlay selection label.
    pub fn display_name(&self) -> &'static str {
        match self {
            ImageFormatChoice::WebP => "WEBP",
            ImageFormatChoice::Png => "PNG",
            ImageFormatChoice::Jpeg => "JPEG",
        }
    }

    /// Left-column label for the combo inside the native save dialog.
    /// No emoji and no format suffix: the dialog is already showing the file type.
    pub fn quality_dialog_label(&self) -> &'static str {
        match self {
            ImageFormatChoice::Png => "Compresión",
            _ => "Calidad",
        }
    }

    /// PNG is always lossless, so its slider controls compression effort, not quality.
    pub fn quality_menu_title(&self) -> &'static str {
        match self {
            ImageFormatChoice::Png => "🗜️ Nivel de Compresión (PNG)",
            ImageFormatChoice::WebP => "⚙️ Calidad (WebP)",
            ImageFormatChoice::Jpeg => "⚙️ Calidad (JPEG)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QualityChoice {
    Max,
    High,
    Medium,
    Low,
}

impl QualityChoice {
    fn config_key(&self) -> &'static str {
        match self {
            QualityChoice::Max => "Max",
            QualityChoice::High => "High",
            QualityChoice::Medium => "Medium",
            QualityChoice::Low => "Low",
        }
    }

    fn from_config_key(key: &str) -> Option<Self> {
        match key {
            "Max" => Some(QualityChoice::Max),
            "High" => Some(QualityChoice::High),
            "Medium" => Some(QualityChoice::Medium),
            "Low" => Some(QualityChoice::Low),
            _ => None,
        }
    }

    /// Lossy quality for WebP and JPEG.
    pub fn to_percentage(&self) -> u8 {
        match self {
            QualityChoice::Max => 100,
            QualityChoice::High => 90,
            QualityChoice::Medium => 75,
            QualityChoice::Low => 50,
        }
    }

    /// DEFLATE effort for PNG. Higher means a smaller file and a slower save,
    /// with byte-identical pixels.
    pub fn png_level(&self) -> u8 {
        match self {
            QualityChoice::Max => 9,
            QualityChoice::High => 7,
            QualityChoice::Medium => 4,
            QualityChoice::Low => 1,
        }
    }

    /// Same meaning as `label_for`, trimmed for the native dialog's combo box,
    /// which truncates at roughly 20 characters.
    pub fn short_label_for(&self, format: ImageFormatChoice) -> &'static str {
        match (format, self) {
            (ImageFormatChoice::Png, QualityChoice::Max) => "Máxima (nivel 9)",
            (ImageFormatChoice::Png, QualityChoice::High) => "Alta (nivel 7)",
            (ImageFormatChoice::Png, QualityChoice::Medium) => "Media (nivel 4)",
            (ImageFormatChoice::Png, QualityChoice::Low) => "Mínima (nivel 1)",

            (ImageFormatChoice::WebP, QualityChoice::Max) => "Sin pérdidas",
            (ImageFormatChoice::Jpeg, QualityChoice::Max) => "Máxima (100%)",

            (_, QualityChoice::High) => "Alta (90%)",
            (_, QualityChoice::Medium) => "Media (75%)",
            (_, QualityChoice::Low) => "Baja (50%)",
        }
    }

    /// Menu label. Each format gets wording that matches what the setting does.
    pub fn label_for(&self, format: ImageFormatChoice) -> &'static str {
        match (format, self) {
            (ImageFormatChoice::Png, QualityChoice::Max) => "Máxima (nivel 9, más lento)",
            (ImageFormatChoice::Png, QualityChoice::High) => "Alta (nivel 7)",
            (ImageFormatChoice::Png, QualityChoice::Medium) => "Media (nivel 4)",
            (ImageFormatChoice::Png, QualityChoice::Low) => "Mínima (nivel 1, más rápido)",

            (ImageFormatChoice::WebP, QualityChoice::Max) => "Sin pérdidas (VP8L)",
            (ImageFormatChoice::WebP, QualityChoice::High) => "Alta (90%)",
            (ImageFormatChoice::WebP, QualityChoice::Medium) => "Media (75%)",
            (ImageFormatChoice::WebP, QualityChoice::Low) => "Baja (50%)",

            (ImageFormatChoice::Jpeg, QualityChoice::Max) => "Máxima (100%)",
            (ImageFormatChoice::Jpeg, QualityChoice::High) => "Alta (90%)",
            (ImageFormatChoice::Jpeg, QualityChoice::Medium) => "Media (75%)",
            (ImageFormatChoice::Jpeg, QualityChoice::Low) => "Baja (50%)",
        }
    }
}

/// Downscale applied to the captured selection before it is written to disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScaleChoice {
    Full,
    P75,
    P50,
    P25,
}

impl ScaleChoice {
    /// The scale is stored as its bare percentage, so the key is numeric.
    fn from_config_key(key: &str) -> Option<Self> {
        match key {
            "100" => Some(ScaleChoice::Full),
            "75" => Some(ScaleChoice::P75),
            "50" => Some(ScaleChoice::P50),
            "25" => Some(ScaleChoice::P25),
            _ => None,
        }
    }

    pub fn percent(&self) -> u32 {
        match self {
            ScaleChoice::Full => 100,
            ScaleChoice::P75 => 75,
            ScaleChoice::P50 => 50,
            ScaleChoice::P25 => 25,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            ScaleChoice::Full => "100% (Resolución original)",
            ScaleChoice::P75 => "75%",
            ScaleChoice::P50 => "50% (1080p → 540p)",
            ScaleChoice::P25 => "25%",
        }
    }

    /// Trimmed for the native dialog's combo box.
    pub fn short_label(&self) -> &'static str {
        match self {
            ScaleChoice::Full => "100% (original)",
            ScaleChoice::P75 => "75%",
            ScaleChoice::P50 => "50%",
            ScaleChoice::P25 => "25%",
        }
    }
}

/// Scale applied to the capture overlay's interface.
///
/// Nominally "text scale", and the text is what it is for — but the boxes that hold the
/// text scale with it (button height, paddings, icons, the dimension label). Growing the
/// glyphs alone would just clip them against a 34 px button.
///
/// Stored as a percentage, like [`ScaleChoice`], under a separate `text_scale` key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextScaleChoice {
    P50,
    P75,
    Full,
    P125,
    P150,
    P200,
}

impl TextScaleChoice {
    /// Every variant, in menu order.
    pub const ALL: [TextScaleChoice; 6] = [
        TextScaleChoice::P50,
        TextScaleChoice::P75,
        TextScaleChoice::Full,
        TextScaleChoice::P125,
        TextScaleChoice::P150,
        TextScaleChoice::P200,
    ];

    pub fn percent(&self) -> u32 {
        match self {
            TextScaleChoice::P50 => 50,
            TextScaleChoice::P75 => 75,
            TextScaleChoice::Full => 100,
            TextScaleChoice::P125 => 125,
            TextScaleChoice::P150 => 150,
            TextScaleChoice::P200 => 200,
        }
    }

    /// Multiplier handed to the overlay.
    pub fn factor(&self) -> f32 {
        self.percent() as f32 / 100.0
    }

    pub fn label(&self) -> &'static str {
        match self {
            TextScaleChoice::P50 => "0,5x (más pequeño)",
            TextScaleChoice::P75 => "0,75x",
            TextScaleChoice::Full => "1x (predeterminado)",
            TextScaleChoice::P125 => "1,25x",
            TextScaleChoice::P150 => "1,5x",
            TextScaleChoice::P200 => "2x (más grande)",
        }
    }

    fn config_key(&self) -> String {
        self.percent().to_string()
    }

    fn from_config_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.config_key() == key)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyPreset {
    PrtScnAltA,
    CtrlShiftS,
    AltPrtScn,
    ShiftPrtScn,
}

impl HotkeyPreset {
    fn config_key(&self) -> &'static str {
        match self {
            HotkeyPreset::PrtScnAltA => "PrtScnAltA",
            HotkeyPreset::CtrlShiftS => "CtrlShiftS",
            HotkeyPreset::AltPrtScn => "AltPrtScn",
            HotkeyPreset::ShiftPrtScn => "ShiftPrtScn",
        }
    }

    fn from_config_key(key: &str) -> Option<Self> {
        match key {
            "PrtScnAltA" => Some(HotkeyPreset::PrtScnAltA),
            "CtrlShiftS" => Some(HotkeyPreset::CtrlShiftS),
            "AltPrtScn" => Some(HotkeyPreset::AltPrtScn),
            "ShiftPrtScn" => Some(HotkeyPreset::ShiftPrtScn),
            _ => None,
        }
    }

    /// `true` when the preset is fired by the PrtScn key, which is the only case
    /// where the low-level keyboard hook has any business consuming it.
    pub fn uses_print_screen(&self) -> bool {
        !matches!(self, HotkeyPreset::CtrlShiftS)
    }
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub format: ImageFormatChoice,
    pub quality: QualityChoice,
    pub scale: ScaleChoice,
    /// Interface scale of the capture overlay. Unrelated to `scale`, which resizes the
    /// saved image.
    pub text_scale: TextScaleChoice,
    pub hotkey: HotkeyPreset,
    pub autostart: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            format: ImageFormatChoice::WebP,
            quality: QualityChoice::High,
            scale: ScaleChoice::Full,
            text_scale: TextScaleChoice::Full,
            hotkey: HotkeyPreset::PrtScnAltA,
            autostart: false,
        }
    }
}

pub fn get_config_dir() -> Result<PathBuf> {
    let base = dirs::config_dir()
        .or_else(dirs::data_dir)
        .unwrap_or_else(|| PathBuf::from("."));
    let dir = base.join("Ruuutu");
    if !dir.exists() {
        let _ = fs::create_dir_all(&dir);
    }
    Ok(dir)
}

fn get_config_path() -> Result<PathBuf> {
    Ok(get_config_dir()?.join("config.json"))
}

/// Reads the value of `"key"` out of the flat config JSON.
///
/// Hand-rolled instead of serde purely for binary size, as the rest of the file.
/// It looks up the key by name rather than matching loose substrings anywhere in
/// the document, so an unrelated value can never be mistaken for a setting.
/// Quotes around string values are stripped; numbers and booleans come back raw.
fn json_field<'a>(content: &'a str, key: &str) -> Option<&'a str> {
    let needle = format!("\"{}\"", key);
    let after_key = content.get(content.find(&needle)? + needle.len()..)?;
    let value = after_key.trim_start().strip_prefix(':')?.trim_start();

    match value.strip_prefix('"') {
        Some(quoted) => quoted.split('"').next(),
        None => Some(
            value
                .split([',', '\n', '}'])
                .next()
                .unwrap_or(value)
                .trim(),
        ),
    }
}

impl AppConfig {
    pub fn load() -> Self {
        let mut cfg = get_config_path()
            .ok()
            .and_then(|path| fs::read_to_string(path).ok())
            .map(|content| AppConfig::from_json(&content))
            .unwrap_or_default();

        // The registry, not the file, is the source of truth for autostart.
        cfg.autostart = is_autostart_enabled();
        cfg
    }

    /// Parses the config document. Every field falls back to its default
    /// independently, so an unknown or corrupt value only loses that setting.
    ///
    /// `autostart` is deliberately not read here: `load()` takes it from the
    /// registry, which is where it actually lives.
    pub fn from_json(content: &str) -> Self {
        let defaults = AppConfig::default();
        AppConfig {
            format: json_field(content, "format")
                .and_then(ImageFormatChoice::from_config_key)
                .unwrap_or(defaults.format),
            quality: json_field(content, "quality")
                .and_then(QualityChoice::from_config_key)
                .unwrap_or(defaults.quality),
            scale: json_field(content, "scale")
                .and_then(ScaleChoice::from_config_key)
                .unwrap_or(defaults.scale),
            text_scale: json_field(content, "text_scale")
                .and_then(TextScaleChoice::from_config_key)
                .unwrap_or(defaults.text_scale),
            hotkey: json_field(content, "hotkey")
                .and_then(HotkeyPreset::from_config_key)
                .unwrap_or(defaults.hotkey),
            autostart: defaults.autostart,
        }
    }

    /// Serializes the config. Must stay symmetric with `from_json`.
    pub fn to_json(&self) -> String {
        format!(
            "{{\n  \"format\": \"{}\",\n  \"quality\": \"{}\",\n  \"scale\": {},\n  \"text_scale\": {},\n  \"hotkey\": \"{}\",\n  \"autostart\": {}\n}}",
            self.format.config_key(),
            self.quality.config_key(),
            self.scale.percent(),
            self.text_scale.config_key(),
            self.hotkey.config_key(),
            self.autostart
        )
    }

    pub fn save(&self) -> Result<()> {
        let path = get_config_path()?;
        fs::write(path, self.to_json())?;
        Ok(())
    }

    /// Resolves the tray settings into concrete encoder parameters.
    pub fn save_options(&self) -> SaveOptions {
        SaveOptions {
            format: self.format.to_output_format(),
            quality: self.quality.to_percentage(),
            // Only WebP can choose; PNG is inherently lossless and JPEG never is.
            lossless: self.format == ImageFormatChoice::WebP && self.quality == QualityChoice::Max,
            png_level: self.quality.png_level(),
            scale_percent: self.scale.percent(),
        }
    }
}

/// Windows Registry Portable Autostart Helper (HKCU\Software\Microsoft\Windows\CurrentVersion\Run)
pub fn is_autostart_enabled() -> bool {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Registry::{RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY_CURRENT_USER, KEY_READ};
        let subkey: Vec<u16> = "Software\\Microsoft\\Windows\\CurrentVersion\\Run\0".encode_utf16().collect();
        let value_name: Vec<u16> = "Ruuutu\0".encode_utf16().collect();

        unsafe {
            let mut hkey = 0;
            if RegOpenKeyExW(HKEY_CURRENT_USER, subkey.as_ptr(), 0, KEY_READ, &mut hkey) == 0 {
                let mut type_reg = 0;
                let mut size = 0;
                let res = RegQueryValueExW(hkey, value_name.as_ptr(), std::ptr::null_mut(), &mut type_reg, std::ptr::null_mut(), &mut size);
                RegCloseKey(hkey);
                return res == 0 && size > 0;
            }
        }
    }
    false
}

pub fn set_autostart_enabled(enable: bool) -> Result<()> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Registry::{
            RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegSetValueExW, HKEY_CURRENT_USER, KEY_WRITE, REG_SZ,
        };

        let current_exe = std::env::current_exe()?;
        let exe_str = format!("\"{}\"", current_exe.to_string_lossy());
        let subkey: Vec<u16> = "Software\\Microsoft\\Windows\\CurrentVersion\\Run\0".encode_utf16().collect();
        let value_name: Vec<u16> = "Ruuutu\0".encode_utf16().collect();

        unsafe {
            let mut hkey = 0;
            if RegOpenKeyExW(HKEY_CURRENT_USER, subkey.as_ptr(), 0, KEY_WRITE, &mut hkey) == 0 {
                if enable {
                    let val_bytes: Vec<u16> = exe_str.encode_utf16().chain(std::iter::once(0)).collect();
                    RegSetValueExW(
                        hkey,
                        value_name.as_ptr(),
                        0,
                        REG_SZ,
                        val_bytes.as_ptr() as *const u8,
                        (val_bytes.len() * 2) as u32,
                    );
                } else {
                    RegDeleteValueW(hkey, value_name.as_ptr());
                }
                RegCloseKey(hkey);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const FORMATS: [ImageFormatChoice; 3] = [
        ImageFormatChoice::WebP,
        ImageFormatChoice::Png,
        ImageFormatChoice::Jpeg,
    ];
    const QUALITIES: [QualityChoice; 4] = [
        QualityChoice::Max,
        QualityChoice::High,
        QualityChoice::Medium,
        QualityChoice::Low,
    ];
    const SCALES: [ScaleChoice; 4] = [
        ScaleChoice::Full,
        ScaleChoice::P75,
        ScaleChoice::P50,
        ScaleChoice::P25,
    ];
    const HOTKEYS: [HotkeyPreset; 4] = [
        HotkeyPreset::PrtScnAltA,
        HotkeyPreset::CtrlShiftS,
        HotkeyPreset::AltPrtScn,
        HotkeyPreset::ShiftPrtScn,
    ];

    /// Every combination must survive a write/read cycle. The PNG and JPEG cases
    /// are the regression: the writer emitted `Png` while the reader looked for
    /// `PNG`, so both formats silently reverted to WebP on restart.
    #[test]
    fn every_setting_survives_a_round_trip() {
        for format in FORMATS {
            for quality in QUALITIES {
                for scale in SCALES {
                    for text_scale in TextScaleChoice::ALL {
                        for hotkey in HOTKEYS {
                            let cfg = AppConfig { format, quality, scale, text_scale, hotkey, autostart: false };
                            let json = cfg.to_json();
                            let parsed = AppConfig::from_json(&json);
                            assert_eq!(parsed.format, format, "format lost: {}", json);
                            assert_eq!(parsed.quality, quality, "quality lost: {}", json);
                            assert_eq!(parsed.scale, scale, "scale lost: {}", json);
                            assert_eq!(parsed.text_scale, text_scale, "text_scale lost: {}", json);
                            assert_eq!(parsed.hotkey, hotkey, "hotkey lost: {}", json);
                        }
                    }
                }
            }
        }
    }

    /// `scale` and `text_scale` are both bare percentages and one key name contains
    /// the other, so they are the pair most likely to be read into each other.
    #[test]
    fn image_scale_and_text_scale_stay_independent() {
        for scale in SCALES {
            for text_scale in TextScaleChoice::ALL {
                let cfg = AppConfig { scale, text_scale, ..AppConfig::default() };
                let parsed = AppConfig::from_json(&cfg.to_json());
                assert_eq!(parsed.scale, scale);
                assert_eq!(parsed.text_scale, text_scale);
            }
        }

        // Also with the keys written in the opposite order to `to_json`.
        let cfg = AppConfig::from_json(r#"{"text_scale": 200, "scale": 25}"#);
        assert_eq!(cfg.scale, ScaleChoice::P25);
        assert_eq!(cfg.text_scale, TextScaleChoice::P200);
    }

    /// A config written by an older build has no `text_scale`, and must not lose
    /// the settings that are there.
    #[test]
    fn a_config_without_text_scale_still_loads() {
        let cfg = AppConfig::from_json(
            "{\n  \"format\": \"Jpeg\",\n  \"quality\": \"Low\",\n  \"scale\": 50,\n  \"hotkey\": \"AltPrtScn\",\n  \"autostart\": true\n}",
        );
        assert_eq!(cfg.format, ImageFormatChoice::Jpeg);
        assert_eq!(cfg.scale, ScaleChoice::P50);
        assert_eq!(cfg.text_scale, TextScaleChoice::Full);
    }

    #[test]
    fn unknown_and_missing_values_fall_back_to_defaults() {
        let defaults = AppConfig::default();

        let empty = AppConfig::from_json("{}");
        assert_eq!(empty.format, defaults.format);
        assert_eq!(empty.quality, defaults.quality);
        assert_eq!(empty.scale, defaults.scale);
        assert_eq!(empty.hotkey, defaults.hotkey);

        let garbage = AppConfig::from_json(r#"{"format": "TIFF", "scale": 3}"#);
        assert_eq!(garbage.format, defaults.format);
        assert_eq!(garbage.scale, defaults.scale);
    }

    /// A stale or unrelated key must not be able to impersonate a setting. The old
    /// `contains()` parser matched any occurrence anywhere in the document.
    #[test]
    fn unrelated_keys_do_not_leak_into_settings() {
        let cfg = AppConfig::from_json(
            r#"{
  "comment": "Png Max 75 CtrlShiftS",
  "format": "Jpeg",
  "quality": "Low",
  "scale": 25,
  "hotkey": "AltPrtScn",
  "indent": 0
}"#,
        );
        assert_eq!(cfg.format, ImageFormatChoice::Jpeg);
        assert_eq!(cfg.quality, QualityChoice::Low);
        assert_eq!(cfg.scale, ScaleChoice::P25);
        assert_eq!(cfg.hotkey, HotkeyPreset::AltPrtScn);
    }

    #[test]
    fn json_field_reads_strings_numbers_and_booleans() {
        let doc = r#"{"a": "text", "b": 42, "c": true}"#;
        assert_eq!(json_field(doc, "a"), Some("text"));
        assert_eq!(json_field(doc, "b"), Some("42"));
        assert_eq!(json_field(doc, "c"), Some("true"));
        assert_eq!(json_field(doc, "missing"), None);
    }

    /// WebP is the only format that can choose, and only at maximum quality.
    #[test]
    fn lossless_is_webp_at_max_only() {
        for format in FORMATS {
            for quality in QUALITIES {
                let cfg = AppConfig { format, quality, ..AppConfig::default() };
                let expected =
                    format == ImageFormatChoice::WebP && quality == QualityChoice::Max;
                assert_eq!(cfg.save_options().lossless, expected);
            }
        }
    }
}

