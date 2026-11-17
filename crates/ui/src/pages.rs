//! The pages behind the sidebar items, and what several of them share.

pub mod about;
pub mod browse;
pub mod home;
pub mod home_cards;
pub mod log;
pub mod mods;
pub mod notifications;
pub mod profiles;
pub mod settings;
pub mod versions;

use castanea_launcher_core::launch;
use castanea_launcher_core::profile::{Profile, StartIn};

use crate::app::{App, now_seconds};
use crate::i18n::Strings;

/// "castanea 0.2.0 · world lake · 2 h ago" for a profile card.
pub fn profile_line(app: &App, profile: &Profile, with_time: bool) -> String {
    let build = match launch::pick_build(&app.registry, profile) {
        Ok(build) => format!("castanea {}", build.version),
        Err(_) => app.tr.get("home.no_build").to_owned(),
    };
    let start = match &profile.start {
        StartIn::Menu => app.tr.get("home.start_menu").to_owned(),
        StartIn::World { name, .. } if name.is_empty() => app.tr.get("home.start_new_world").to_owned(),
        StartIn::World { name, .. } => app.tr.fill("home.start_world", &[("name", name)]),
        StartIn::Join { address } => app.tr.fill("home.start_join", &[("address", address)]),
    };
    let mut parts = vec![build, start];
    if with_time {
        parts.push(ago(&app.tr, profile.last_played));
    }
    parts.join("  ·  ")
}

/// "just now", "5 min ago", "2 h ago", "3 days ago" or "never played".
pub fn ago(tr: &Strings, when: Option<u64>) -> String {
    let Some(when) = when else { return tr.get("home.never_played").to_owned() };
    let seconds = now_seconds().saturating_sub(when);
    let (key, n) = match seconds {
        0..60 => return tr.get("home.played_now").to_owned(),
        60..3600 => ("home.played_minutes", seconds / 60),
        3600..86400 => ("home.played_hours", seconds / 3600),
        _ => ("home.played_days", seconds / 86400),
    };
    tr.fill(key, &[("n", &n.to_string())])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ago_picks_the_unit() {
        let tr = Strings::load("en");
        let now = now_seconds();
        assert_eq!(ago(&tr, None), tr.get("home.never_played"));
        assert_eq!(ago(&tr, Some(now)), tr.get("home.played_now"));
        assert!(ago(&tr, Some(now - 300)).contains('5'));
        assert!(ago(&tr, Some(now - 7200)).contains('2'));
        assert!(ago(&tr, Some(now - 3 * 86400)).contains('3'));
    }
}
