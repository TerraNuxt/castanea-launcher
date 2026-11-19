//! The OS account's display name, the default player name until the user types one. It is read locally and goes
//! nowhere but to the game as `--name` (docs/player-name.md).

/// A passwd line `login:x:uid:gid:gecos:home:shell` as (login, gecos).
pub fn passwd_fields(line: &str) -> Option<(&str, &str)> {
    let mut fields = line.split(':');
    let login = fields.next().filter(|l| !l.is_empty())?;
    let gecos = fields.nth(3)?;
    Some((login, gecos))
}

/// The full name in a GECOS field: its first comma part, with `&` standing for the login name capitalised.
pub fn gecos_name(gecos: &str, login: &str) -> Option<String> {
    let first = gecos.split(',').next().unwrap_or("").trim();
    if first.is_empty() {
        return None;
    }
    let mut capital = String::new();
    let mut letters = login.chars();
    if let Some(c) = letters.next() {
        capital.extend(c.to_uppercase());
        capital.push_str(letters.as_str());
    }
    let name = first.replace('&', &capital);
    let name = name.trim();
    (!name.is_empty()).then(|| name.to_owned())
}

/// The display name for a passwd entry: the full name, else the login name.
pub fn from_passwd(login: &str, gecos: &str) -> Option<String> {
    gecos_name(gecos, login).or_else(|| Some(login.trim().to_owned()).filter(|l| !l.is_empty()))
}

/// The text of a UTF-16 buffer the system filled in: up to the first NUL, or `len` units when it gives one.
pub fn from_wide(buffer: &[u16], len: Option<usize>) -> Option<String> {
    let units = match len {
        Some(len) => &buffer[..len.min(buffer.len())],
        None => buffer,
    };
    let end = units.iter().position(|&u| u == 0).unwrap_or(units.len());
    let text = String::from_utf16_lossy(&units[..end]);
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_owned())
}

/// The login name from the environment, the last resort on every platform.
fn login_from_env() -> Option<String> {
    ["USER", "LOGNAME", "USERNAME"]
        .iter()
        .filter_map(|name| std::env::var(name).ok())
        .map(|value| value.trim().to_owned())
        .find(|value| !value.is_empty())
}

/// The current account's display name: Windows the full name of the account (`GetUserNameExW`, `NameDisplay`),
/// Linux and macOS the GECOS full name (`getpwuid_r`); the login name when there is none.
pub fn detect() -> Option<String> {
    #[cfg(windows)]
    let found = windows::display_name();
    #[cfg(unix)]
    let found = unix::display_name();
    #[cfg(not(any(windows, unix)))]
    let found = None;
    found.or_else(login_from_env)
}

#[cfg(unix)]
mod unix {
    use std::ffi::CStr;

    /// The passwd entry of the real user through the system's name service (files, LDAP, homed).
    pub fn display_name() -> Option<String> {
        // SAFETY: getpwuid_r fills `passwd` and `buffer`, which both outlive the strings read from them below.
        unsafe {
            let uid = libc::getuid();
            let mut size = 1024;
            loop {
                let mut passwd: libc::passwd = std::mem::zeroed();
                let mut result: *mut libc::passwd = std::ptr::null_mut();
                let mut buffer = vec![0 as libc::c_char; size];
                let code = libc::getpwuid_r(uid, &mut passwd, buffer.as_mut_ptr(), buffer.len(), &mut result);
                if code == libc::ERANGE && size < 1 << 16 {
                    size *= 4;
                    continue;
                }
                if code != 0 || result.is_null() || passwd.pw_name.is_null() {
                    return None;
                }
                let login = CStr::from_ptr(passwd.pw_name).to_string_lossy().into_owned();
                let gecos = if passwd.pw_gecos.is_null() {
                    String::new()
                } else {
                    CStr::from_ptr(passwd.pw_gecos).to_string_lossy().into_owned()
                };
                return super::from_passwd(&login, &gecos);
            }
        }
    }
}

#[cfg(windows)]
mod windows {
    /// `EXTENDED_NAME_FORMAT::NameDisplay`.
    const NAME_DISPLAY: i32 = 3;
    const ERROR_MORE_DATA: i32 = 234;

