use anyhow::{ensure, Result};
use std::path::{Path, PathBuf};

#[cfg(windows)]
pub fn choose_json(save: bool) -> Result<Option<PathBuf>> {
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::UI::Controls::Dialogs::{
        CommDlgExtendedError, GetOpenFileNameW, GetSaveFileNameW, OFN_EXPLORER, OFN_FILEMUSTEXIST,
        OFN_NOCHANGEDIR, OFN_OVERWRITEPROMPT, OFN_PATHMUSTEXIST, OPENFILENAMEW,
    };
    let filter: Vec<u16> = "JSON history (*.json)\0*.json\0\0".encode_utf16().collect();
    let extension: Vec<u16> = "json\0".encode_utf16().collect();
    let title: Vec<u16> = if save {
        "Export displayed games\0"
    } else {
        "Import game history\0"
    }
    .encode_utf16()
    .collect();
    let mut filename = vec![0u16; 32768];
    if save {
        let name: Vec<u16> = "shc-games.json".encode_utf16().collect();
        filename[..name.len()].copy_from_slice(&name);
    }
    let mut options = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        lpstrFilter: filter.as_ptr(),
        nFilterIndex: 1,
        lpstrFile: filename.as_mut_ptr(),
        nMaxFile: filename.len() as u32,
        lpstrTitle: title.as_ptr(),
        lpstrDefExt: extension.as_ptr(),
        Flags: OFN_EXPLORER
            | OFN_NOCHANGEDIR
            | OFN_PATHMUSTEXIST
            | if save {
                OFN_OVERWRITEPROMPT
            } else {
                OFN_FILEMUSTEXIST
            },
        ..Default::default()
    };
    let accepted = unsafe {
        if save {
            GetSaveFileNameW(&mut options)
        } else {
            GetOpenFileNameW(&mut options)
        }
    };
    if accepted == 0 {
        let error = unsafe { CommDlgExtendedError() };
        ensure!(error == 0, "Could not open history file dialog ({error})");
        return Ok(None);
    }
    let end = filename
        .iter()
        .position(|ch| *ch == 0)
        .unwrap_or(filename.len());
    Ok(Some(PathBuf::from(std::ffi::OsString::from_wide(
        &filename[..end],
    ))))
}

#[cfg(not(windows))]
pub fn choose_json(_save: bool) -> Result<Option<PathBuf>> {
    anyhow::bail!("History file dialogs require Windows")
}

#[cfg(windows)]
pub fn open_folder(path: &Path) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL};
    let operation: Vec<u16> = "open\0".encode_utf16().collect();
    let path: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            operation.as_ptr(),
            path.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    ensure!(
        (result as isize) > 32,
        "Windows could not open the history folder"
    );
    Ok(())
}

#[cfg(not(windows))]
pub fn open_folder(_path: &Path) -> Result<()> {
    anyhow::bail!("Opening the history folder requires Windows")
}
