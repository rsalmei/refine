use crate::commands::Refine;
use crate::entries::{Depth, Entries, Entry, EntrySet};
use crate::{Warnings, utils};
use anyhow::Result;
use clap::{Args, ValueEnum};
use human_repr::HumanCount;
use std::cmp::Ordering;
use std::sync::OnceLock;
use yansi::{Color, Paint};

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
    /// Do not calculate directory sizes.
    #[arg(short = 'c', long)]
    no_calc_dirs: bool,
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
    size_count: Option<(u64, u32)>,
}

static CALC_DIR_SIZES: OnceLock<bool> = OnceLock::new();

impl Refine for List {
    type Media = Media;
    const OPENING_LINE: &'static str = "List files";
    const HANDLES: EntrySet = EntrySet::ContentOverDirs;

    fn tweak(&mut self, _: &Warnings) {
        if !self.rev {
            const ORDERING: [bool; 3] = [true, false, false];
            self.rev = ORDERING[self.by as usize];
        }
        if let By::Path = self.by {
            self.paths = true;
        }
        CALC_DIR_SIZES.set(!self.no_calc_dirs).unwrap();
    }

    fn refine(&self, mut medias: Vec<Self::Media>) -> Result<()> {
        // step: sort the files by name, size, or path.
        let compare = match self.by {
            By::Size => |m: &Media, n: &Media| {
                m.size_count
                    .map(|(s, _)| s)
                    .cmp(&n.size_count.map(|(s, _)| s))
            },
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
            let (size, count) = match m.size_count {
                Some((s, c)) => (&*format!("{}", s.human_count_bytes()), &*format!("{c}")),
                None => ("?", "?"),
            };
            match self.paths {
                true => print!("{size:>8} {}", m.entry.display_path()),
                false => print!("{size:>8} {}", m.entry.display_filename()),
            };
            if m.entry.is_dir() {
                print!(" {} files", count.paint(Color::Blue).linger());
            }
            println!("{}", "".resetting());
        });

        // step: display receipt summary.
        if !medias.is_empty() {
            println!();
        }
        let (mut size, mut count) = (0, 0);
        medias
            .iter()
            .filter_map(|m| m.size_count)
            .for_each(|(s, c)| {
                size += s;
                count += c;
            });
        println!(
            "listed entries: {}{}",
            medias.len(),
            utils::display_abort(true),
        );
        println!("  total: {} in {count} files", size.human_count("B"),);

        Ok(())
    }
}

impl TryFrom<Entry> for Media {
    type Error = (anyhow::Error, Entry);

    fn try_from(entry: Entry) -> Result<Self, Self::Error> {
        let size_count = match (entry.is_dir(), CALC_DIR_SIZES.get().unwrap()) {
            (true, false) => None,
            (true, true) => {
                let entries = Entries::single(&entry, Depth::Unlimited);
                let mut count = 0;
                let sum = entries
                    .fetch(EntrySet::Files)
                    .map(|e| {
                        count += 1;
                        e.metadata().map_or(0, |md| md.len())
                    })
                    .sum::<u64>();
                Some((sum, count))
            }
            (false, _) => {
                let size = entry
                    .metadata()
                    .map_err(|err| (err.into(), entry.clone()))?
                    .len();
                Some((size, 1))
            }
        };
        Ok(Self { entry, size_count })
    }
}
