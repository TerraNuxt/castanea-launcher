//! The launch plan: program, arguments, working folder and environment, as plain data.

use std::path::PathBuf;

use super::start::Start;
use crate::install::Build;
use crate::settings::GameOptions;
use crate::{Error, Result};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub env: Vec<(String, String)>,
}

impl Plan {
    /// The command as one line for the log; arguments with spaces are quoted.
    pub fn display(&self) -> String {
        let mut line = self.program.display().to_string();
        for arg in &self.args {
            line.push(' ');
            if arg.is_empty() || arg.contains(char::is_whitespace) || arg.contains('"') {
                line.push_str(&format!("\"{}\"", arg.replace('"', "\\\"")));
            } else {
                line.push_str(arg);
            }
        }
        line
    }
}

/// Options that name where to go, so the user wants no start screen: a seed first, or one of these.
fn starts_directly(extra: &[String]) -> bool {
    extra.first().is_some_and(|first| !first.starts_with("--"))
        || extra.iter().any(|arg| matches!(arg.as_str(), "--menu" | "--world" | "--join" | "--host"))
}

/// Builds the plan for `build` with `options`, run in `game_dir`. The game opens its start screen: it does so only for no
/// arguments or `--menu`, and `--name` is an argument, so `--menu` goes first unless the extra arguments name a world.
pub fn plan(options: &GameOptions, build: &Build, game_dir: PathBuf) -> Result<Plan> {
    plan_for(options, build, game_dir, &Start::Menu)
}

/// `plan` for a start that may go straight into a world or onto a server: those arguments come first, the extra
/// arguments after them. The player name is passed in the form the game accepts (`playername::for_game`).
pub fn plan_for(options: &GameOptions, build: &Build, game_dir: PathBuf, start: &Start) -> Result<Plan> {
    let extra = split_args(&options.extra_args)?;
    let mut args = match start {
        Start::Menu if starts_directly(&extra) => Vec::new(),
        other => other.args()?,
    };
    if let Some(player) = crate::playername::for_game(&options.player_name) {
        args.extend(["--name".to_owned(), player]);
    }
    args.extend(extra);

    let mut env = Vec::new();
    if let Some(value) = options.graphics.env_value() {
        env.push(("CASTANEA_BACKEND".to_owned(), value.to_owned()));
    }
    let window = options.window.trim();
    if !window.is_empty() {
        let valid = window.split_once('x').is_some_and(|(w, h)| w.parse::<u32>().is_ok() && h.parse::<u32>().is_ok());
        if !valid {
            return Err(Error::invalid(format!("window size `{window}` must look like 1280x720")));
        }
        env.push(("CASTANEA_WINDOW".to_owned(), window.to_owned()));
    }
    for (key, value) in &options.env {
        let key = key.trim();
        if key.is_empty() {
            continue;
        }
        if key.contains('=') || key.contains('\0') || value.contains('\0') {
            return Err(Error::invalid(format!("environment variable `{key}` is not valid")));
        }
        env.retain(|(k, _)| k != key);
        env.push((key.to_owned(), value.clone()));
    }
    Ok(Plan { program: build.binary.clone(), args, cwd: game_dir, env })
}

/// Splits like a shell without expansion; a backslash escapes only a quote, space or backslash (Windows paths stay).
pub fn split_args(text: &str) -> Result<Vec<String>> {
    let mut out = Vec::new();
    let mut word = String::new();
    let mut in_word = false;
    let mut chars = text.chars();
    let mut quote: Option<char> = None;
    while let Some(c) = chars.next() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some('\''), c) => word.push(c),
            (Some(_), '\\') | (None, '\\') => match chars.clone().next() {
                Some(next @ ('"' | '\'' | '\\' | ' ')) => {
                    chars.next();
                    word.push(next);
                }
                _ => word.push('\\'),
            },
            (Some(_), c) => word.push(c),
            (None, '"' | '\'') => {
                quote = Some(c);
                in_word = true;
            }
            (None, c) if c.is_whitespace() => {
                if in_word {
                    out.push(std::mem::take(&mut word));
                    in_word = false;
                }
            }
            (None, c) => {
                word.push(c);
                in_word = true;
            }
        }
        if quote.is_none() && !word.is_empty() {
            in_word = true;
        }
    }
    if quote.is_some() {
        return Err(Error::invalid("extra arguments have an open quote"));
    }
    if in_word {
        out.push(word);
    }
    Ok(out)
}
