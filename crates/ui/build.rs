//! Windows only: puts the icon and the version info into the exe. The resource script is written here and compiled by
//! `windres` (mingw, the release build) or `rc` (MSVC, when it is on the path); with neither, the exe is built
//! without them and a warning says so. No other target, and no dependency, is involved.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{env, fs};

const ICON: &str = "../../packaging/windows/castanea-launcher.ico";

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={ICON}");
    println!("cargo:rerun-if-env-changed=WINDRES");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let icon = manifest.join(ICON).canonicalize().expect("the Windows icon is missing");
    let script = out.join("launcher.rc");
    fs::write(&script, resource_script(&icon, &env::var("CARGO_PKG_VERSION").unwrap_or_default())).expect("write .rc");

    let compiled = if env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        let res = out.join("launcher.res");
        run("rc", &[OsStr::new("/nologo"), OsStr::new("/fo"), res.as_os_str(), script.as_os_str()]).then_some(res)
    } else {
        let object = out.join("launcher_res.o");
        let windres = env::var("WINDRES").unwrap_or_else(|_| "x86_64-w64-mingw32-windres".to_owned());
        let args = [
            OsStr::new("-O"),
            OsStr::new("coff"),
            OsStr::new("-i"),
            script.as_os_str(),
            OsStr::new("-o"),
            object.as_os_str(),
        ];
        (run(&windres, &args) || run("windres", &args)).then_some(object)
    };
    match compiled {
        Some(file) => println!("cargo:rustc-link-arg-bins={}", file.display()),
        None => println!("cargo:warning=no windres or rc found: the exe has no icon and no version info"),
    }
}

fn run(program: &str, args: &[&OsStr]) -> bool {
    Command::new(program).args(args).status().is_ok_and(|status| status.success())
}

/// The icon (resource 1, which Explorer shows) and a VERSIONINFO built from the package version.
fn resource_script(icon: &Path, version: &str) -> String {
    let numbers: Vec<u32> =
        version.split(['-', '+']).next().unwrap_or("").split('.').map(|n| n.parse().unwrap_or(0)).collect();
    let part = |n: usize| numbers.get(n).copied().unwrap_or(0);
    let file_version = format!("{},{},{},0", part(0), part(1), part(2));
    let icon = icon.display().to_string().replace('\\', "/");
    format!(
        "1 ICON \"{icon}\"\n\
         1 VERSIONINFO\n FILEVERSION {file_version}\n PRODUCTVERSION {file_version}\n FILEOS 0x40004\n FILETYPE 1\n{{\n \
         BLOCK \"StringFileInfo\"\n {{\n  BLOCK \"040904B0\"\n  {{\n   \
         VALUE \"CompanyName\", \"castanea\"\n   \
         VALUE \"FileDescription\", \"castanea launcher\"\n   \
         VALUE \"FileVersion\", \"{version}\"\n   \
         VALUE \"InternalName\", \"castanea-launcher\"\n   \
         VALUE \"LegalCopyright\", \"MIT licence\"\n   \
         VALUE \"OriginalFilename\", \"castanea-launcher.exe\"\n   \
         VALUE \"ProductName\", \"castanea launcher\"\n   \
         VALUE \"ProductVersion\", \"{version}\"\n  }}\n }}\n \
         BLOCK \"VarFileInfo\"\n {{\n  VALUE \"Translation\", 0x409, 1200\n }}\n}}\n"
    )
}
