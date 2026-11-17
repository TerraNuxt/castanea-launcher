//! Zip archives: checks for unsafe paths and sizes, reading one file, and unpacking a game build.

use std::fs;
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};

use crate::{Error, Result};

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub max_entries: usize,
    /// The sum of the unpacked sizes.
    pub max_bytes: u64,
}

impl Limits {
    pub const MOD: Self = Self { max_entries: 4096, max_bytes: 64 << 20 };
    pub const GAME: Self = Self { max_entries: 200_000, max_bytes: 8 << 30 };
}

/// What a checked archive holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Contents {
    /// The one folder everything sits in (`castanea-0.1.0/`), or empty when the files are at the top.
    pub prefix: String,
    pub files: Vec<String>,
    pub bytes: u64,
}

fn open(path: &Path) -> Result<zip::ZipArchive<fs::File>> {
    let file = fs::File::open(path).map_err(|error| Error::io(path, error))?;
    zip::ZipArchive::new(file).map_err(|error| Error::invalid(format!("{}: not a zip archive: {error}", path.display())))
}

/// A path inside an archive that stays inside the target: relative, no `..`, no drive or root.
pub fn safe_relative(name: &str) -> Option<PathBuf> {
    if name.contains('\\') || name.contains('\0') {
        return None;
    }
    let path = Path::new(name);
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => {
                let text = part.to_str()?;
                if text.contains(':') {
                    return None;
                }
                out.push(part);
            }
            Component::CurDir => {}
            _ => return None,
        }
    }
    (!out.as_os_str().is_empty()).then_some(out)
}

/// Lists and checks every entry; any unsafe name or a size over the limits refuses the whole archive.
pub fn check(path: &Path, limits: Limits) -> Result<Contents> {
    let mut archive = open(path)?;
    if archive.len() > limits.max_entries {
        return Err(Error::refused(format!("{}: {} entries, the limit is {}", path.display(), archive.len(), limits.max_entries)));
    }
    let mut files = Vec::new();
    let mut bytes = 0u64;
    for index in 0..archive.len() {
        let entry = archive.by_index(index).map_err(|error| Error::invalid(format!("{}: {error}", path.display())))?;
        let name = entry.name().to_owned();
        if safe_relative(&name).is_none() {
            return Err(Error::refused(format!("{}: unsafe path `{name}`", path.display())));
        }
        if entry.is_symlink() {
            return Err(Error::refused(format!("{}: symbolic link `{name}`", path.display())));
        }
        bytes = bytes.saturating_add(entry.size());
        if bytes > limits.max_bytes {
            return Err(Error::refused(format!("{}: unpacks to more than {} bytes", path.display(), limits.max_bytes)));
        }
        if !entry.is_dir() {
            files.push(name.trim_start_matches("./").to_owned());
        }
    }
    let prefix = common_prefix(&files);
    Ok(Contents { prefix, files, bytes })
}

/// `top/` when every file is below one folder `top`, else empty.
fn common_prefix(files: &[String]) -> String {
    let mut tops = files.iter().map(|name| name.split_once('/').map(|(top, _)| top));
    let Some(Some(first)) = tops.next() else { return String::new() };
    if tops.all(|top| top == Some(first)) { format!("{first}/") } else { String::new() }
}

/// One file of the archive, at most `max` bytes.
pub fn read_file(path: &Path, name: &str, max: u64) -> Result<Option<Vec<u8>>> {
    let mut archive = open(path)?;
    let Ok(entry) = archive.by_name(name) else { return Ok(None) };
    if entry.size() > max {
        return Err(Error::refused(format!("{}: `{name}` is larger than {max} bytes", path.display())));
    }
    let mut bytes = Vec::new();
    entry.take(max).read_to_end(&mut bytes).map_err(|error| Error::io(path, error))?;
    Ok(Some(bytes))
}

/// Unpacks into `dest` (which must not exist), dropping the one top folder if there is one.
/// Unpacks into a sibling temp folder first, so a failure leaves nothing behind.
pub fn extract(path: &Path, dest: &Path, limits: Limits) -> Result<()> {
    let contents = check(path, limits)?;
    if dest.exists() {
        return Err(Error::invalid(format!("{} already exists", dest.display())));
    }
    let temp = dest.with_extension("unpacking");
    let _ = fs::remove_dir_all(&temp);
    fs::create_dir_all(&temp).map_err(|error| Error::io(&temp, error))?;
    let result = extract_into(path, &temp, &contents.prefix);
    if let Err(error) = result {
        let _ = fs::remove_dir_all(&temp);
        return Err(error);
    }
    fs::rename(&temp, dest).map_err(|error| Error::io(dest, error))
}

