// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 juanre7

//! Native Windows "Guardar como" dialog with Ruuutu's encoder settings embedded in it.
//!
//! `rfd` only exposes extension filters, so the quality and scale controls are added
//! through `IFileDialogCustomize` on a real `IFileSaveDialog`. Everything here is COM,
//! and it inherits the same constraint as the rest of the OLE code: it must never run
//! inside a window event callback. Call it from `about_to_wait`, after the overlay
//! window has been destroyed.
//!
//! Layout note: `IFileDialogCustomize` draws the left-hand label from the *visual
//! group*, not from the control. Each combo therefore lives in its own single-control
//! group, which is what produces the "Calidad:" / "Escala:" captions. The check button
//! sits outside any group because its own text is already its label.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use windows::core::{implement, Interface, Ref, HSTRING, PCWSTR};
use windows::Win32::Foundation::ERROR_CANCELLED;
use windows::Win32::System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_INPROC_SERVER};
use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
use windows::Win32::UI::Shell::{
    FileSaveDialog, IFileDialog, IFileDialogCustomize, IFileDialogEvents, IFileDialogEvents_Impl,
    IFileSaveDialog, IShellItem, SHCreateItemFromParsingName, FDEOR_DEFAULT, FDESVR_DEFAULT,
    FDE_OVERWRITE_RESPONSE, FDE_SHAREVIOLATION_RESPONSE, SIGDN_FILESYSPATH,
};

use crate::config::{ImageFormatChoice, QualityChoice, ScaleChoice};

const ID_GROUP_QUALITY: u32 = 1000;
const ID_QUALITY: u32 = 1001;
const ID_GROUP_SCALE: u32 = 1002;
const ID_SCALE: u32 = 1003;
const ID_REMEMBER: u32 = 1005;

/// File type order. The 1-based position in this array is the dialog's file type index.
const FORMATS: [ImageFormatChoice; 3] = [
    ImageFormatChoice::WebP,
    ImageFormatChoice::Png,
    ImageFormatChoice::Jpeg,
];

/// Combo order. The index in these arrays is the control item id.
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

/// What the user picked, once the dialog closed with a confirmed path.
pub struct DialogOutcome {
    pub path: PathBuf,
    pub format: ImageFormatChoice,
    pub quality: QualityChoice,
    pub scale: ScaleChoice,
    /// `true` when "Recordar estos ajustes" was ticked.
    pub remember: bool,
}

fn index_of<T: PartialEq + Copy>(items: &[T], value: T) -> u32 {
    items.iter().position(|&i| i == value).unwrap_or(0) as u32
}

fn format_at_type_index(index: u32) -> ImageFormatChoice {
    FORMATS
        .get(index.saturating_sub(1) as usize)
        .copied()
        .unwrap_or(ImageFormatChoice::WebP)
}

/// Retitles the quality group and its items to match `format`.
///
/// This is the whole point of the events hook: "Máxima" means lossless VP8L in WebP,
/// DEFLATE level 9 in PNG and quality 100 in JPEG, exactly as in the tray menu, so the
/// wording has to follow the file type the user selects inside the dialog.
///
/// # Safety
/// `custom` must be a live `IFileDialogCustomize` obtained from the running dialog.
unsafe fn apply_format_labels(custom: &IFileDialogCustomize, format: ImageFormatChoice) {
    unsafe {
        let _ = custom.SetControlLabel(
            ID_GROUP_QUALITY,
            &HSTRING::from(format.quality_dialog_label()),
        );

        // `SetControlItemText` updates the item but does not repaint a combo box that
        // is already populated, so the list is torn down and rebuilt instead.
        // Removing the items also clears the selection, hence the read-back here and
        // the restore at the end.
        let selected = custom.GetSelectedControlItem(ID_QUALITY).unwrap_or(0);

        for idx in 0..QUALITIES.len() as u32 {
            let _ = custom.RemoveControlItem(ID_QUALITY, idx);
        }
        for (idx, q) in QUALITIES.iter().enumerate() {
            let _ = custom.AddControlItem(
                ID_QUALITY,
                idx as u32,
                &HSTRING::from(q.short_label_for(format)),
            );
        }

        let _ = custom.SetSelectedControlItem(ID_QUALITY, selected);
    }
}

/// Relabels the quality combo whenever the user switches file type mid-dialog.
#[implement(IFileDialogEvents)]
struct TypeChangeHandler;

