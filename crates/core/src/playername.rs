//! The player name: what the user typed, the OS account's name as the default, and the form the game accepts.
//!
//! The game takes 3 to 16 ASCII letters, digits or `_` (`--name`, castanea `network/link/ident.rs`). The launcher keeps
//! and shows the name as typed (tidied), and passes the game its closest allowed form.

/// The game's shortest and longest player name.
pub const GAME_MIN: usize = 3;
pub const GAME_MAX: usize = 16;
/// The longest name the launcher keeps and shows, in characters.
pub const SHOWN_MAX: usize = 32;

/// Where the name in use comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// Typed in Settings; it wins until it is cleared.
    User,
    /// The OS account's name, used while nothing is typed.
    System,
}

/// The name in use and where it comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Name {
    pub shown: String,
    pub source: Source,
}

/// Invisible formatting characters that would make two names look alike or hide text.
fn invisible(c: char) -> bool {
    matches!(c, '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2060}'..='\u{2064}' | '\u{2066}'..='\u{2069}' | '\u{FEFF}')
}

/// Trims, collapses runs of white space to one space, drops control characters, keeps at most `SHOWN_MAX` characters.
pub fn tidy(text: &str) -> String {
    let mut out = String::new();
    let mut gap = false;
    for c in text.chars() {
        if c.is_whitespace() {
            gap = true;
        } else if !c.is_control() && !invisible(c) {
            if gap && !out.is_empty() {
                out.push(' ');
            }
            gap = false;
            out.push(c);
        }
    }
    let kept: String = out.chars().take(SHOWN_MAX).collect();
    kept.trim_end().to_owned()
}

/// The name in use: the user's own when it is not empty, else the OS account's, else none.
pub fn resolve(user: &str, system: Option<&str>) -> Option<Name> {
    let user = tidy(user);
    if !user.is_empty() {
        return Some(Name { shown: user, source: Source::User });
    }
    let system = tidy(system?);
    (!system.is_empty()).then_some(Name { shown: system, source: Source::System })
}

/// The ASCII spelling of one character: letters with accents and the German umlauts become letters, the rest `_`.
fn ascii_of(c: char) -> &'static str {
    match c {
        'ä' => "ae",
        'ö' => "oe",
        'ü' => "ue",
        'Ä' => "Ae",
        'Ö' => "Oe",
        'Ü' => "Ue",
        'ß' | 'ẞ' => "ss",
        'æ' => "ae",
        'Æ' => "Ae",
        'œ' => "oe",
        'Œ' => "Oe",
        'à' | 'á' | 'â' | 'ã' | 'å' | 'ā' | 'ă' | 'ą' => "a",
        'À' | 'Á' | 'Â' | 'Ã' | 'Å' | 'Ā' | 'Ă' | 'Ą' => "A",
        'ç' | 'ć' | 'č' => "c",
        'Ç' | 'Ć' | 'Č' => "C",
        'ď' | 'đ' => "d",
        'Ď' | 'Đ' => "D",
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ę' | 'ě' => "e",
        'È' | 'É' | 'Ê' | 'Ë' | 'Ē' | 'Ę' | 'Ě' => "E",
        'ì' | 'í' | 'î' | 'ï' | 'ī' => "i",
        'Ì' | 'Í' | 'Î' | 'Ï' | 'Ī' => "I",
        'ł' | 'ĺ' | 'ľ' => "l",
        'Ł' | 'Ĺ' | 'Ľ' => "L",
        'ñ' | 'ń' | 'ň' => "n",
        'Ñ' | 'Ń' | 'Ň' => "N",
        'ò' | 'ó' | 'ô' | 'õ' | 'ø' | 'ō' | 'ő' => "o",
        'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ø' | 'Ō' | 'Ő' => "O",
        'ř' | 'ŕ' => "r",
        'Ř' | 'Ŕ' => "R",
        'š' | 'ś' | 'ş' => "s",
        'Š' | 'Ś' | 'Ş' => "S",
        'ť' | 'ţ' => "t",
        'Ť' | 'Ţ' => "T",
        'ù' | 'ú' | 'û' | 'ū' | 'ů' | 'ű' => "u",
        'Ù' | 'Ú' | 'Û' | 'Ū' | 'Ů' | 'Ű' => "U",
        'ý' | 'ÿ' => "y",
        'Ý' | 'Ÿ' => "Y",
        'ž' | 'ź' | 'ż' => "z",
        'Ž' | 'Ź' | 'Ż' => "Z",
        _ => "_",
    }
}

