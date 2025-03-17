use crate::Warnings;
use crate::commands::Refine;
use crate::entries::{Entry, EntrySet};
use anyhow::Result;
use clap::{Args, ValueEnum};
use human_repr::HumanCount;
use std::cmp::Ordering;

#[derive(Debug, Args)]
pub struct List {
    /// Sort by.
    #[arg(short = 'b', long, default_value_t = By::Size, value_name = "STR", value_enum)]
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
    #[value(alias = "s")]
    Size,
    #[value(alias = "n")]
    Name,
    #[value(alias = "p")]
    Path,
}

#[derive(Debug)]
pub struct Media {
    entry: Entry,
    size: u64,
}

impl Refine for List {
    type Media = Media;
    const OPENING_LINE: &'static str = "List files";
    const HANDLES: EntrySet = EntrySet::Files;

    fn tweak(&mut self, _: &Warnings) {
        if !self.rev {
            const ORDERING: [bool; 3] = [true, false, false];
            self.rev = ORDERING[self.by as usize];
        }
        if let By::Path = self.by {
            self.paths = true;
        }
    }

    fn refine(&self, mut medias: Vec<Self::Media>) -> Result<()> {
        // step: sort the files by name, size, or path.
        let compare = match self.by {
            By::Size => |m: &Media, n: &Media| m.size.cmp(&n.size),
            By::Name => |m: &Media, n: &Media| m.entry.file_name().cmp(n.entry.file_name()),
            By::Path => |m: &Media, n: &Media| m.entry.cmp(&n.entry),
        };
        let compare: &dyn Fn(&Media, &Media) -> Ordering = match self.rev {
            false => &compare,
            true => &|m, n| compare(m, n).reverse(),
        };
        medias.sort_unstable_by(compare);

        // step: display the results.
        medias.iter().for_each(|m| {
            let size = format!("{}", m.size.human_count_bytes());
            match self.paths {
                true => println!("{size:>7} {}", m.entry.display_path()),
                false => println!("{size:>7} {}", m.entry.display_filename()),
            };
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

impl TryFrom<Entry> for Media {
    type Error = (anyhow::Error, Entry);

        Ok(Self {
            size: entry.metadata()?.len(),
            entry,
        })
    fn try_from(entry: Entry) -> Result<Self, Self::Error> {
    }
}
