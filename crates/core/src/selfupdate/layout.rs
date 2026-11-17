//! The files next to the running launcher that an update touches.

use std::path::{Path, PathBuf};

/// The running executable and the names derived from it: `castanea-launcher.old.exe` is the build before an update.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layout {
    pub exe: PathBuf,
}

impl Layout {
    pub fn new(exe: impl Into<PathBuf>) -> Self {
        Self { exe: exe.into() }
    }

    /// The running executable. Call it at the start: on Linux a replaced file reads back as `… (deleted)` later.
    pub fn detect() -> Option<Self> {
        std::env::current_exe().ok().map(Self::new)
    }

    pub fn dir(&self) -> &Path {
        self.exe.parent().unwrap_or(Path::new("."))
    }

    /// The previous launcher, kept until the new one has shown its first frame.
    pub fn old(&self) -> PathBuf {
        self.sibling("old")
    }

    /// A launcher that failed to start, moved aside by a rollback.
    pub fn bad(&self) -> PathBuf {
        self.sibling("bad")
    }

    /// The new launcher copied next to the old one, so the final rename stays on one filesystem.
    pub fn incoming(&self) -> PathBuf {
        self.sibling("new")
    }

    /// Everything an update can leave behind.
    pub fn leftovers(&self) -> [PathBuf; 3] {
        [self.old(), self.bad(), self.incoming()]
    }

    fn sibling(&self, tag: &str) -> PathBuf {
        let stem =
            self.exe.file_stem().map_or_else(|| "castanea-launcher".into(), |s| s.to_string_lossy().into_owned());
        let name = match self.exe.extension() {
            Some(ext) => format!("{stem}.{tag}.{}", ext.to_string_lossy()),
            None => format!("{stem}.{tag}"),
        };
        self.dir().join(name)
    }
}