/// The closest name the game accepts, or none when nothing of the text is usable (the game then keeps its own name).
/// Spaces and other characters become one `_`, umlauts and accents become letters, the result has 3 to 16 characters.
pub fn for_game(text: &str) -> Option<String> {
    let mut out = String::new();
    for c in tidy(text).chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if c == '_' {
            out.push('_');
        } else {
            out.push_str(ascii_of(c));
        }
    }
    let mut collapsed = String::new();
    for c in out.chars() {
        if c != '_' || !collapsed.ends_with('_') {
            collapsed.push(c);
        }
    }
    let kept: String = collapsed.trim_matches('_').chars().take(GAME_MAX).collect();
    let mut name = kept.trim_end_matches('_').to_owned();
    if name.is_empty() {
        return None;
    }
    while name.len() < GAME_MIN {
        name.push('_');
    }
    Some(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tidy_trims_collapses_and_drops_control_characters() {
        assert_eq!(tidy("  Anna   Lena \t\n"), "Anna Lena");
        assert_eq!(tidy("An\u{0}na\u{7}"), "Anna");
        assert_eq!(tidy("An\u{200B}na\u{202E}"), "Anna");
        assert_eq!(tidy("\u{0}\u{1}"), "");
        assert_eq!(tidy("   "), "");
        assert_eq!(tidy("Jürgen Müller"), "Jürgen Müller", "unicode stays as typed");
    }

    #[test]
    fn tidy_shortens_by_characters_not_bytes() {
        let long = "ü".repeat(SHOWN_MAX + 10);
        assert_eq!(tidy(&long).chars().count(), SHOWN_MAX);
        let spaced = format!("{} {}", "a".repeat(SHOWN_MAX - 1), "b");
        assert_eq!(tidy(&spaced), "a".repeat(SHOWN_MAX - 1), "a name cut at a space does not keep it");
    }

    #[test]
    fn the_user_name_beats_the_system_name_beats_nothing() {
        let user = resolve("Anna", Some("Jürgen Müller")).unwrap();
        assert_eq!(user, Name { shown: "Anna".into(), source: Source::User });
        let system = resolve("", Some("Jürgen Müller")).unwrap();
        assert_eq!(system, Name { shown: "Jürgen Müller".into(), source: Source::System });
        assert_eq!(resolve("   ", Some(" Eva ")).unwrap().shown, "Eva", "a blank user name is no name");
        assert_eq!(resolve("", None), None);
        assert_eq!(resolve("", Some("  ")), None);
        assert_eq!(resolve("Anna", None).unwrap().source, Source::User, "the user name needs no system name");
    }

    #[test]
    fn clearing_the_user_name_returns_to_the_system_name() {
        let mut typed = "Anna".to_owned();
        assert_eq!(resolve(&typed, Some("Eva")).unwrap().shown, "Anna");
        typed.clear();
        assert_eq!(resolve(&typed, Some("Eva")).unwrap().shown, "Eva");
        assert_eq!(resolve(&typed, Some("Eva Neu")).unwrap().shown, "Eva Neu", "a changed system name is followed");
        assert_eq!(
            resolve("Anna", Some("Eva Neu")).unwrap().shown,
            "Anna",
            "a typed name stays when the system name changes"
        );
    }

    #[test]
    fn the_game_name_follows_the_games_rule() {
        assert_eq!(for_game("Anna").as_deref(), Some("Anna"));
        assert_eq!(for_game("Jürgen Müller").as_deref(), Some("Juergen_Mueller"));
        assert_eq!(for_game("Straße").as_deref(), Some("Strasse"));
        assert_eq!(for_game("José García").as_deref(), Some("Jose_Garcia"));
        assert_eq!(for_game("a_b_1").as_deref(), Some("a_b_1"));
        assert_eq!(for_game("Anna - Lena").as_deref(), Some("Anna_Lena"));
        assert_eq!(for_game("  Eva  ").as_deref(), Some("Eva"));
    }

    #[test]
    fn the_game_name_is_cut_padded_and_never_looks_like_an_option() {
        assert_eq!(for_game("Alexander Hamilton-Smith").as_deref(), Some("Alexander_Hamilt"));
        assert_eq!(for_game("Al").as_deref(), Some("Al_"));
        assert_eq!(for_game("X").as_deref(), Some("X__"));
        assert_eq!(for_game("--world").as_deref(), Some("world"));
        assert_eq!(for_game("--"), None);
        assert_eq!(for_game(""), None);
        assert_eq!(for_game("日本語"), None, "nothing usable: the game keeps its own name");
        let all: Vec<String> = ["Anna Lena", "Ünal", "x", "名前 Eva", "Aaaaaaaaaaaaaaaaaaaaaaaa"]
            .iter()
            .filter_map(|n| for_game(n))
            .collect();
        for name in all {
            assert!((GAME_MIN..=GAME_MAX).contains(&name.len()), "{name}");
            assert!(name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'), "{name}");
        }
    }
}
