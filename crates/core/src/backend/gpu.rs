//! A cheap fingerprint of the machine's GPU and driver, to key the probe cache. It is a hash of what the system
//! lists without starting any graphics API; an empty key means unknown (the cache then keeps results for a day).

#[cfg(not(windows))]
use std::path::Path;

use sha2::{Digest, Sha256};

/// The key for this machine, or an empty string when nothing could be read.
pub fn system_key() -> String {
    #[cfg(windows)]
    let raw = windows_raw();
    #[cfg(not(windows))]
    let raw =
        linux_raw(Path::new("/sys"), Path::new("/proc"), Path::new("/usr/share/vulkan"), Path::new("/etc/vulkan"));
    key_of(&raw)
}

pub fn key_of(raw: &str) -> String {
    if raw.trim().is_empty() {
        return String::new();
    }
    let digest = Sha256::digest(raw.as_bytes());
    digest.iter().take(8).map(|b| format!("{b:02x}")).collect()
}

/// PCI ids and kernel drivers of the display cards, the NVIDIA kernel module's version, and the age of the Vulkan
/// driver manifests (a driver install touches them).
#[cfg(not(windows))]
pub fn linux_raw(sys: &Path, proc: &Path, share_vulkan: &Path, etc_vulkan: &Path) -> String {
    let mut lines = Vec::new();
    if let Ok(entries) = std::fs::read_dir(sys.join("class/drm")) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let is_card =
                name.strip_prefix("card").is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()));
            if !is_card {
                continue;
            }
            let device = entry.path().join("device");
            let read = |file: &str| {
                std::fs::read_to_string(device.join(file)).map(|t| t.trim().to_owned()).unwrap_or_default()
            };
            let driver = std::fs::read_link(device.join("driver"))
                .ok()
                .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
                .unwrap_or_default();
            lines.push(format!("{name} {} {} {driver}", read("vendor"), read("device")));
        }
    }
    lines.sort();
    if let Ok(text) = std::fs::read_to_string(proc.join("driver/nvidia/version")) {
        lines.push(text.lines().next().unwrap_or_default().trim().to_owned());
    }
    if !lines.is_empty() {
        for dir in [share_vulkan.join("icd.d"), etc_vulkan.join("icd.d")] {
            let seconds = std::fs::metadata(&dir)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_secs());
            lines.push(format!("icd {seconds}"));
        }
    }
    lines.join("\n")
}

/// The values named `name` in `reg query /s /v <name>` output, in order.
pub fn reg_values(text: &str, name: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| {
            let rest = line.trim().strip_prefix(name)?;
            let (kind, value) = rest.trim_start().split_once(char::is_whitespace)?;
            (rest.starts_with(char::is_whitespace) && kind.starts_with("REG_")).then(|| value.trim().to_owned())
        })
        .collect()
}

/// Display adapters and driver versions from the registry, as sorted `description version` lines.
pub fn windows_lines(descriptions: &str, versions: &str) -> String {
    let names = reg_values(descriptions, "DriverDesc");
    let versions = reg_values(versions, "DriverVersion");
    let mut lines: Vec<String> = names
        .iter()
        .enumerate()
        .map(|(n, name)| format!("{name} {}", versions.get(n).map_or("", String::as_str)))
        .collect();
    lines.sort();
    lines.join("\n")
}

#[cfg(windows)]
fn windows_raw() -> String {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const CLASS: &str = r"HKLM\SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}";
    let query = |value: &str| {
        std::process::Command::new("reg")
            .args(["query", CLASS, "/s", "/v", value])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
            .unwrap_or_default()
    };
    windows_lines(&query("DriverDesc"), &query("DriverVersion"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_key_is_stable_short_and_empty_for_nothing() {
        assert_eq!(key_of("a"), key_of("a"));
        assert_ne!(key_of("a"), key_of("b"));
        assert_eq!(key_of("a").len(), 16);
        assert_eq!(key_of("  \n"), "");
    }

    #[test]
    fn registry_output_gives_adapter_and_driver_lines() {
        let names = "\nHKEY_LOCAL_MACHINE\\X\\0000\n    DriverDesc    REG_SZ    NVIDIA GeForce RTX 4070\n\nHKEY_LOCAL_MACHINE\\X\\0001\n    DriverDesc    REG_SZ    Intel(R) UHD Graphics\n";
        let versions = "HKEY_LOCAL_MACHINE\\X\\0000\n    DriverVersion    REG_SZ    32.0.15.6094\nHKEY_LOCAL_MACHINE\\X\\0001\n    DriverVersion    REG_SZ    31.0.101.2111\n";
        assert_eq!(
            windows_lines(names, versions),
            "Intel(R) UHD Graphics 31.0.101.2111\nNVIDIA GeForce RTX 4070 32.0.15.6094"
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn linux_cards_and_nvidia_version_are_read_and_connectors_are_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let card = dir.path().join("sys/class/drm/card0/device");
        std::fs::create_dir_all(&card).unwrap();
        std::fs::write(card.join("vendor"), "0x10de\n").unwrap();
        std::fs::write(card.join("device"), "0x2786\n").unwrap();
        std::fs::create_dir_all(dir.path().join("sys/class/drm/card0-DP-1/device")).unwrap();
        std::fs::create_dir_all(dir.path().join("proc/driver/nvidia")).unwrap();
        std::fs::write(dir.path().join("proc/driver/nvidia/version"), "NVRM version: 550.54\nGCC\n").unwrap();
        let raw = |d: &Path| linux_raw(&d.join("sys"), &d.join("proc"), &d.join("a"), &d.join("b"));
        let text = raw(dir.path());
        assert!(text.contains("card0 0x10de 0x2786") && text.contains("NVRM version: 550.54"), "{text}");
        assert!(!text.contains("DP-1"));
        assert_eq!(raw(&dir.path().join("nothing")), "", "no cards, no key");
    }
}
