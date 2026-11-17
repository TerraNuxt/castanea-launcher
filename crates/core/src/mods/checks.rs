//! Problems with a profile's mods: broken manifests, duplicate ids, missing or disabled dependencies, cycles,
//! keys the game refuses, and game versions outside a mod's range.

use std::collections::{HashMap, HashSet};

use super::installed::InstalledMod;
use crate::version::{self, Compat};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Problem {
    Broken(String),
    DuplicateId,
    MissingDependency(String),
    DisabledDependency(String),
    Cycle,
    UnknownKeys(Vec<String>),
    Incompatible { range: String, game: String },
}

/// The problems of each mod, keyed by its file name. `ranges` maps a mod id to its game range (from the index).
pub fn check(mods: &[InstalledMod], game: Option<&str>, ranges: &HashMap<String, String>) -> Vec<(String, Problem)> {
    let mut out = Vec::new();
    let mut by_id: HashMap<&str, Vec<&InstalledMod>> = HashMap::new();
    for package in mods {
        by_id.entry(package.id()).or_default().push(package);
    }
    for package in mods {
        let file = package.file_name.clone();
        let manifest = match &package.manifest {
            Ok(manifest) => manifest,
            Err(error) => {
                out.push((file, Problem::Broken(error.clone())));
                continue;
            }
        };
        if by_id[manifest.id.as_str()].len() > 1 {
            out.push((file.clone(), Problem::DuplicateId));
        }
        if !manifest.unknown_keys.is_empty() {
            out.push((file.clone(), Problem::UnknownKeys(manifest.unknown_keys.clone())));
        }
        if package.enabled {
            for dep in &manifest.depends {
                match by_id.get(dep.as_str()) {
                    None => out.push((file.clone(), Problem::MissingDependency(dep.clone()))),
                    Some(list) if !list.iter().any(|m| m.enabled) => {
                        out.push((file.clone(), Problem::DisabledDependency(dep.clone())))
                    }
                    Some(_) => {}
                }
            }
        }
        if let Some(range) = ranges.get(&manifest.id)
            && version::compat(Some(range), game) == Compat::No
        {
            out.push((file.clone(), Problem::Incompatible { range: range.clone(), game: game.unwrap_or("").to_owned() }));
        }
    }
    for id in cycle_members(mods) {
        for package in mods.iter().filter(|m| m.enabled && m.id() == id) {
            out.push((package.file_name.clone(), Problem::Cycle));
        }
    }
    out
}

/// Ids of enabled mods that sit on a dependency cycle.
fn cycle_members(mods: &[InstalledMod]) -> HashSet<String> {
    let mut graph: HashMap<&str, Vec<&str>> = HashMap::new();
    for manifest in mods.iter().filter(|m| m.enabled).filter_map(|m| m.manifest.as_ref().ok()) {
        graph.entry(manifest.id.as_str()).or_default().extend(manifest.depends.iter().map(String::as_str));
    }
    let mut on_cycle = HashSet::new();
    for &start in graph.keys() {
        let mut stack: Vec<&str> = graph[start].clone();
        let mut seen = HashSet::new();
        while let Some(next) = stack.pop() {
            if next == start {
                on_cycle.insert(start.to_owned());
                break;
            }
            if seen.insert(next)
                && let Some(deps) = graph.get(next)
            {
                stack.extend(deps.iter().copied());
            }
        }
    }
    on_cycle
}

/// The mods that must be enabled with `id`, deepest first, from what is installed.
pub fn dependencies_of<'a>(mods: &'a [InstalledMod], id: &str) -> Vec<&'a InstalledMod> {
    let mut out: Vec<&InstalledMod> = Vec::new();
    let mut stack = vec![id.to_owned()];
    let mut seen = HashSet::new();
    while let Some(next) = stack.pop() {
        if !seen.insert(next.clone()) {
            continue;
        }
        let Some(package) = mods.iter().find(|m| m.id() == next) else { continue };
        if let Ok(manifest) = &package.manifest {
            stack.extend(manifest.depends.iter().cloned());
        }
        if next != id {
            out.push(package);
        }
    }
    out.reverse();
    out
}