    #[link(name = "secur32")]
    unsafe extern "system" {
        fn GetUserNameExW(format: i32, name: *mut u16, size: *mut u32) -> u8;
    }

    /// The account's display name, or none when Windows has none for it (a local account without a full name).
    pub fn display_name() -> Option<String> {
        let mut capacity: u32 = 256;
        for _ in 0..2 {
            let mut buffer = vec![0u16; capacity as usize];
            let mut size = capacity;
            // SAFETY: `buffer` holds `size` UTF-16 units, which the call fills and reports back in `size`.
            let ok = unsafe { GetUserNameExW(NAME_DISPLAY, buffer.as_mut_ptr(), &mut size) };
            if ok != 0 {
                return super::from_wide(&buffer, Some(size as usize));
            }
            if std::io::Error::last_os_error().raw_os_error() != Some(ERROR_MORE_DATA) || size <= capacity {
                return None;
            }
            capacity = size;
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_gecos_field_gives_its_first_part() {
        assert_eq!(gecos_name("Anna Schmidt,Room 4,555-1234,,", "anna").as_deref(), Some("Anna Schmidt"));
        assert_eq!(gecos_name("Anna Schmidt", "anna").as_deref(), Some("Anna Schmidt"));
        assert_eq!(gecos_name(" Anna  ,", "anna").as_deref(), Some("Anna"));
    }

    #[test]
    fn an_empty_gecos_gives_nothing_and_an_ampersand_is_the_login() {
        assert_eq!(gecos_name("", "anna"), None);
        assert_eq!(gecos_name(",,,", "anna"), None);
        assert_eq!(gecos_name("   ", "anna"), None);
        assert_eq!(gecos_name("&", "anna").as_deref(), Some("Anna"));
        assert_eq!(gecos_name("& Schmidt,x", "anna").as_deref(), Some("Anna Schmidt"));
    }

    #[test]
    fn unicode_names_pass_through() {
        assert_eq!(gecos_name("Jürgen Müller,,,", "jm").as_deref(), Some("Jürgen Müller"));
        assert_eq!(gecos_name("名前 太郎", "x").as_deref(), Some("名前 太郎"));
        assert_eq!(gecos_name("&", "émile").as_deref(), Some("Émile"));
    }

    #[test]
    fn a_passwd_line_is_split_into_login_and_gecos() {
        let line = "anna:x:1000:1000:Anna Schmidt,,,:/home/anna:/bin/bash";
        assert_eq!(passwd_fields(line), Some(("anna", "Anna Schmidt,,,")));
        assert_eq!(passwd_fields("root:x:0:0:root:/root:/bin/bash"), Some(("root", "root")));
        assert_eq!(passwd_fields("nobody:x:65534:65534::/nonexistent:/usr/sbin/nologin"), Some(("nobody", "")));
        assert_eq!(passwd_fields(":x:1:1:Nobody::"), None);
        assert_eq!(passwd_fields("broken"), None);
    }

    #[test]
    fn the_login_is_the_fallback_for_an_entry_without_a_full_name() {
        assert_eq!(from_passwd("anna", "Anna Schmidt,,,").as_deref(), Some("Anna Schmidt"));
        assert_eq!(from_passwd("anna", "").as_deref(), Some("anna"));
        assert_eq!(from_passwd("anna", ",,,").as_deref(), Some("anna"));
        assert_eq!(from_passwd("", ""), None);
    }

    #[test]
    fn a_wide_buffer_is_read_up_to_its_length_or_nul() {
        let wide: Vec<u16> = "Jürgen Müller".encode_utf16().chain([0, 0, 0]).collect();
        assert_eq!(from_wide(&wide, Some(13)).as_deref(), Some("Jürgen Müller"));
        assert_eq!(from_wide(&wide, None).as_deref(), Some("Jürgen Müller"));
        assert_eq!(from_wide(&[0u16; 8], Some(8)), None);
        assert_eq!(from_wide(&[], None), None);
        let spaced: Vec<u16> = "  Eva  ".encode_utf16().collect();
        assert_eq!(from_wide(&spaced, Some(99)).as_deref(), Some("Eva"), "a length past the buffer is cut to it");
    }

    #[cfg(unix)]
    #[test]
    fn this_machine_has_some_name() {
        assert!(detect().is_some_and(|name| !name.trim().is_empty()));
    }
}
