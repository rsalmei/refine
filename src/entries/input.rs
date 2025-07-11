use crate::entries::{Entry, Fetcher, FilterSpec};
use anyhow::{Result, anyhow};
use clap::Args;
use std::path::PathBuf;

#[derive(Debug, Args)]
pub struct Input {
    /// Directories to scan.
    #[arg(global = true, help_heading = None)]
    dirs: Vec<PathBuf>,
    /// The maximum recursion depth; use 0 for unlimited.
    #[arg(short = 'R', long, default_value_t = 0, value_name = "INT", global = true, help_heading = Some("Fetch"))]
    recursion: u32,
    #[command(flatten)]
    filter: FilterSpec,
}

/// Information about the input paths provided by the user.
#[derive(Debug)]
pub struct InputInfo {
    /// The effective number of paths to scan, after deduplication and validation.
    pub num_valid: usize,
    /// Whether there were invalid/not found paths.
    pub has_invalid: bool,
}

impl TryFrom<Input> for (Fetcher, InputInfo) {
    type Error = anyhow::Error;

    fn try_from(input: Input) -> Result<(Fetcher, InputInfo)> {
        let (dirs, info) = validate_dirs(input.dirs)?;
        let fetcher = Fetcher::new(dirs, input.recursion.into(), input.filter)?;
        Ok((fetcher, info))
    }
}

fn validate_dirs(mut dirs: Vec<PathBuf>) -> Result<(Vec<Entry>, InputInfo)> {
    if dirs.is_empty() {
        dirs = vec![".".into()]; // use the current directory if no paths are given.
    }
    let n = dirs.len();
    dirs.sort_unstable();
    dirs.dedup();
    if n != dirs.len() {
        eprintln!("warning: {} duplicated directories ignored", n - dirs.len());
    }

    let n = dirs.len();
    let dirs = dirs
        .into_iter()
        .map(Entry::try_from)
        .filter_map(|res| match res {
            Ok(entry) if entry.is_dir() => Some(entry),
            Ok(entry) => {
                eprintln!("warning: {entry} is not a directory, skipping");
                None
            }
            Err((pb, err)) => {
                eprintln!("warning: invalid path {pb:?}: {err}");
                None
            }
        })
        .collect::<Vec<_>>();

    if dirs.is_empty() {
        return Err(anyhow!("no valid paths given"));
    }

    let info = InputInfo {
        num_valid: dirs.len(),
        has_invalid: n != dirs.len(),
    };
    Ok((dirs, info))
}
