//! Where the launcher is installed, and whether it may replace itself there.

use std::fs;
use std::path::Path;

/// Why the launcher leaves its own update to something else.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Why {
    /// An AppImage is a read-only image; it is replaced as a whole.
    AppImage,
    /// A package manager, a Nix store, a Flatpak or snap, or a Windows system folder owns the file.
    System,
    /// The folder cannot be written by this user.
    ReadOnly,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Install {
    /// The launcher may replace its own file.
    SelfUpdating,
    /// It may not: show the new version and point to the download.
    Managed(Why),
}

/// Folders that belong to a package manager or a platform (lower case, `/` separators).
const SYSTEM_PREFIXES: [&str; 6] = ["/usr/", "/nix/store/", "/snap/", "/app/", "/var/lib/flatpak/", "/program files"];

pub fn classify(exe: &Path, appimage: bool, writable: &dyn Fn(&Path) -> bool) -> Install {
    let text = exe.to_string_lossy().replace('\\', "/").to_ascii_lowercase();
    if appimage || text.starts_with("/tmp/.mount_") {
        return Install::Managed(Why::AppImage);
    }
    // On Windows the drive letter comes first: `c:/program files/…`.
    let tail = if text.as_bytes().get(1) == Some(&b':') { &text[2..] } else { text.as_str() };
    if SYSTEM_PREFIXES.iter().any(|prefix| tail.starts_with(prefix)) || tail.contains("/windowsapps/") {
        return Install::Managed(Why::System);
    }
    let dir = exe.parent().unwrap_or(Path::new("."));
    if writable(dir) { Install::SelfUpdating } else { Install::Managed(Why::ReadOnly) }
}

/// For the running launcher: `APPIMAGE` in the environment and a probe file in its folder.
pub fn detect(exe: &Path) -> Install {
    classify(exe, std::env::var_os("APPIMAGE").is_some_and(|v| !v.is_empty()), &dir_writable)
}

/// Whether a file can be created in `dir` (a read-only mount or a system folder says no).
pub fn dir_writable(dir: &Path) -> bool {
    let probe = dir.join(format!(".castanea-write-test-{}", std::process::id()));
    let ok = fs::File::create(&probe).is_ok();
    let _ = fs::remove_file(&probe);
    ok
}
