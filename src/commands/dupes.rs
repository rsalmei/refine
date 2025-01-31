use crate::command::{EntryKind, Refine};
use crate::utils::{self, Sequence};
use anyhow::Result;
use clap::Args;
use human_repr::HumanCount;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::{fs, io};

#[derive(Debug, Args)]
pub struct Dupes {
    /// Sample size in bytes (0 to disable).
    #[arg(short = 's', long, default_value_t = 2 * 1024, value_name = "BYTES")]
    sample: usize,
}

#[derive(Debug)]
pub struct Media {
    path: PathBuf,
    size: u64,
    words: Box<[String]>,
    sample: Option<Option<Box<[u8]>>>, // only populated if needed, and double to remember when already tried.
}

impl Refine for Dupes {
    type Media = Media;
    const OPENING_LINE: &'static str = "Detecting duplicate files...";
    const ENTRY_KIND: EntryKind = EntryKind::Files;

    fn refine(&self, mut medias: Vec<Self::Media>) -> Result<()> {
        // step: detect duplicates by size.
        println!("by size:");
        let by_size = detect_duplicates(
            &mut medias,
            self.sample,
            |m| &m.size,
            |&size, acc| {
                println!("\n{} x{}", size.human_count_bytes(), acc.len());
                acc.iter().for_each(|&m| println!("{}", m.path.display()));
            },
        );

        // step: detect duplicates by name.
        println!("\nby name:");
        let by_name = detect_duplicates(
            &mut medias,
            self.sample,
            |m| &m.words,
            |words, acc| {
                println!("\n{:?} x{}", words, acc.len());
                acc.iter()
                    .for_each(|m| println!("{}: {}", m.size.human_count_bytes(), m.path.display()));
            },
        );

        // step: display receipt summary.
        let total = medias.len();
        println!("\ntotal files: {total}{}", utils::aborted(by_size == 0));
        println!("  by size: {by_size} dupes{}", utils::aborted(by_name == 0));
        println!("  by name: {by_name} dupes{}", utils::aborted(true));
        Ok(())
    }
}

/// Sort the files by groups, and apply some algorithm on each.
fn detect_duplicates<G, FG, FS>(
    medias: &mut [Media],
    sample: usize,
    grouping: FG,
    show: FS,
) -> usize
where
    G: PartialEq + Ord,
    FG: Fn(&Media) -> &G,
    FS: Fn(&G, Vec<&Media>),
{
    medias.sort_unstable_by(|m1, m2| grouping(m1).cmp(grouping(m2)));
    medias
        .chunk_by_mut(|m, m2| grouping(m) == grouping(m2))
        .filter(|_| utils::is_running())
        .filter(|g| g.len() > 1)
        .flat_map(|g| {
            g.iter_mut().for_each(|m| {
                m.cache_sample(sample); // warm up samples for groups with at least 2 files.
            });
            let mut split = HashMap::with_capacity(g.len());
            g.iter()
                .map(|m| (m, m.sample.as_ref().unwrap()))
                .for_each(|(m, sample)| split.entry(sample).or_insert_with(Vec::new).push(m));
            split.into_values().filter(|v| v.len() > 1)
        })
        .map(|mut g| {
            g.sort_unstable_by(|m, n| m.path.cmp(&n.path));
            show(grouping(g[0]), g)
        })
        .count()
}

fn words(path: &Path) -> Result<Box<[String]>> {
    let (mut name, _) = utils::filename_parts(path)?;
    name = &name[..Sequence::from(name).true_len];
    let mut words = name
        .split(&[' ', '.', '-', '_'])
        .filter(|s| !s.is_empty())
        .filter(|s| !(s.len() == 1 && s.is_ascii())) // remove vowels.
        .map(|s| s.to_lowercase())
        .collect::<Vec<_>>();
    words.sort_unstable();
    words.dedup();
    Ok(words.into_boxed_slice())
}

impl Media {
    fn cache_sample(&mut self, sample: usize) {
        if self.sample.is_none() {
            let grab_sample = || {
                let mut file = File::open(&self.path)?;
                let mut buf = vec![0; sample];
                let mut read = 0;
                while read < buf.len() {
                    let n = file.read(&mut buf[read..])?;
                    if n == 0 {
                        break;
                    }
                    read += n;
                }
                buf.truncate(read);
                Ok::<_, io::Error>(buf)
            };

            self.sample = match grab_sample() {
                Ok(buf) => Some(Some(buf.into_boxed_slice())),
                Err(err) => {
                    eprintln!("error: load sample: {err:?}");
                    Some(None)
                }
            };
        }
    }
}

impl TryFrom<PathBuf> for Media {
    type Error = anyhow::Error;

    fn try_from(path: PathBuf) -> Result<Self> {
        Ok(Media {
            size: fs::metadata(&path)?.len(),
            words: words(&path)?,
            path, // I can use path above before moving it here!
            sample: None,
        })
    }
}
