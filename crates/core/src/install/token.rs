//! The GitHub token for a private game repo: from the environment or a private file, never from launcher.json.

use std::fs;

use crate::paths::Paths;
use crate::{Result, json_file};

pub const ENV: [&str; 2] = ["CASTANEA_GITHUB_TOKEN", "GITHUB_TOKEN"];

/// The token, if any: the environment first, then `config/github-token`.
pub fn read(paths: &Paths) -> Option<String> {
    for name in ENV {
        if let Ok(value) = std::env::var(name)
            && !value.trim().is_empty()
        {
            return Some(value.trim().to_owned());
        }
    }
    let text = fs::read_to_string(paths.token_file()).ok()?;
    let token = text.trim();
    (!token.is_empty()).then(|| token.to_owned())
}

/// Saves the token to `config/github-token` (mode 0600 on Unix); an empty token deletes the file.
pub fn save(paths: &Paths, token: &str) -> Result<()> {
    let file = paths.token_file();
    if token.trim().is_empty() {
        let _ = fs::remove_file(&file);
        return Ok(());
    }
    json_file::write_atomic(&file, format!("{}\n", token.trim()).as_bytes())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&file, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

/// Where the token comes from, for the Settings tab (never the token itself).
pub fn origin(paths: &Paths) -> Option<String> {
    for name in ENV {
        if std::env::var(name).is_ok_and(|v| !v.trim().is_empty()) {
            return Some(format!("${name}"));
        }
    }
    paths.token_file().is_file().then(|| paths.token_file().display().to_string())
}