#[allow(non_snake_case)]
impl IFileDialogEvents_Impl for TypeChangeHandler_Impl {
    fn OnTypeChange(&self, pfd: Ref<IFileDialog>) -> windows::core::Result<()> {
        let Some(dialog) = pfd.as_ref() else {
            return Ok(());
        };
        unsafe {
            if let Ok(custom) = dialog.cast::<IFileDialogCustomize>() {
                let index = dialog.GetFileTypeIndex().unwrap_or(1);
                apply_format_labels(&custom, format_at_type_index(index));
            }
        }
        Ok(())
    }

    fn OnFileOk(&self, _pfd: Ref<IFileDialog>) -> windows::core::Result<()> {
        Ok(())
    }
    fn OnFolderChanging(
        &self,
        _pfd: Ref<IFileDialog>,
        _psifolder: Ref<IShellItem>,
    ) -> windows::core::Result<()> {
        Ok(())
    }
    fn OnFolderChange(&self, _pfd: Ref<IFileDialog>) -> windows::core::Result<()> {
        Ok(())
    }
    fn OnSelectionChange(&self, _pfd: Ref<IFileDialog>) -> windows::core::Result<()> {
        Ok(())
    }
    fn OnShareViolation(
        &self,
        _pfd: Ref<IFileDialog>,
        _psi: Ref<IShellItem>,
    ) -> windows::core::Result<FDE_SHAREVIOLATION_RESPONSE> {
        Ok(FDESVR_DEFAULT)
    }
    fn OnOverwrite(
        &self,
        _pfd: Ref<IFileDialog>,
        _psi: Ref<IShellItem>,
    ) -> windows::core::Result<FDE_OVERWRITE_RESPONSE> {
        Ok(FDEOR_DEFAULT)
    }
}

/// A fully configured dialog that has not been shown yet.
///
/// Building and showing are split so the COM plumbing — instantiation, the
/// `IFileDialogCustomize` cast, the events hook and every control — can be exercised
/// by `test_bench` without a modal window blocking the run.
pub struct PreparedDialog {
    dialog: IFileSaveDialog,
    custom: IFileDialogCustomize,
    /// Kept alive: `COMDLG_FILTERSPEC` holds raw pointers into these strings.
    _types: Vec<(HSTRING, HSTRING)>,
    /// Advise cookie, released after the dialog closes.
    cookie: u32,
    fallback: (ImageFormatChoice, QualityChoice, ScaleChoice),
}

/// Builds and shows the dialog. `Ok(None)` means the user cancelled.
pub fn show_save_dialog(
    default_name: &str,
    initial_dir: &Path,
    format: ImageFormatChoice,
    quality: QualityChoice,
    scale: ScaleChoice,
) -> Result<Option<DialogOutcome>> {
    prepare_dialog(default_name, initial_dir, format, quality, scale)?.show()
}

