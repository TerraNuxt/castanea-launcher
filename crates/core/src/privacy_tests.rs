//! The player name stays on this computer: the code that talks to the network never reads it (docs/player-name.md).

use std::fs;
use std::path::{Path, PathBuf};

fn sources(root: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(root).unwrap().flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            if name != "tests" {
                sources(&path, out);
            }
        } else if name.ends_with(".rs") && !name.contains("tests") && !name.contains("server") {
            out.push(path);
        }
    }
}

#[test]
fn the_network_code_never_reads_the_player_name() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    for part in ["download", "install", "index", "news", "selfupdate", "signature", "backend"] {
        sources(&src.join(part), &mut files);
    }
    for file in ["download.rs", "news.rs", "selfupdate.rs", "signature.rs", "backend.rs", "trust.rs"] {
        files.push(src.join(file));
    }
    assert!(files.len() > 20, "{} files scanned", files.len());
    for file in files {
        let text = fs::read_to_string(&file).unwrap();
        for word in ["player_name", "playername", "osname", "gecos", "GetUserNameEx", "with_default_name"] {
            assert!(!text.contains(word), "{} mentions {word}", file.display());
        }
    }
}

#[test]
fn the_launcher_sends_only_its_version_to_the_release_server() {
    let text = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/download.rs")).unwrap();
    assert!(
        text.contains(r#"concat!("castanea-launcher/", env!("CARGO_PKG_VERSION"))"#),
        "the user agent is the launcher and its version"
    );
    let api = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/install/release_api.rs")).unwrap();
    assert!(api.contains("platform={}&channel={}"), "a release request names the platform and the channel");
}
