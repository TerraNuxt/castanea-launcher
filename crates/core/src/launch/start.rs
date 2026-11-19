//! Where a start goes: the game's start screen, straight into a saved world, or onto a server.

use serde::{Deserialize, Serialize};

use crate::{Error, Result};

/// What a start asks the game for. The game opens its start screen only for `--menu` (or no arguments); `--world` and
/// `--join` go straight in.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "target", rename_all = "snake_case")]
pub enum Start {
    #[default]
    Menu,
    /// A saved world by its folder name.
    World(String),
    /// A server by `host:port` or a join code.
    Join(String),
}

/// The game's rule for a world name (`save::valid_name`): 1 to 64 letters, digits, `-` or `_`; one that starts with `-` would read
/// as an option after `--world`, so the launcher refuses it.
pub fn valid_world(name: &str) -> bool {
    (1..=64).contains(&name.len())
        && !name.starts_with('-')
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// A server address or join code the launcher hands the game: no white space, not an option, plain host characters.
pub fn valid_address(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 255
        && !text.starts_with('-')
        && text.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b':' | b'-' | b'_' | b'[' | b']'))
}

impl Start {
    /// The game's arguments for this start, or why it cannot be asked for.
    pub fn args(&self) -> Result<Vec<String>> {
        match self {
            Self::Menu => Ok(vec!["--menu".to_owned()]),
            Self::World(name) if valid_world(name) => Ok(vec!["--world".to_owned(), name.clone()]),
            Self::Join(address) if valid_address(address) => Ok(vec!["--join".to_owned(), address.clone()]),
            Self::World(name) => Err(Error::invalid(format!("world name `{name}` is not valid"))),
            Self::Join(address) => Err(Error::invalid(format!("server address `{address}` is not valid"))),
        }
    }

    /// What a start with these (extra) arguments goes to: the world after `--world`, the server after `--join`, else
    /// the start screen. This is how the launcher knows where a start went when the user named it in the extra arguments.
    pub fn from_args(args: &[String]) -> Self {
        let value = |flag: &str| {
            let at = args.iter().position(|a| a == flag)?;
            args.get(at + 1).filter(|v| !v.starts_with("--")).cloned()
        };
        if let Some(address) = value("--join").filter(|a| valid_address(a)) {
            return Self::Join(address);
        }
        if let Some(world) = value("--world").filter(|w| valid_world(w)) {
            return Self::World(world);
        }
        Self::Menu
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(text: &str) -> Vec<String> {
        text.split_whitespace().map(str::to_owned).collect()
    }

    #[test]
    fn each_start_has_its_arguments() {
        assert_eq!(Start::Menu.args().unwrap(), ["--menu"]);
        assert_eq!(Start::World("lake_1".into()).args().unwrap(), ["--world", "lake_1"]);
        assert_eq!(Start::Join("10.0.0.5:47800".into()).args().unwrap(), ["--join", "10.0.0.5:47800"]);
        assert_eq!(Start::Join("AB12CD".into()).args().unwrap(), ["--join", "AB12CD"]);
        assert_eq!(Start::Join("[::1]:47800".into()).args().unwrap(), ["--join", "[::1]:47800"]);
    }

    #[test]
    fn names_and_addresses_that_could_be_options_or_paths_are_refused() {
        for world in ["", "--menu", "a b", "../x", "x/y", &"w".repeat(65), "ü"] {
            assert!(Start::World(world.into()).args().is_err(), "{world:?}");
        }
        for address in ["", "--host", "a b", "a;b", "host/path", &"h".repeat(256), "-x"] {
            assert!(Start::Join(address.into()).args().is_err(), "{address:?}");
        }
    }

    #[test]
    fn extra_arguments_tell_where_a_start_went() {
        assert_eq!(Start::from_args(&words("--world lake --gamemode creative")), Start::World("lake".into()));
        assert_eq!(Start::from_args(&words("--join 10.0.0.5:47800 --name Anna")), Start::Join("10.0.0.5:47800".into()));
        assert_eq!(Start::from_args(&words("--gamemode creative")), Start::Menu);
        assert_eq!(Start::from_args(&words("--world")), Start::Menu);
        assert_eq!(Start::from_args(&words("--world --menu")), Start::Menu);
        assert_eq!(Start::from_args(&words("--world bad/name")), Start::Menu);
        assert_eq!(Start::from_args(&[]), Start::Menu);
    }

    #[test]
    fn a_start_round_trips_through_json() {
        for start in [Start::Menu, Start::World("lake".into()), Start::Join("host:1".into())] {
            let text = serde_json::to_string(&start).unwrap();
            assert_eq!(serde_json::from_str::<Start>(&text).unwrap(), start, "{text}");
        }
    }
}
