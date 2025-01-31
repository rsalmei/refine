use super::{EntryKind, Fetcher, Refine};
use anyhow::Result;
use clap::{Args, ValueEnum};
use human_repr::HumanCount;
use std::borrow::Cow;
use std::cmp::Ordering;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Args)]
pub struct List {
    /// Sort by.
    #[arg(short = 'b', long, value_enum, default_value_t = By::Name)]
    by: By,
    /// Reverse the default order (name:asc, size:desc, path:asc).
    #[arg(short = 'r', long)]
    rev: bool,
    /// Show full file paths.
    #[arg(short = 'p', long)]
    paths: bool,
}

#[derive(Debug, Copy, Clone, ValueEnum)]
pub enum By {
    #[value(alias = "n")]
    Name,
    #[value(alias = "s")]
    Size,
    #[value(alias = "p")]
    Path,
}

#[derive(Debug)]
pub struct Media {
    path: PathBuf,
    size: u64,
}

impl Refine for List {
    type Media = Media;
    const OPENING_LINE: &'static str = "Listing files...";
    const ENTRY_KIND: EntryKind = EntryKind::Files;

    fn adjust(&mut self, _fetcher: &Fetcher) {
        if !self.rev {
            const ORDERING: [bool; 3] = [false, true, false];
            self.rev = ORDERING[self.by as usize];
        }
        if let By::Path = self.by {
            self.paths = true;
        }
    }

    fn refine(&self, mut medias: Vec<Self::Media>) -> Result<()> {
        // step: sort the files by name, size, or path.
        let compare = match self.by {
            By::Name => |m: &Media, n: &Media| m.path.file_name().cmp(&n.path.file_name()),
            By::Size => |m: &Media, n: &Media| m.size.cmp(&n.size),
            By::Path => |m: &Media, n: &Media| m.path.cmp(&n.path),
        };
        let compare: &dyn Fn(&Media, &Media) -> Ordering = match self.rev {
            false => &compare,
            true => &|m, n| compare(m, n).reverse(),
        };
        medias.sort_unstable_by(compare);

        // step: display the results.
        let show: fn(m: &Media) -> Cow<str> = match self.paths {
            true => |m| m.path.to_string_lossy(),
            false => |m| m.path.file_name().unwrap().to_string_lossy(),
        };
        medias.iter().for_each(|m| {
            println!(
                "{:>7} {}",
                format!("{}", m.size.human_count_bytes()),
                show(m)
            )
        });

        // step: display receipt summary.
        if !medias.is_empty() {
            println!();
        }
        let size = medias.iter().map(|m| m.size).sum::<u64>();
        println!("total files: {} ({})", medias.len(), size.human_count("B"));

        Ok(())
    }
}

impl TryFrom<PathBuf> for Media {
    type Error = anyhow::Error;

    fn try_from(path: PathBuf) -> Result<Self> {
        Ok(Self {
            size: fs::metadata(&path)?.len(),
            path,
        })
    }
}