/// Creates the dialog and attaches Ruuutu's controls, without showing it.
pub fn prepare_dialog(
    default_name: &str,
    initial_dir: &Path,
    format: ImageFormatChoice,
    quality: QualityChoice,
    scale: ScaleChoice,
) -> Result<PreparedDialog> {
    unsafe {
        let dialog: IFileSaveDialog = CoCreateInstance(&FileSaveDialog, None, CLSCTX_INPROC_SERVER)
            .context("Failed to create the native IFileSaveDialog")?;

        dialog.SetTitle(&HSTRING::from("Guardar captura de pantalla como..."))?;
        dialog.SetFileName(&HSTRING::from(default_name))?;

        // A missing folder is not fatal; the dialog just opens wherever Windows prefers.
        if let Ok(item) =
            SHCreateItemFromParsingName::<_, _, IShellItem>(&HSTRING::from(initial_dir), None)
        {
            let _ = dialog.SetFolder(&item);
        }

        // The HSTRINGs must outlive SetFileTypes, so bind them before building the specs.
        let types: Vec<(HSTRING, HSTRING)> = FORMATS
            .iter()
            .map(|f| {
                let (name, spec) = match f {
                    ImageFormatChoice::WebP => ("Imagen WebP (*.webp)", "*.webp"),
                    ImageFormatChoice::Png => ("Imagen PNG (*.png)", "*.png"),
                    ImageFormatChoice::Jpeg => ("Imagen JPEG (*.jpg)", "*.jpg;*.jpeg"),
                };
                (HSTRING::from(name), HSTRING::from(spec))
            })
            .collect();

        let specs: Vec<COMDLG_FILTERSPEC> = types
            .iter()
            .map(|(name, spec)| COMDLG_FILTERSPEC {
                pszName: PCWSTR(name.as_ptr()),
                pszSpec: PCWSTR(spec.as_ptr()),
            })
            .collect();

        dialog.SetFileTypes(&specs)?;
        let _ = dialog.SetFileTypeIndex(index_of(&FORMATS, format) + 1); // 1-based
        let _ = dialog.SetDefaultExtension(&HSTRING::from(format.extension()));

        // Ruuutu's own controls, appended below the standard file name row.
        let custom: IFileDialogCustomize = dialog
            .cast()
            .context("IFileSaveDialog does not expose IFileDialogCustomize")?;

        custom.StartVisualGroup(
            ID_GROUP_QUALITY,
            &HSTRING::from(format.quality_dialog_label()),
        )?;
        custom.AddComboBox(ID_QUALITY)?;
        for (idx, q) in QUALITIES.iter().enumerate() {
            custom.AddControlItem(
                ID_QUALITY,
                idx as u32,
                &HSTRING::from(q.short_label_for(format)),
            )?;
        }
        custom.SetSelectedControlItem(ID_QUALITY, index_of(&QUALITIES, quality))?;
        custom.EndVisualGroup()?;

        custom.StartVisualGroup(ID_GROUP_SCALE, &HSTRING::from("Escala"))?;
        custom.AddComboBox(ID_SCALE)?;
        for (idx, s) in SCALES.iter().enumerate() {
            custom.AddControlItem(ID_SCALE, idx as u32, &HSTRING::from(s.short_label()))?;
        }
        custom.SetSelectedControlItem(ID_SCALE, index_of(&SCALES, scale))?;
        custom.EndVisualGroup()?;

        // Loose, outside any group: its own text is already its label.
        //
        // The shell packs it into the free cell under the scale combo, which makes that
        // column taller than the quality one and top-aligns the "Escala" caption by a
        // few pixels. That is not fixable here: `IFileDialogCustomize` exposes no
        // placement, sizing or alignment control, and wrapping the button in its own
        // visual group (labelled or empty) does not move it to a row of its own — it
        // only adds a stray caption column. Tried and reverted; do not retry.
        custom.AddCheckButton(
            ID_REMEMBER,
            &HSTRING::from("Recordar estos ajustes"),
            false,
        )?;

        // Keep the quality wording in sync when the file type changes mid-dialog.
        let events: IFileDialogEvents = TypeChangeHandler.into();
        let cookie = dialog.Advise(&events).unwrap_or(0);

        Ok(PreparedDialog {
            dialog,
            custom,
            _types: types,
            cookie,
            fallback: (format, quality, scale),
        })
    }
}

impl PreparedDialog {
    /// Shows the modal dialog and reads the controls back. `Ok(None)` on cancel.
    ///
    /// Like every other OLE call in Ruuutu, this blocks and must not run inside a
    /// window event callback.
    pub fn show(self) -> Result<Option<DialogOutcome>> {
        let PreparedDialog {
            dialog,
            custom,
            _types,
            cookie,
            fallback: (format, quality, scale),
        } = self;

        unsafe {
            // Cancelling is a normal outcome, not an error: Windows reports it as
            // ERROR_CANCELLED wrapped in an HRESULT.
            let shown = dialog.Show(None);
            if let Err(e) = shown {
                if cookie != 0 {
                    let _ = dialog.Unadvise(cookie);
                }
                if e.code() == windows::core::HRESULT::from_win32(ERROR_CANCELLED.0) {
                    return Ok(None);
                }
                return Err(anyhow::anyhow!("Save dialog failed: {:?}", e));
            }

            // Read the controls back before releasing anything.
            let quality = QUALITIES
                .get(custom.GetSelectedControlItem(ID_QUALITY).unwrap_or(0) as usize)
                .copied()
                .unwrap_or(quality);
            let scale = SCALES
                .get(custom.GetSelectedControlItem(ID_SCALE).unwrap_or(0) as usize)
                .copied()
                .unwrap_or(scale);
            let remember = custom
                .GetCheckButtonState(ID_REMEMBER)
                .map(|b| b.as_bool())
                .unwrap_or(false);

            let type_index = dialog.GetFileTypeIndex().ok();

            let item = dialog.GetResult().context("Save dialog returned no item")?;
            let wide = item
                .GetDisplayName(SIGDN_FILESYSPATH)
                .context("Could not read the chosen path")?;
            let path = PathBuf::from(wide.to_string()?);
            CoTaskMemFree(Some(wide.0 as *const core::ffi::c_void));

            if cookie != 0 {
                let _ = dialog.Unadvise(cookie);
            }

            // The extension the user typed wins over the selected file type, so derive
            // the format from the final path and fall back to the type index.
            let format = path
                .extension()
                .and_then(|e| e.to_str())
                .and_then(ImageFormatChoice::from_extension)
                .or_else(|| type_index.map(format_at_type_index))
                .unwrap_or(format);

            Ok(Some(DialogOutcome {
                path,
                format,
                quality,
                scale,
                remember,
            }))
        }
    }
}
