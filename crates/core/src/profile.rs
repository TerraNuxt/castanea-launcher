//! Profiles (instances): which build to start, with which options, in which game folder.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::json_file;
use crate::paths::{Paths, safe_name};
use crate::{Error, Result};

/// The game's renderer, through `CASTANEA_BACKEND` (docs/dev/wgpu-backend.md of the game).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Graphics {
    /// The game's own setting decides.
    #[default]
    Auto,
    Vulkan,
    Wgpu,
}

impl Graphics {
    pub const ALL: [Self; 3] = [Self::Auto, Self::Vulkan, Self::Wgpu];

    pub fn env_value(self) -> Option<&'static str> {
        match self {
            Self::Auto => None,
            Self::Vulkan => Some("vulkan"),
            Self::Wgpu => Some("wgpu"),
        }
    }
}

/// How the game starts: the menu, one world, or joining a server.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StartIn {
    #[default]
    Menu,
    World { name: String, seed: String, host_port: Option<u16> },
    Join { address: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Profile {
    pub id: String,
    pub name: String,
    /// A build id, or `None` for the newest installed build.
    pub build: Option<String>,
    pub start: StartIn,
    /// The player name (`--name`), used when joining.
    pub player_name: String,
    /// The relay backend, written into the profile's `settings.json` as `backend_url`; empty leaves it alone.
    pub backend_url: String,
    pub graphics: Graphics,
    /// `CASTANEA_WINDOW`, such as `1280x720`; empty for the game's default.
    pub window: String,
    /// More arguments, split like a shell does (quotes group words).
    pub extra_args: String,
    pub env: Vec<(String, String)>,
    /// A game folder of the user's choice; `None` is `data/instances/<id>`.
    pub folder: Option<PathBuf>,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            id: "default".to_owned(),
            name: "Default".to_owned(),
            build: None,
            start: StartIn::Menu,
            player_name: String::new(),
            backend_url: String::new(),
            graphics: Graphics::Auto,
            window: String::new(),
            extra_args: String::new(),
            env: Vec::new(),
            folder: None,
        }
    }
}

impl Profile {
    /// The game's working directory for this profile.
    pub fn game_dir(&self, paths: &Paths) -> PathBuf {
        self.folder.clone().unwrap_or_else(|| paths.instances().join(&self.id))
    }

    pub fn mods_dir(&self, paths: &Paths) -> PathBuf {
        self.game_dir(paths).join("mods")
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profiles {
    pub profiles: Vec<Profile>,
}

impl Profiles {
    /// The saved profiles; a first start gets one default profile.
    pub fn load(paths: &Paths) -> Result<Self> {
        let mut profiles: Self = json_file::load_or_default(&paths.profiles_file())?;
        if profiles.profiles.is_empty() {
            profiles.profiles.push(Profile::default());
        }
        Ok(profiles)
    }

    pub fn save(&self, paths: &Paths) -> Result<()> {
        json_file::save(&paths.profiles_file(), self)
    }

    pub fn get(&self, id: &str) -> Option<&Profile> {
        self.profiles.iter().find(|p| p.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut Profile> {
        self.profiles.iter_mut().find(|p| p.id == id)
    }

    /// A new profile named `name` with a free id; returns the id.
    pub fn add(&mut self, name: &str) -> String {
        let base = safe_name(&name.to_lowercase());
        let mut id = base.clone();
        let mut n = 2;
        while self.get(&id).is_some() {
            id = format!("{base}-{n}");
            n += 1;
        }
        self.profiles.push(Profile { id: id.clone(), name: name.to_owned(), ..Profile::default() });
        id
    }

    /// Removes the profile from the list; its game folder stays on disk.
    pub fn remove(&mut self, id: &str) -> Result<Profile> {
        if self.profiles.len() == 1 {
            return Err(Error::invalid("the last profile cannot be removed"));
        }
        let index = self.profiles.iter().position(|p| p.id == id).ok_or_else(|| Error::invalid(format!("no profile `{id}`")))?;
        Ok(self.profiles.remove(index))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_first_start_has_one_profile_and_ids_stay_unique() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::portable(dir.path());
        let mut profiles = Profiles::load(&paths).unwrap();
        assert_eq!(profiles.profiles.len(), 1);
        assert_eq!(profiles.add("Modded World"), "modded_world");
        assert_eq!(profiles.add("Modded World"), "modded_world-2");
        profiles.get_mut("modded_world").unwrap().graphics = Graphics::Wgpu;
        profiles.save(&paths).unwrap();
        let back = Profiles::load(&paths).unwrap();
        assert_eq!(back, profiles);
        assert_eq!(back.get("default").unwrap().game_dir(&paths), paths.instances().join("default"));
    }

    #[test]
    fn the_last_profile_stays() {
        let mut profiles = Profiles { profiles: vec![Profile::default()] };
        assert!(profiles.remove("default").is_err());
        profiles.add("Two");
        assert!(profiles.remove("default").is_ok());
        assert!(profiles.remove("nope").is_err());
    }
}
