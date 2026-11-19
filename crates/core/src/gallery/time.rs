//! Calendar days from Unix time and the local time zone, without a date library.

/// A calendar date.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Day {
    pub year: i32,
    pub month: u8,
    pub day: u8,
}

impl Day {
    /// `2026-11-19`.
    pub fn iso(self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }

    /// Days since 1970-01-01.
    pub fn days(self) -> i64 {
        days_from_civil(self.year, self.month, self.day)
    }
}

/// Days since 1970-01-01 of a proleptic Gregorian date (Howard Hinnant's algorithm).
pub fn days_from_civil(year: i32, month: u8, day: u8) -> i64 {
    let y = i64::from(year) - i64::from(month <= 2);
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let m = i64::from(month);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(day) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The date of a day count since 1970-01-01.
pub fn civil_from_days(days: i64) -> Day {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u8;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u8;
    let year = (yoe + era * 400 + i64::from(month <= 2)) as i32;
    Day { year, month, day }
}

/// The date of `seconds` since 1970 (already shifted to local time).
pub fn day_of(seconds: i64) -> Day {
    civil_from_days(seconds.div_euclid(86_400))
}

/// The local time zone's offset from UTC, in seconds east, at the instant `unix_seconds` (daylight saving included).
pub fn local_offset(unix_seconds: i64) -> i64 {
    #[cfg(unix)]
    {
        unix_offset(unix_seconds)
    }
    #[cfg(windows)]
    {
        windows_offset(unix_seconds)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = unix_seconds;
        0
    }
}

#[cfg(unix)]
#[allow(clippy::useless_conversion)] // `tm_gmtoff` is a `c_long`: `i64` on 64-bit systems, narrower on others
fn unix_offset(unix_seconds: i64) -> i64 {
    let t = libc::time_t::try_from(unix_seconds).unwrap_or(0);
    // SAFETY: localtime_r writes only into `tm`, which is zeroed and lives for the call.
    unsafe {
        let mut tm: libc::tm = std::mem::zeroed();
        if libc::localtime_r(&t, &mut tm).is_null() { 0 } else { i64::from(tm.tm_gmtoff) }
    }
}

#[cfg(windows)]
#[repr(C)]
#[derive(Default)]
struct SystemTime {
    year: u16,
    month: u16,
    day_of_week: u16,
    day: u16,
    hour: u16,
    minute: u16,
    second: u16,
    milliseconds: u16,
}

#[cfg(windows)]
unsafe extern "system" {
    fn SystemTimeToTzSpecificLocalTime(zone: *const u8, utc: *const SystemTime, local: *mut SystemTime) -> i32;
}

#[cfg(windows)]
fn windows_offset(unix_seconds: i64) -> i64 {
    let day = day_of(unix_seconds);
    let of_day = unix_seconds.rem_euclid(86_400);
    let utc = SystemTime {
        year: day.year.clamp(1601, 30_000) as u16,
        month: u16::from(day.month),
        day: u16::from(day.day),
        hour: (of_day / 3600) as u16,
        minute: (of_day / 60 % 60) as u16,
        second: (of_day % 60) as u16,
        ..SystemTime::default()
    };
    let mut local = SystemTime::default();
    // SAFETY: both pointers refer to initialised structs of the layout the call expects; a null zone means the current one.
    if unsafe { SystemTimeToTzSpecificLocalTime(std::ptr::null(), &utc, &mut local) } == 0 {
        return 0;
    }
    let shown = days_from_civil(i32::from(local.year), local.month as u8, local.day as u8) * 86_400
        + i64::from(local.hour) * 3600
        + i64::from(local.minute) * 60
        + i64::from(local.second);
    shown - unix_seconds
}
