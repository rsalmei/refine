use crate::utils;
use anyhow::Result;
use clap::{Args, ValueEnum};
use human_repr::HumanCount;
use std::cmp::Ordering;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Args)]
pub struct List {
    /// Sort by.
    #[arg(short, long, value_enum, default_value_t = By::Name)]
    by: By,
    /// Use descending order.
    #[arg(short, long)]
    desc: bool,
}

#[derive(Debug, Copy, Clone, ValueEnum)]
pub enum By {
    Name,
    Size,
    Path,
}

fn opt() -> &'static List {
    match &super::args().cmd {
        super::Command::List(opt) => opt,
        _ => unreachable!(),
    }
}

#[derive(Debug)]
pub struct Media {
    path: PathBuf,
    name: String,
    size: u64,
}

pub fn run(mut medias: Vec<Media>) -> Result<()> {
    println!("=> Listing files...\n");

    let compare = match opt().by {
        By::Name => |a: &Media, b: &Media| a.name.cmp(&b.name),
        By::Size => |a: &Media, b: &Media| a.size.cmp(&b.size),
        By::Path => |a: &Media, b: &Media| a.path.cmp(&b.path),
    };
    let compare: &dyn Fn(&Media, &Media) -> Ordering = match opt().desc {
        true => &|a, b| compare(a, b).reverse(),
        false => &compare,
    };
    medias.sort_unstable_by(compare);
    medias
        .iter()
        .for_each(|m| println!("{} - {}", m.size.human_count_bytes(), m.path.display()));

    if !medias.is_empty() {
        println!();
    }
    let size = medias.iter().map(|m| m.size).sum::<u64>();
    println!("total files: {} ({})", medias.len(), size.human_count("B"));
    Ok(())
}

impl TryFrom<PathBuf> for Media {
    type Error = anyhow::Error;

    fn try_from(path: PathBuf) -> Result<Self> {
        let (name, _) = utils::file_stem_ext(&path)?;
        Ok(Self {
            name: name.to_lowercase(),
            size: fs::metadata(&path)?.len(),
            path,
        })
    }
}
