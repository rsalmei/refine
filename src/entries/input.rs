use crate::entries::{Entry, Fetcher, Filter};
use anyhow::{Result, anyhow};
use clap::Args;
use std::path::PathBuf;

#[derive(Debug, Args)]
pub struct Input {
    /// Just show the entries that would be processed, without running any command.
    #[arg(long, global = true)]
    show: bool,
    /// Directories to scan.
    #[arg(global = true, help_heading = None)]
    dirs: Vec<PathBuf>,
    /// The maximum recursion depth; use 0 for unlimited.
    #[arg(short = 'R', long, default_value_t = 0, value_name = "INT", global = true, help_heading = Some("Fetch"))]
    recursion: u32,
    #[command(flatten)]
    filter: Filter,
}

/// The input data structure that holds the effective paths to scan and their properties.
#[derive(Debug)]
pub struct EffectiveInput {
    pub info: InputInfo,
    pub show: bool,
    pub fetcher: Fetcher,
}

#[derive(Debug)]
pub struct InputInfo {
    /// The effective number of paths to scan, after deduplication and validation.
    pub num_valid: usize,
    /// Whether there were invalid/not found paths.
    pub has_invalid: bool,
}

impl TryFrom<Input> for EffectiveInput {
    type Error = anyhow::Error;

    fn try_from(input: Input) -> Result<EffectiveInput> {
        let (dirs, info) = validate(input.dirs)?;
        if dirs.is_empty() {
            return Err(anyhow!("no valid paths given"));
        }
        let filter = input.filter.try_into()?;
        let fetcher = Fetcher::new(dirs, input.recursion.into(), filter);
        let ei = EffectiveInput {
            info,
            show: input.show,
            fetcher,
        };
        Ok(ei)
    }
}

fn validate(mut dirs: Vec<PathBuf>) -> Result<(Vec<Entry>, InputInfo)> {
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
