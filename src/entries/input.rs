use crate::entries::{Entry, Fetcher, Filter};
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
    recurse: u32,
    #[command(flatten)]
    filter: Filter,
}

/// Warnings that were encountered while parsing the input paths.
#[derive(Debug)]
pub struct Warnings {
    /// Whether there were missing paths.
    pub missing: bool,
}

impl TryFrom<Input> for (Fetcher, Warnings) {
    type Error = anyhow::Error;

    fn try_from(input: Input) -> Result<(Fetcher, Warnings)> {
        let (dirs, warnings) = validate_dirs(input.dirs)?;
        let fetcher = Fetcher::new(dirs, input.recurse.into(), input.filter)?;
        Ok((fetcher, warnings))
    }
}

fn validate_dirs(mut dirs: Vec<PathBuf>) -> Result<(Vec<Entry>, Warnings)> {
    if dirs.is_empty() {
        dirs = vec![".".into()]; // use the current directory if no paths are given.
    }
    let n = dirs.len();
    dirs.sort_unstable();
    dirs.dedup();
    if n != dirs.len() {
        eprintln!("warning: {} duplicated directories ignored", n - dirs.len());
    }

    let (dirs, missing) = dirs
        .into_iter()
        .map(|pb| Entry::try_from(&*pb).map_err(|err| (pb, err)))
        .inspect(|res| {
            if let Err((pb, err)) = res {
                eprintln!("warning: invalid directory {pb:?}: {err}");
            }
        })
        .flatten()
        .partition::<Vec<_>, _>(|entry| entry.is_dir());

    missing
        .iter()
        .for_each(|entry| eprintln!("warning: directory not found: {entry}"));
    if dirs.is_empty() {
        return Err(anyhow!("no valid paths given"));
    }

    let warnings = Warnings {
        missing: !missing.is_empty(),
    };
    Ok((dirs, warnings))
}
