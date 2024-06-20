use anyhow::{anyhow, Result};
use clap::Args;
use human_repr::HumanCount;
use regex::Regex;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::{fs, io};

#[derive(Debug, Args)]
pub struct Dupes {
    /// Sample size in bytes (0 to disable).
    #[arg(short, long, default_value_t = 2 * 1024)]
    pub sample: usize,
    /// Case-sensitive file name comparison.
    #[arg(short, long)]
    pub case: bool,
}

static OPT: OnceLock<Dupes> = OnceLock::new();

pub fn find_dupes(files: impl Iterator<Item = PathBuf>, opt: Dupes, _verbose: bool) -> Result<()> {
    println!("Detecting duplicate files...");
    println!("  - sample bytes: {}", opt.sample.human_count_bytes());
    println!("  - match case: {}", opt.case);
    OPT.set(opt).unwrap();
    let mut medias = super::gen_medias(files); // gen medias only after setting OPT.

    // first by size.
    println!("\n-- by size");
    let size_count = detect_duplicates(
        &mut medias,
        |m| &m.size,
        |&size, mut acc| {
            println!("\n{} x{}", size.human_count_bytes(), acc.len());
            acc.sort_unstable();
            acc.iter().for_each(|&m| println!("{}", m.path.display()));
        },
    );

    // then by name.
    println!("\n-- by name");
    let name_count = detect_duplicates(
        &mut medias,
        |m| &m.words,
        |words, mut acc| {
            println!("\n{:?} x{}", words, acc.len());
            acc.sort_unstable();
            acc.iter()
                .for_each(|m| println!("{}: {}", m.size.human_count_bytes(), m.path.display()));
        },
    );

    println!("\ntotal files: {}", medias.len());
    println!("  by size: {size_count} duplicates");
    println!("  by name: {name_count} duplicates");
    Ok(())
}

/// Sort the files by groups, and apply some algorithm on each.
fn detect_duplicates<G, FG, FS>(medias: &mut [Media], grouping: FG, show: FS) -> usize
where
    G: PartialEq + Ord,
    FG: Fn(&Media) -> &G,
    FS: Fn(&G, Vec<&Media>),
{
    medias.sort_unstable_by(|m1, m2| grouping(m1).cmp(grouping(m2)));
    medias
        .chunk_by_mut(|m, m2| grouping(m) == grouping(m2))
        .flat_map(|acc| {
            acc.iter_mut().for_each(|m| {
                m.cache_sample(); // warm up samples.
            });
            let mut split = HashMap::with_capacity(acc.len());
            acc.iter()
                .map(|m| (m, m.sample.as_ref().unwrap()))
                .for_each(|(m, sample)| split.entry(sample).or_insert_with(Vec::new).push(m));
            split.into_values().filter(|v| v.len() > 1)
        })
        .map(|acc| show(grouping(acc[0]), acc))
        .count()
}

#[derive(Debug)]
pub struct Media {
    path: PathBuf,
    size: u64,
    words: Box<[String]>,
    sample: Option<Option<Box<[u8]>>>, // only populated if needed, and double to remember when already tried.
}

impl TryFrom<PathBuf> for Media {
    type Error = anyhow::Error;

    fn try_from(path: PathBuf) -> Result<Self> {
        Ok(Media {
            size: fs::metadata(&path)?.len(),
            words: Media::words(&path)?,
            path, // I can use path above before moving it here!
            sample: None,
        })
    }
}

impl Media {
    fn words(path: &Path) -> Result<Box<[String]>> {
        static RE_WORDS_COPY: OnceLock<Regex> = OnceLock::new();
        static RE_WORDS_MULT: OnceLock<Regex> = OnceLock::new();
        let rec = RE_WORDS_COPY.get_or_init(|| Regex::new(r" copy( \d+)?$").unwrap());
        let rem = RE_WORDS_MULT.get_or_init(|| Regex::new(r"-\d+$").unwrap());

        let name = path
            .file_stem()
            .and_then(|x| x.to_str())
            .ok_or_else(|| anyhow!("no file name: {path:?}"))?;
        let name = rec.split(name).next().ok_or_else(|| anyhow!("only copy"))?;
        let name = rem.split(name).next().ok_or_else(|| anyhow!("only -#"))?;

        let mut words = name
            .split(&[' ', '.', '-', '_'])
            .filter(|s| !s.is_empty())
            .filter(|s| !(s.len() == 1 && s.is_ascii())) // remove vowels.
            .map(|s| match OPT.get().unwrap().case {
                true => s.to_owned(),
                false => s.to_lowercase(),
            })
            .collect::<Vec<_>>();
        words.sort_unstable();
        words.dedup();
        Ok(words.into_boxed_slice())
    }

    fn cache_sample(&mut self) {
        if self.sample.is_none() {
            let grab_sample = || {
                let mut file = File::open(&self.path)?;
                let mut buf = vec![0; OPT.get().unwrap().sample];
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
                    eprintln!("  {err}");
                    Some(None)
                }
            };
        }
    }
}

impl PartialEq for Media {
    fn eq(&self, other: &Self) -> bool {
        self.path.eq(&other.path)
    }
}

impl Eq for Media {}

impl PartialOrd for Media {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Media {
    fn cmp(&self, other: &Self) -> Ordering {
        self.path.cmp(&other.path)
    }
}
