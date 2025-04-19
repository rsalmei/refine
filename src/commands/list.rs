use crate::commands::Refine;
use crate::entries::input::Warnings;
use crate::entries::{Entry, Fetcher, Recurse, TraversalMode};
use crate::utils;
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
    /// Reverse the default order (size/count:desc, name/path:asc).
    #[arg(short = 'r', long)]
    rev: bool,
    /// Show full file paths.
    #[arg(short = 'p', long)]
    paths: bool,
    /// Do not calculate directory sizes.
    #[arg(short = 'c', long)]
    no_calc_dirs: bool,
}

#[derive(Debug, Copy, Clone, PartialEq, ValueEnum)]
pub enum By {
    #[value(alias = "s")]
    Size,
    #[value(alias = "c")]
    Count,
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

const ORDERING: &[(By, bool)] = &[
    (By::Size, true),
    (By::Count, true),
    (By::Name, false),
    (By::Path, false),
];
static CALC_DIR_SIZES: OnceLock<bool> = OnceLock::new();

impl Refine for List {
    type Media = Media;
    const OPENING_LINE: &'static str = "List files";
    const MODE: TraversalMode = TraversalMode::ContentOverDirs;

    fn tweak(&mut self, _: &Warnings) {
        self.rev ^= ORDERING.iter().find(|(b, _)| *b == self.by).unwrap().1;
        if self.by == By::Path && !self.paths {
            self.paths = true;
            eprintln!("Enabling full file paths due to path sorting.\n");
        }
        CALC_DIR_SIZES.set(!self.no_calc_dirs).unwrap();
    }

    fn refine(&self, mut medias: Vec<Self::Media>) -> Result<()> {
        // step: sort the files by size, count, name, or path.
        let compare = match self.by {
            By::Size => |m: &Media, n: &Media| {
                m.size_count
                    .map(|(s, _)| s)
                    .cmp(&n.size_count.map(|(s, _)| s))
            },
            By::Count => |m: &Media, n: &Media| {
                m.size_count
                    .map(|(_, c)| c)
                    .cmp(&n.size_count.map(|(_, c)| c))
            },
            By::Name => |m: &Media, n: &Media| m.entry.file_name().cmp(n.entry.file_name()),
            By::Path => |m: &Media, n: &Media| m.entry.cmp(&n.entry),
        };
        let compare: &dyn Fn(&Media, &Media) -> Ordering = match self.rev {
            false => &compare,
            true => &|m, n| compare(m, n).reverse(),
        };
        medias.sort_unstable_by(|m, n| compare(m, n).then_with(|| m.entry.cmp(&n.entry)));

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
            if m.entry.is_dir() && m.size_count.is_some() {
                print!(" {} files", count.paint(Color::Blue).linger());
            }
            println!("{}", "".resetting());
        });

        // step: display summary receipt.
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
                let fetcher = Fetcher::single(&entry, Recurse::Full);
                let mut count = 0;
                let sum = fetcher
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