fn extract_into(path: &Path, temp: &Path, prefix: &str) -> Result<()> {
    let mut archive = open(path)?;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|error| Error::invalid(format!("{}: {error}", path.display())))?;
        let name = entry.name().trim_start_matches("./").to_owned();
        let Some(inner) = name.strip_prefix(prefix) else { continue };
        let Some(relative) = safe_relative(inner) else { continue };
        let out = temp.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(&out).map_err(|error| Error::io(&out, error))?;
            continue;
        }
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent).map_err(|error| Error::io(parent, error))?;
        }
        let mut file = fs::File::create(&out).map_err(|error| Error::io(&out, error))?;
        io::copy(&mut entry, &mut file).map_err(|error| Error::io(&out, error))?;
        #[cfg(unix)]
        if let Some(mode) = entry.unix_mode() {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&out, fs::Permissions::from_mode(mode & 0o755));
        }
    }
    Ok(())
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::io::Write;

    /// Writes a zip with the given (name, bytes) entries; mode 0o755 for names ending in `castanea`.
    pub fn write_zip(path: &Path, entries: &[(&str, &[u8])]) {
        let mut writer = zip::ZipWriter::new(fs::File::create(path).unwrap());
        for (name, bytes) in entries {
            let mode = if name.ends_with("castanea") { 0o755 } else { 0o644 };
            let options = zip::write::SimpleFileOptions::default().unix_permissions(mode);
            writer.start_file(*name, options).unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap();
    }

    #[test]
    fn unsafe_names_are_refused() {
        for bad in ["../x", "/etc/passwd", "a/../../b", "C:/x", "a\\b", ""] {
            assert!(safe_relative(bad).is_none(), "{bad}");
        }
        assert_eq!(safe_relative("./a/b.json").unwrap(), Path::new("a/b.json"));
    }

    #[test]
    fn a_zip_slip_archive_is_refused_whole() {
        let dir = tempfile::tempdir().unwrap();
        let zip = dir.path().join("evil.zip");
        write_zip(&zip, &[("mod.json", b"{}"), ("../../evil.txt", b"x")]);
        let error = check(&zip, Limits::MOD).unwrap_err().to_string();
        assert!(error.contains("unsafe path"), "{error}");
        assert!(extract(&zip, &dir.path().join("out"), Limits::MOD).is_err());
        assert!(!dir.path().join("out").exists());
    }

    #[test]
    fn the_top_folder_is_dropped_and_modes_kept() {
        let dir = tempfile::tempdir().unwrap();
        let zip = dir.path().join("game.zip");
        write_zip(&zip, &[("castanea-0.1.0/castanea", b"bin"), ("castanea-0.1.0/assets/a.json", b"{}")]);
        let contents = check(&zip, Limits::GAME).unwrap();
        assert_eq!(contents.prefix, "castanea-0.1.0/");
        let out = dir.path().join("out");
        extract(&zip, &out, Limits::GAME).unwrap();
        assert_eq!(fs::read(out.join("castanea")).unwrap(), b"bin");
        assert!(out.join("assets/a.json").is_file());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(fs::metadata(out.join("castanea")).unwrap().permissions().mode() & 0o111, 0o111);
        }
    }

    #[test]
    fn size_and_entry_limits_hold() {
        let dir = tempfile::tempdir().unwrap();
        let zip = dir.path().join("big.zip");
        let big = vec![0u8; 3000];
        write_zip(&zip, &[("a", &big), ("b", &big)]);
        assert!(check(&zip, Limits { max_entries: 10, max_bytes: 5000 }).is_err());
        assert!(check(&zip, Limits { max_entries: 1, max_bytes: 1 << 20 }).is_err());
        assert!(check(&zip, Limits { max_entries: 10, max_bytes: 1 << 20 }).is_ok());
        assert_eq!(read_file(&zip, "a", 5000).unwrap().unwrap().len(), 3000);
        assert!(read_file(&zip, "a", 100).is_err());
        assert!(read_file(&zip, "zzz", 100).unwrap().is_none());
    }
}
