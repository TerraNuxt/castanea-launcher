//! Moving a file to the system's trash: the FreeDesktop trash on Linux, the Recycle Bin on Windows, `~/.Trash` on macOS.
//! When that is not possible (another disk, no trash folder) the caller asks before deleting for good.

use std::fs;
use std::path::{Path, PathBuf};

use super::time::{Day, day_of};

/// Percent-encodes a path for a `.trashinfo` file: unreserved characters and `/` stay.
pub fn encode_path(path: &str) -> String {
    let mut out = String::new();
    for b in path.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~' | b'/') {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// The text of a `.trashinfo` file (FreeDesktop trash specification 1.0).
pub fn info_text(original: &Path, unix_seconds: i64) -> String {
    let day: Day = day_of(unix_seconds);
    let of_day = unix_seconds.rem_euclid(86_400);
    format!(
        "[Trash Info]\nPath={}\nDeletionDate={}T{:02}:{:02}:{:02}\n",
        encode_path(&original.to_string_lossy()),
        day.iso(),
        of_day / 3600,
        of_day / 60 % 60,
        of_day % 60
    )
}

/// A name for `file_name` that is free in both `files` and `info` of the trash folder: `a.png`, `a.2.png`, `a.3.png`, ...
pub fn free_name(trash: &Path, file_name: &str) -> String {
    let taken = |name: &str| {
        trash.join("files").join(name).exists() || trash.join("info").join(format!("{name}.trashinfo")).exists()
    };
    if !taken(file_name) {
        return file_name.to_owned();
    }
    let (stem, ext) = match file_name.rsplit_once('.') {
        Some((stem, ext)) => (stem, format!(".{ext}")),
        None => (file_name, String::new()),
    };
    (2..).map(|n| format!("{stem}.{n}{ext}")).find(|name| !taken(name)).unwrap_or_else(|| file_name.to_owned())
}

/// Moves `file` into the trash folder `trash` (`files/` and `info/` below it), as the FreeDesktop specification says:
/// the info file first, then the rename; a rename that fails (another disk) removes the info file again and fails.
pub fn trash_into(trash: &Path, file: &Path, unix_seconds: i64) -> Result<PathBuf, String> {
    let name = file.file_name().and_then(|n| n.to_str()).ok_or("the file has no usable name")?;
    let absolute = fs::canonicalize(file).map_err(|e| format!("{}: {e}", file.display()))?;
    for sub in ["files", "info"] {
        fs::create_dir_all(trash.join(sub)).map_err(|e| format!("{}: {e}", trash.join(sub).display()))?;
    }
    let target_name = free_name(trash, name);
    let info = trash.join("info").join(format!("{target_name}.trashinfo"));
    fs::write(&info, info_text(&absolute, unix_seconds)).map_err(|e| format!("{}: {e}", info.display()))?;
    let target = trash.join("files").join(&target_name);
    if let Err(error) = fs::rename(file, &target) {
        let _ = fs::remove_file(&info);
        return Err(format!("{}: {error}", file.display()));
    }
    Ok(target)
}

/// Moves `file` to the system's trash, or says why it cannot.
pub fn move_to_trash(file: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        windows::recycle(file)
    }
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var_os("HOME").ok_or("no home folder")?;
        let dir = Path::new(&home).join(".Trash");
        let name = file.file_name().ok_or("the file has no name")?;
        let mut target = dir.join(name);
        let mut n = 1;
        while target.exists() {
            n += 1;
            target = dir.join(format!("{n} {}", name.to_string_lossy()));
        }
        fs::rename(file, &target).map_err(|e| e.to_string())
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let base = std::env::var_os("XDG_DATA_HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| Path::new(&h).join(".local/share")))
            .ok_or("no home folder")?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0));
        let local = now + super::time::local_offset(now);
        trash_into(&base.join("Trash"), file, local).map(|_| ())
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = file;
        Err("this system has no trash the launcher knows".to_owned())
    }
}

#[cfg(windows)]
mod windows {
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;

    const FO_DELETE: u32 = 3;
    const FOF_SILENT: u16 = 0x0004;
    const FOF_NOCONFIRMATION: u16 = 0x0010;
    const FOF_ALLOWUNDO: u16 = 0x0040;
    const FOF_NOERRORUI: u16 = 0x0400;

    #[repr(C)]
    struct FileOp {
        hwnd: isize,
        func: u32,
        from: *const u16,
        to: *const u16,
        flags: u16,
        aborted: i32,
        mappings: *mut core::ffi::c_void,
        title: *const u16,
    }

    #[link(name = "shell32")]
    unsafe extern "system" {
        fn SHFileOperationW(op: *mut FileOp) -> i32;
    }

    /// Sends the file to the Recycle Bin (`FOF_ALLOWUNDO`), with no dialog.
    pub fn recycle(file: &Path) -> Result<(), String> {
        if cfg!(not(target_pointer_width = "64")) {
            return Err("the Recycle Bin call is only declared for 64-bit Windows".to_owned());
        }
        // The path list ends with two NULs.
        let mut from: Vec<u16> = file.as_os_str().encode_wide().collect();
        from.extend([0, 0]);
        let mut op = FileOp {
            hwnd: 0,
            func: FO_DELETE,
            from: from.as_ptr(),
            to: std::ptr::null(),
            flags: FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_SILENT | FOF_NOERRORUI,
            aborted: 0,
            mappings: std::ptr::null_mut(),
            title: std::ptr::null(),
        };
        // SAFETY: `op` and the NUL-terminated `from` list live for the call, which only reads them.
        let code = unsafe { SHFileOperationW(&mut op) };
        if code != 0 || op.aborted != 0 {
            return Err(format!("the Recycle Bin refused it (code {code})"));
        }
        Ok(())
    }
}
