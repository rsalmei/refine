use crate::entries::Entry;
use crate::fetcher::{Fetcher, FetcherArgs};
use crate::warning;
use anyhow::{Result, anyhow};
use std::path::PathBuf;

/// Information about the input entries, used to tweak the behavior of commands.
#[derive(Debug)]
pub struct InputInfo {
    /// The total number of valid entries.
    pub num_valid: usize,
    /// Whether there were invalid/not found paths (the exact number is not relevant).
    pub had_invalid: bool,
}

impl TryFrom<FetcherArgs> for (Fetcher, InputInfo) {
    type Error = anyhow::Error;

    fn try_from(args: FetcherArgs) -> Result<(Fetcher, InputInfo)> {
        let (dirs, info) = validate(args.dirs)?;
        Ok((
            Fetcher {
                dirs,
                recursion: args.recursion.into(),
                filter: args.filter.try_into()?,
            },
            info,
        ))
    }
}

/// Validate the given paths, and return the valid ones as entries, along with information about
/// them. Validation includes deduplication and filtering out non-directories, invalid paths, and
/// directories contained within another already in the list.
fn validate(mut dirs: Vec<PathBuf>) -> Result<(Vec<Entry>, InputInfo)> {
    if dirs.is_empty() {
        dirs = vec![".".into()]; // use the current directory if no paths are given.
    }
    let n = dirs.len();
    dirs.sort_unstable();
    dirs.dedup();
    if n != dirs.len() {
        warning!("duplicated directories: {}", n - dirs.len());
    }

    // try to convert all paths to entries, and filter out non-directories and invalid ones, while
    // counting them for the info struct.
    let n = dirs.len();
    let dirs = dirs
        .into_iter()
        .map(Entry::try_from)
        .filter_map(|res| match res {
            Ok(entry) if entry.is_dir() => Some(entry),
            Ok(entry) => {
                warning!("entry is not a directory: {entry}");
                None
            }
            Err((pb, err)) => {
                warning!("invalid path: {pb:?}: {err}");
                None
            }
        })
        .collect::<Vec<_>>();
    let had_invalid = n != dirs.len();

    // filter out directories contained within another already in the list. since dirs is sorted,
    // a parent always comes before its children, so only the last accepted entry needs to be
    // checked against the current one.
    let dirs = dirs
        .into_iter()
        .fold(Vec::new(), |mut acc: Vec<Entry>, entry| {
            match acc.last().is_some_and(|prev| {
                let (s, p) = (entry.to_str(), prev.to_str());
                s.starts_with(p) && s[p.len()..].starts_with('/')
            }) {
                true => warning!("dir is contained within another: {entry}"),
                false => acc.push(entry),
            }
            acc
        });
    if dirs.is_empty() {
        return Err(anyhow!("no valid paths given"));
    }

    let info = InputInfo {
        num_valid: dirs.len(),
        had_invalid,
    };
    Ok((dirs, info))
}
